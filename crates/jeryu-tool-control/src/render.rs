use crate::pin::Pin;
use crate::render_rules::{ensure_script, regex, render_consumer, require_function};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File};
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CANONICAL_FAMILY_ROOT: &str = "/home/ubuntu/jain-split/jeryu-split";
const DEFAULT_TOKEN_FILE: &str = "/home/ubuntu/.jeryu/secrets/merge-token";
const GIT_BIN: &str = "/usr/bin/git";
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
                    "--family-root" => {
                        if parsed.family_root.replace(PathBuf::from(value)).is_some() {
                            return Err("duplicate --family-root".to_owned());
                        }
                    }
                    _ => unreachable!(),
                }
            }
            value => return Err(format!("unrecognized renderer argument: {value}")),
        }
        index += 1;
    }
    Ok(parsed)
}

fn scrub_git_environment(command: &mut Command) {
    for key in [
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_ASKPASS",
        "GIT_COMMON_DIR",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_DIR",
        "GIT_EXEC_PATH",
        "GIT_EXTERNAL_DIFF",
        "GIT_OBJECT_DIRECTORY",
        "GIT_PROXY_COMMAND",
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_WORK_TREE",
        "SSH_ASKPASS",
    ] {
        command.env_remove(key);
    }
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("NO_PROXY", "127.0.0.1,localhost,::1")
        .env("no_proxy", "127.0.0.1,localhost,::1");
}

fn local_git_command(root: &Path) -> Command {
    let mut command = Command::new(GIT_BIN);
    command.arg("-C").arg(root).args([
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "credential.helper=",
        "-c",
        "diff.external=",
        "-c",
        "submodule.recurse=false",
    ]);
    scrub_git_environment(&mut command);
    command
}

fn git_local_output(root: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = local_git_command(root);
    command.args(args);
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

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
        && left.nlink() == right.nlink()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(unix)]
fn read_token_file_with_hook(path: &Path, after_open: impl FnOnce()) -> Result<String, String> {
    let fail = || {
        format!(
            "renderer credential file failed custody validation: {}",
            path.display()
        )
    };
    if !path.is_absolute() || fs::canonicalize(path).map_err(|_| fail())? != path {
        return Err(fail());
    }
    let before = fs::symlink_metadata(path).map_err(|_| fail())?;
    let process_uid = fs::metadata("/proc/self").map_err(|_| fail())?.uid();
    if !before.file_type().is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.uid() != process_uid
        || before.mode() & 0o777 != 0o600
        || before.len() == 0
        || before.len() > 16_384
    {
        return Err(fail());
    }
    let mut file = File::open(path).map_err(|_| fail())?;
    let opened = file.metadata().map_err(|_| fail())?;
    if !same_file_identity(&before, &opened) {
        return Err(fail());
    }
    after_open();
    let mut value = String::new();
    (&mut file)
        .take(16_385)
        .read_to_string(&mut value)
        .map_err(|_| fail())?;
    let after = file.metadata().map_err(|_| fail())?;
    let path_after = fs::symlink_metadata(path).map_err(|_| fail())?;
    if !same_file_identity(&opened, &after)
        || !same_file_identity(&after, &path_after)
        || value.len() > 16_384
    {
        return Err(fail());
    }
    let value = value.trim_end_matches(['\r', '\n']);
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(fail());
    }
    Ok(value.to_owned())
}

#[cfg(not(unix))]
fn read_token_file_with_hook(path: &Path, _after_open: impl FnOnce()) -> Result<String, String> {
    Err(format!(
        "renderer credential custody requires Unix metadata: {}",
        path.display()
    ))
}

