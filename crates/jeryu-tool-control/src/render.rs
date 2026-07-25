use crate::pin::Pin;
use crate::render_rules::{ensure_script, regex, render_consumer, require_function};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CANONICAL_REPOS: [&str; 11] = [
    "jeryu",
    "jeryu-cache",
    "jeryu-ci-runner",
    "jeryu-core",
    "jeryu-deploy",
    "jeryu-intelligence",
    "jeryu-jira",
    "jeryu-release-ops",
    "jeryu-tool",
    "jeryu-tool-finder",
    "jeryu-web",
];

#[derive(Default)]
struct Args {
    check: bool,
    repos: Vec<String>,
    roots: Vec<String>,
    heads: Vec<String>,
    family_root: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => parsed.check = true,
            "--repo" | "--repo-root" | "--expected-head" | "--family-root" => {
                let flag = &args[index];
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{flag} requires a value"))?
                    .clone();
                match flag.as_str() {
                    "--repo" => parsed.repos.push(value),
                    "--repo-root" => parsed.roots.push(value),
                    "--expected-head" => parsed.heads.push(value),
                    "--family-root" => parsed.family_root = Some(PathBuf::from(value)),
                    _ => unreachable!(),
                }
            }
            value => return Err(format!("unrecognized renderer argument: {value}")),
        }
        index += 1;
    }
    Ok(parsed)
}

fn git_output(root: &Path, args: &[&str]) -> Result<String, String> {
    let token_file = env::var_os("JERYU_FORGE_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/ubuntu/.jeryu/secrets/merge-token"));
    let token = fs::read_to_string(token_file)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("NO_PROXY", "127.0.0.1,localhost,::1")
        .env("no_proxy", "127.0.0.1,localhost,::1");
    if let Some(token) = token {
        command
            .env("GIT_CONFIG_COUNT", "2")
            .env("GIT_CONFIG_KEY_0", "http.extraHeader")
            .env(
                "GIT_CONFIG_VALUE_0",
                format!("Authorization: Bearer {token}"),
            )
            .env("GIT_CONFIG_KEY_1", "http.followRedirects")
            .env("GIT_CONFIG_VALUE_1", "false");
    } else {
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.followRedirects")
            .env("GIT_CONFIG_VALUE_0", "false");
    }
    let output = command.output().map_err(|error| {
        format!(
            "renderer custody check failed for {}: {error}",
            root.display()
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        return Err(format!(
            "renderer custody check failed for {}: {}",
            root.display(),
            if stderr.is_empty() { stdout } else { stderr }
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn validate_write_root(name: &str, root: &Path, expected_head: &str) -> Result<(), String> {
    if !git_output(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Err(format!(
            "renderer write root must start clean: {name}={}",
            root.display()
        ));
    }
    let expected_origin = format!("http://127.0.0.1:8787/git/jeryu/{name}.git");
    let origin = git_output(root, &["remote", "get-url", "origin"])?;
    if origin != expected_origin {
        return Err(format!(
            "renderer write root has non-canonical origin: {name}={origin}; expected {expected_origin}"
        ));
    }
    let remote_line = git_output(root, &["ls-remote", "--heads", "origin", "refs/heads/main"])?;
    let fields: Vec<&str> = remote_line.split_whitespace().collect();
    if fields.len() != 2
        || fields[1] != "refs/heads/main"
        || !regex("^[0-9a-f]{40}$").is_match(fields[0])
    {
        return Err(format!(
            "renderer could not resolve protected main for {name}"
        ));
    }
    let head = git_output(root, &["rev-parse", "HEAD"])?;
    if head != expected_head {
        return Err(format!(
            "renderer write root HEAD mismatch: {name} expected={expected_head} actual={head}"
        ));
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", "--is-ancestor", fields[0], &head])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("renderer ancestry check failed: {error}"))?;
    if !status.success() {
        return Err(format!(
            "renderer write root is not based on current protected main: {name} main={} head={head}",
            fields[0]
        ));
    }
    if !git_output(
        root,
        &["rev-list", "--merges", &format!("{}..{head}", fields[0])],
    )?
    .is_empty()
    {
        return Err(format!(
            "renderer write root contains non-linear commits after protected main: {name}"
        ));
    }
    Ok(())
}

fn repo_root(
    tool_root: &Path,
    family_root: &Path,
    name: &str,
    overrides: &BTreeMap<String, PathBuf>,
) -> PathBuf {
    overrides.get(name).cloned().unwrap_or_else(|| {
        if name == "jeryu-tool" {
            tool_root.to_owned()
        } else {
            family_root.join(name)
        }
    })
}

fn consumer_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let fixed = [
        "ops/ci/ensure-jankurai.sh",
        "ops/ci/lib.sh",
        "ops/ci/common.sh",
        "ops/ci/coverage.sh",
        "ops/ci/pr-ci.sh",
        "ci-fast-push.sh",
        "agent/ci-lanes.toml",
        "agent/audit-policy.toml",
        "agent/native-cli-manifest.toml",
        "images/agent-sandbox/Dockerfile",
        "images/agent-sandbox/README.md",
        "ops/agent-sandbox/smoke.sh",
        "scripts/ci-doctor.sh",
        "crates/jeryu-api/src/ci_bridge.rs",
        "crates/jeryu-repogate/tests/ci_lanes.rs",
        "docs/testing.md",
        "docs/audit-rules.md",
        "policy/default-audit-policy.toml",
    ];
    let mut paths: BTreeSet<PathBuf> = fixed
        .iter()
        .map(|relative| root.join(relative))
        .filter(|path| path.is_file())
        .collect();
    let workflows = root.join(".github/workflows");
    if workflows.is_dir() {
        for entry in fs::read_dir(&workflows)
            .map_err(|error| format!("failed to read {}: {error}", workflows.display()))?
        {
            let path = entry
                .map_err(|error| format!("failed to read {}: {error}", workflows.display()))?
                .path();
            if path.extension().and_then(|value| value.to_str()) == Some("yml") {
                paths.insert(path);
            }
        }
    }
    Ok(paths.into_iter().collect())
}

pub fn run(tool_root: &Path, raw_args: &[String]) -> Result<i32, String> {
    let mut args = parse_args(raw_args)?;
    if args.repos.is_empty() && !args.check {
        args.check = true;
        eprintln!(
            "unscoped renderer invocation is check-only; pass --repo, --repo-root, and \
--expected-head for writes"
        );
    }
    for name in &args.repos {
        if !CANONICAL_REPOS.contains(&name.as_str()) {
            return Err(format!("invalid --repo {name:?}"));
        }
    }
    let pin = Pin::load(tool_root)?;
    let function = require_function(tool_root)?;
    let family_root = fs::canonicalize(args.family_root.unwrap_or_else(|| {
        env::var_os("JERYU_FAMILY_ROOT").map_or_else(
            || tool_root.parent().unwrap_or(tool_root).to_owned(),
            PathBuf::from,
        )
    }))
    .map_err(|error| format!("failed to resolve family root: {error}"))?;
    let repos = if args.repos.is_empty() {
        CANONICAL_REPOS.iter().map(ToString::to_string).collect()
    } else {
        args.repos
    };
    let repo_set: BTreeSet<&str> = repos.iter().map(String::as_str).collect();
    let mut overrides = BTreeMap::new();
    for value in args.roots {
        let (name, raw_path) = value.split_once('=').ok_or_else(|| {
            format!("invalid --repo-root {value:?}; expected canonical-name=/absolute/path")
        })?;
        if !CANONICAL_REPOS.contains(&name) || raw_path.is_empty() {
            return Err(format!(
                "invalid --repo-root {value:?}; expected canonical-name=/absolute/path"
            ));
        }
        let path = fs::canonicalize(raw_path)
            .map_err(|_| format!("--repo-root is not a Git worktree: {raw_path}"))?;
        if !path.join(".git").exists() {
            return Err(format!(
                "--repo-root is not a Git worktree: {}",
                path.display()
            ));
        }
        overrides.insert(name.to_owned(), path);
    }
    let unexpected: Vec<&String> = overrides
        .keys()
        .filter(|name| !repo_set.contains(name.as_str()))
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "--repo-root without matching --repo: {unexpected:?}"
        ));
    }
    let mut heads = BTreeMap::new();
    if !args.check {
        let missing: Vec<&&str> = repo_set
            .iter()
            .filter(|name| !overrides.contains_key(**name))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "renderer write mode requires an explicit --repo-root for every selected repo: {missing:?}"
            ));
        }
        for value in args.heads {
            let (name, head) = value.split_once('=').ok_or_else(|| {
                format!("invalid --expected-head {value:?}; expected canonical-name=40-hex-sha")
            })?;
            if !CANONICAL_REPOS.contains(&name) || !regex("^[0-9a-f]{40}$").is_match(head) {
                return Err(format!(
                    "invalid --expected-head {value:?}; expected canonical-name=40-hex-sha"
                ));
            }
            if heads.insert(name.to_owned(), head.to_owned()).is_some() {
                return Err(format!("duplicate --expected-head for {name}"));
            }
        }
        let unexpected: Vec<&String> = heads
            .keys()
            .filter(|name| !repo_set.contains(name.as_str()))
            .collect();
        if !unexpected.is_empty() {
            return Err(format!(
                "--expected-head without matching --repo: {unexpected:?}"
            ));
        }
        let missing: Vec<&&str> = repo_set
            .iter()
            .filter(|name| !heads.contains_key(**name))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "renderer write mode requires --expected-head for every selected repo: {missing:?}"
            ));
        }
        for name in &repos {
            validate_write_root(name, &overrides[name], &heads[name])?;
        }
    } else if !args.heads.is_empty() {
        return Err("--expected-head is valid only in explicit write mode".to_owned());
    }

    let mut changed = Vec::new();
    if repo_set.contains("jeryu-tool") {
        let path = repo_root(tool_root, &family_root, "jeryu-tool", &overrides)
            .join("generated/jankurai-pin.env");
        if fs::read_to_string(&path).ok().as_deref() != Some(&pin.env_text()) {
            changed.push(path.clone());
            if !args.check {
                fs::create_dir_all(path.parent().unwrap_or(tool_root))
                    .map_err(|error| format!("failed to create generated directory: {error}"))?;
                fs::write(&path, pin.env_text())
                    .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
            }
        }
    }
    for name in &repos {
        let root = repo_root(tool_root, &family_root, name, &overrides);
        if !root.is_dir() {
            continue;
        }
        for path in consumer_paths(&root)? {
            let original = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            let rendered = render_consumer(&path, &pin, &function)?;
            if rendered != original {
                changed.push(path.clone());
                if !args.check {
                    fs::write(&path, rendered)
                        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
                }
            }
        }
    }
    let display = |path: &Path| {
        path.strip_prefix(&family_root)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    if args.check && !changed.is_empty() {
        println!("jankurai identity DRIFT — render these canonical consumers:");
        for path in changed {
            println!("  - {}", display(&path));
        }
        return Ok(1);
    }
    if args.check {
        println!(
            "jankurai identity ok: {} sha256={}",
            pin.get("version"),
            pin.get("binary_sha256")
        );
    } else if changed.is_empty() {
        println!("jankurai identity already current: {}", pin.get("version"));
    } else {
        println!(
            "rendered {} into {} file(s):",
            pin.get("version"),
            changed.len()
        );
        for path in changed {
            println!("  - {}", display(&path));
        }
    }
    Ok(0)
}