fn read_forge_token() -> Result<String, String> {
    let path = env::var_os("JERYU_FORGE_TOKEN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TOKEN_FILE));
    read_token_file_with_hook(&path, || {})
}

fn remote_main(name: &str, expected_origin: &str) -> Result<String, String> {
    let token = read_forge_token()?;
    let mut command = Command::new(GIT_BIN);
    command.current_dir("/").args([
        "-c",
        "credential.helper=",
        "ls-remote",
        "--heads",
        expected_origin,
        "refs/heads/main",
    ]);
    scrub_git_environment(&mut command);
    command
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "http.extraHeader")
        .env(
            "GIT_CONFIG_VALUE_0",
            format!("Authorization: Bearer {token}"),
        )
        .env("GIT_CONFIG_KEY_1", "http.followRedirects")
        .env("GIT_CONFIG_VALUE_1", "false");
    let output = command
        .output()
        .map_err(|_| format!("renderer could not resolve protected main for {name}"))?;
    if !output.status.success() {
        return Err(format!(
            "renderer could not resolve protected main for {name}"
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn validate_repository_storage(root: &Path) -> Result<(), String> {
    let dot_git = root.join(".git");
    let metadata = fs::symlink_metadata(&dot_git).map_err(|_| {
        format!(
            "renderer write root lacks repository-local .git: {}",
            root.display()
        )
    })?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "renderer write root must have an ordinary repository-local .git directory: {}",
            root.display()
        ));
    }
    let common = git_local_output(root, &["rev-parse", "--git-common-dir"])?;
    let common = PathBuf::from(common);
    let common = if common.is_absolute() {
        common
    } else {
        root.join(common)
    };
    let common = fs::canonicalize(common).map_err(|_| {
        format!(
            "renderer write root has unresolved Git common directory: {}",
            root.display()
        )
    })?;
    let expected = fs::canonicalize(&dot_git).map_err(|_| {
        format!(
            "renderer write root has unresolved repository-local .git: {}",
            root.display()
        )
    })?;
    if common != expected {
        return Err(format!(
            "renderer write root uses an alternate Git common directory: {}",
            root.display()
        ));
    }
    let registered = git_local_output(root, &["worktree", "list", "--porcelain"])?;
    let worktrees: Vec<&str> = registered
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .collect();
    if worktrees.len() != 1 || Path::new(worktrees[0]) != root {
        return Err(format!(
            "renderer write root repository has alternate registered worktrees: {}",
            root.display()
        ));
    }
    let top = fs::canonicalize(git_local_output(root, &["rev-parse", "--show-toplevel"])?)
        .map_err(|_| {
            format!(
                "renderer write root has unresolved top level: {}",
                root.display()
            )
        })?;
    if top != root {
        return Err(format!(
            "renderer write root top-level mismatch: expected={} actual={}",
            root.display(),
            top.display()
        ));
    }
    Ok(())
}

fn validate_write_root(
    name: &str,
    root: &Path,
    family_root: &Path,
    expected_head: &str,
) -> Result<(), String> {
    let expected_root = family_root.join(name);
    if root != expected_root {
        return Err(format!(
            "renderer write root is not the canonical physical checkout: {name}={} expected={}",
            root.display(),
            expected_root.display()
        ));
    }
    validate_repository_storage(root)?;
    if !git_local_output(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Err(format!(
            "renderer write root must start clean: {name}={}",
            root.display()
        ));
    }
    let expected_origin = format!("http://127.0.0.1:8787/git/jeryu/{name}.git");
    let origin = git_local_output(root, &["remote", "get-url", "origin"])?;
    if origin != expected_origin {
        return Err(format!(
            "renderer write root has non-canonical origin: {name}={origin}; expected {expected_origin}"
        ));
    }
    let head = git_local_output(root, &["rev-parse", "HEAD"])?;
    if head != expected_head {
        return Err(format!(
            "renderer write root HEAD mismatch: {name} expected={expected_head} actual={head}"
        ));
    }
    let remote_line = remote_main(name, &expected_origin)?;
    let fields: Vec<&str> = remote_line.split_whitespace().collect();
    if fields.len() != 2
        || fields[1] != "refs/heads/main"
        || !regex("^[0-9a-f]{40}$").is_match(fields[0])
    {
        return Err(format!(
            "renderer could not resolve protected main for {name}"
        ));
    }
    let mut ancestry = local_git_command(root);
    let status = ancestry
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
    if !git_local_output(
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
    let mut selected = BTreeSet::new();
    for name in &args.repos {
        if !CANONICAL_REPOS.contains(&name.as_str()) {
            return Err(format!("invalid --repo {name:?}"));
        }
        if !selected.insert(name) {
            return Err(format!("duplicate --repo {name:?}"));
        }
    }
    let pin = Pin::load(tool_root)?;
    let function = require_function(tool_root)?;
    let requested_family_root = args.family_root.unwrap_or_else(|| {
        env::var_os("JERYU_FAMILY_ROOT").map_or_else(
            || tool_root.parent().unwrap_or(tool_root).to_owned(),
            PathBuf::from,
        )
    });
    let family_root = fs::canonicalize(&requested_family_root)
        .map_err(|error| format!("failed to resolve family root: {error}"))?;
    if !args.check {
        let canonical_family_root = Path::new(CANONICAL_FAMILY_ROOT);
        if requested_family_root != canonical_family_root || family_root != canonical_family_root {
            return Err(format!(
                "renderer write mode requires the exact canonical family root: {CANONICAL_FAMILY_ROOT}"
            ));
        }
        let canonical_tool_root = canonical_family_root.join("jeryu-tool");
        if tool_root != canonical_tool_root
            || fs::canonicalize(tool_root)
                .map_err(|error| format!("failed to resolve tool root: {error}"))?
                != canonical_tool_root
        {
            return Err(format!(
                "renderer write mode requires the exact canonical tool source: {}",
                canonical_tool_root.display()
            ));
        }
    }
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
        let raw_path = PathBuf::from(raw_path);
        if !raw_path.is_absolute() {
            return Err(format!(
                "invalid --repo-root {value:?}; expected canonical-name=/absolute/path"
            ));
        }
        if !args.check && raw_path != family_root.join(name) {
            return Err(format!(
                "renderer write root is not the canonical physical checkout: {name}={} expected={}",
                raw_path.display(),
                family_root.join(name).display()
            ));
        }
        let path = fs::canonicalize(&raw_path)
            .map_err(|_| format!("--repo-root is not a Git worktree: {}", raw_path.display()))?;
        if !path.join(".git").exists() {
            return Err(format!(
                "--repo-root is not a Git worktree: {}",
                path.display()
            ));
        }
        if overrides.insert(name.to_owned(), path).is_some() {
            return Err(format!("duplicate --repo-root for {name}"));
        }
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
            validate_write_root(name, &overrides[name], &family_root, &heads[name])?;
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
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};
    #[cfg(unix)]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    fn test_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "jeryu-tool-render-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create test root");
        root
    }

    #[cfg(unix)]
    fn write_private(path: &Path, value: &str) {
        fs::write(path, value).expect("write fixture");
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("chmod fixture");
    }

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

    #[cfg(unix)]
    #[test]
    fn credential_file_custody_rejects_mode_links_and_path_swaps() {
        let root = test_root("credential");
        let token = root.join("token");
        write_private(&token, "fixture-token\n");
        assert_eq!(
            read_token_file_with_hook(&token, || {}).expect("private token"),
            "fixture-token"
        );

        fs::set_permissions(&token, fs::Permissions::from_mode(0o640)).expect("chmod hostile");
        assert!(read_token_file_with_hook(&token, || {}).is_err());
        fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).expect("restore mode");

        let hardlink = root.join("hardlink");
        fs::hard_link(&token, &hardlink).expect("hardlink fixture");
        assert!(read_token_file_with_hook(&token, || {}).is_err());
        fs::remove_file(&hardlink).expect("remove hardlink");

        let link = root.join("symlink");
        symlink(&token, &link).expect("symlink fixture");
        assert!(read_token_file_with_hook(&link, || {}).is_err());

        let old = root.join("old-token");
        let replacement = token.clone();
        let result = read_token_file_with_hook(&token, || {
            fs::rename(&replacement, &old).expect("move opened token");
            write_private(&replacement, "replacement-token\n");
        });
        assert!(result.is_err());
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[cfg(unix)]
    #[test]
    fn local_git_checks_disable_checkout_execution_and_reject_git_files() {
        let root = test_root("local-git");
        let status = Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .expect("git init");
        assert!(status.success());
        let marker = root.join("fsmonitor-ran");
        let monitor = root.join("monitor.sh");
        fs::write(
            &monitor,
            format!("#!/bin/sh\ntouch '{}'\nprintf '0\\n'\n", marker.display()),
        )
        .expect("write monitor");
        fs::set_permissions(&monitor, fs::Permissions::from_mode(0o700)).expect("chmod monitor");
        let status = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["config", "core.fsmonitor", monitor.to_str().expect("UTF-8")])
            .status()
            .expect("configure fsmonitor");
        assert!(status.success());
        git_local_output(&root, &["status", "--porcelain"]).expect("scrubbed status");
        assert!(!marker.exists(), "checkout-local fsmonitor executed");

        fs::remove_dir_all(root.join(".git")).expect("remove fixture metadata");
        fs::write(root.join(".git"), "gitdir: /tmp/attacker\n").expect("write gitfile");
        assert!(validate_repository_storage(&root).is_err());
        fs::remove_dir_all(root).expect("remove test root");
    }
}