pub fn emit_ensure_script(tool_root: &Path, args: &[String]) -> Result<i32, String> {
    if !args.is_empty() {
        return Err("emit-ensure-script accepts no arguments".to_owned());
    }
    let pin = Pin::load(tool_root)?;
    let function = require_function(tool_root)?;
    print!("{}", ensure_script(&pin, &function));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_and_future_identity_are_shape_driven() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pin = Pin::load(&root).expect("pin");
        let function = require_function(&root).expect("function");
        let script = ensure_script(&pin, &function);
        assert!(!pin.shell_block().contains("JERYU_GOVERNED_JANKURAI_BIN"));
        assert!(function.contains("/opt/jain-ci/authority/release-bin/jankurai"));
        assert!(function.contains("/home/ubuntu/.jeryu/bin/jankurai"));
        assert!(function.contains("local mode=receipt-bound"));
        assert!(script.contains(&function));
        let future = "Jankurai 9.9.9 / jankurai 9.9.9 / \
v9.9.9-deadlang-precision-split.9 / https://github.com/neverhuman/jankurai.git\n";
        let rendered = crate::render_rules::semantic_identity_rules(future, &pin);
        assert!(!rendered.contains("9.9.9"));
        assert!(rendered.contains(pin.get("version")));
        assert!(rendered.contains(pin.get("tag")));
        assert!(rendered.contains(pin.get("repo")));
    }
}
