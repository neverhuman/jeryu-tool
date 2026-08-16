use crate::pin::Pin;
use crate::render_rules::{
    ManifestAuthority, RenderContext, ensure_script, regex, render_consumer, require_function,
    sandbox_receipt,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CANONICAL_FAMILY_ROOT: &str = "/home/ubuntu/jain-split/jeryu-split";
const DEFAULT_TOKEN_FILE: &str = "/home/ubuntu/.jeryu/secrets/merge-token";
const GIT_BIN: &str = "/usr/bin/git";
const SHA256_BIN: &str = "/usr/bin/sha256sum";
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

fn git_local_output_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
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
    Ok(output.stdout)
}

fn git_local_output(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_local_output_bytes(root, args)?;
    Ok(String::from_utf8_lossy(&output).trim().to_owned())
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

fn git_remote_url(root: &Path, name: &str) -> Result<Option<String>, String> {
    let mut command = local_git_command(root);
    command.args(["remote", "get-url", name]);
    let output = command.output().map_err(|error| {
        format!(
            "renderer custody check failed for {}: {error}",
            root.display()
        )
    })?;
    if output.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        ));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("No such remote") {
        Ok(None)
    } else {
        Err(format!(
            "renderer custody check failed for {}: {}",
            root.display(),
            stderr.trim()
        ))
    }
}

fn protected_main_commit(
    tool_root: &Path,
    name: &str,
    expected_origin: &str,
    authenticated: bool,
) -> Result<String, String> {
    if let Some(origin) = git_remote_url(tool_root, "origin")? {
        if origin != expected_origin {
            return Err(format!(
                "renderer tool source has non-canonical origin: {origin}; expected {expected_origin}"
            ));
        }
        if authenticated {
            return remote_main(name, expected_origin);
        }
        let commit = git_local_output(tool_root, &["rev-parse", "refs/remotes/origin/main"])?;
        if !regex("^[0-9a-f]{40}$").is_match(&commit) {
            return Err("renderer could not resolve tracked protected jeryu-tool main".to_owned());
        }
        return Ok(commit);
    }

    if let Ok(base) = env::var("JAIN_CONTRACT_BASE_REF") {
        if !regex("^[0-9a-f]{40}$").is_match(&base) {
            return Err(format!(
                "harness JAIN_CONTRACT_BASE_REF is not a full commit sha: {base}"
            ));
        }
        git_local_output(
            tool_root,
            &["cat-file", "-e", &format!("{base}^{{commit}}")],
        )?;
        return Ok(base);
    }

    if authenticated {
        return remote_main(name, expected_origin);
    }

    if env::var("JAIN_RELEASE_CI").ok().as_deref() == Some("1") {
        return Err(
            "release renderer custody requires harness-authenticated JAIN_CONTRACT_BASE_REF \
when origin is absent"
                .to_owned(),
        );
    }

    Err(format!(
        "renderer custody check failed for {}: no origin remote and no JAIN_CONTRACT_BASE_REF",
        tool_root.display()
    ))
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
    let line = String::from_utf8_lossy(&output.stdout);
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() != 2
        || fields[1] != "refs/heads/main"
        || !regex("^[0-9a-f]{40}$").is_match(fields[0])
    {
        return Err(format!(
            "renderer could not resolve protected main for {name}"
        ));
    }
    Ok(fields[0].to_owned())
}

fn sha256_bytes(bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new(SHA256_BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("renderer could not start SHA-256 verifier: {error}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "renderer SHA-256 verifier has no input pipe".to_owned())?
        .write_all(bytes)
        .map_err(|error| format!("renderer could not stream SHA-256 input: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("renderer SHA-256 verifier failed: {error}"))?;
    if !output.status.success() {
        return Err("renderer SHA-256 verifier failed".to_owned());
    }
    let digest = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_owned();
    if !regex("^[0-9a-f]{64}$").is_match(&digest) {
        return Err("renderer SHA-256 verifier returned malformed output".to_owned());
    }
    Ok(digest)
}

fn manifest_authority(tool_root: &Path, authenticated: bool) -> Result<ManifestAuthority, String> {
    let expected_origin = "http://127.0.0.1:8787/git/jeryu/jeryu-tool.git";
    let commit = protected_main_commit(tool_root, "jeryu-tool", expected_origin, authenticated)?;
    let manifest = fs::read(tool_root.join("tool-manifest.toml"))
        .map_err(|error| format!("failed to read tool-manifest.toml: {error}"))?;
    let object = format!("{commit}:tool-manifest.toml");
    let protected_manifest = git_local_output_bytes(tool_root, &["show", &object])?;
    if manifest != protected_manifest {
        return Err(
            "tool-manifest.toml must land on protected jeryu-tool main before sandbox receipt rendering"
                .to_owned(),
        );
    }
    let tree_object = format!("{commit}^{{tree}}");
    let tree = git_local_output(tool_root, &["rev-parse", &tree_object])?;
    if !regex("^[0-9a-f]{40}$").is_match(&tree) {
        return Err("renderer resolved malformed jeryu-tool manifest tree".to_owned());
    }
    Ok(ManifestAuthority {
        commit,
        tree,
        sha256: sha256_bytes(&protected_manifest)?,
    })
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

fn validate_index_state(name: &str, root: &Path) -> Result<(), String> {
    let entries = git_local_output_bytes(root, &["ls-files", "-v", "-z"])?;
    if !entries.is_empty() && entries.last() != Some(&0) {
        return Err(format!(
            "renderer write root has malformed tracked index state: {name}={}",
            root.display()
        ));
    }
    if entries.is_empty() {
        return Ok(());
    }
    let entries = entries.strip_suffix(&[0]).unwrap_or(entries.as_slice());
    for entry in entries.split(|byte| *byte == 0) {
        if entry.len() < 3 || entry[0] != b'H' || entry[1] != b' ' {
            return Err(format!(
                "renderer write root has tracked index suppression or nonordinary state: \
{name}={} entry={:?}",
                root.display(),
                String::from_utf8_lossy(entry)
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_write_target(root: &Path, path: &Path) -> Result<(), String> {
    let fail = || {
        format!(
            "renderer write target must be a stable regular HEAD blob: {}",
            path.display()
        )
    };
    let relative = path.strip_prefix(root).map_err(|_| fail())?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(fail());
    }
    let relative = relative.to_str().ok_or_else(fail)?;
    if fs::canonicalize(path).map_err(|_| fail())? != path {
        return Err(fail());
    }
    let before = fs::symlink_metadata(path).map_err(|_| fail())?;
    if !before.file_type().is_file() || before.file_type().is_symlink() || before.nlink() != 1 {
        return Err(fail());
    }

    let entry = git_local_output_bytes(
        root,
        &["ls-tree", "-z", "--full-tree", "HEAD", "--", relative],
    )?;
    if entry.last() != Some(&0) || entry[..entry.len() - 1].contains(&0) {
        return Err(fail());
    }
    let entry = &entry[..entry.len() - 1];
    let tab = entry
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or_else(fail)?;
    if &entry[tab + 1..] != relative.as_bytes() {
        return Err(fail());
    }
    let header = std::str::from_utf8(&entry[..tab]).map_err(|_| fail())?;
    let fields: Vec<&str> = header.split_whitespace().collect();
    if fields.len() != 3
        || !matches!(fields[0], "100644" | "100755")
        || fields[1] != "blob"
        || !regex("^[0-9a-f]{40}$").is_match(fields[2])
    {
        return Err(fail());
    }
    let working = git_local_output(root, &["hash-object", "--no-filters", "--", relative])?;
    if working != fields[2] {
        return Err(fail());
    }
    let after = fs::symlink_metadata(path).map_err(|_| fail())?;
    if !same_file_identity(&before, &after) {
        return Err(fail());
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_write_target(_root: &Path, path: &Path) -> Result<(), String> {
    Err(format!(
        "renderer write target custody requires Unix metadata: {}",
        path.display()
    ))
}

fn validate_write_targets(repository_targets: &[(PathBuf, Vec<PathBuf>)]) -> Result<(), String> {
    for (root, targets) in repository_targets {
        let name = root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("<unknown>");
        validate_index_state(name, root)?;
        for target in targets {
            validate_write_target(root, target)?;
        }
    }
    Ok(())
}

fn apply_rendered_changes(
    repository_targets: &[(PathBuf, Vec<PathBuf>)],
    rendered: &[(PathBuf, String)],
) -> Result<(), String> {
    validate_write_targets(repository_targets)?;
    let allowed: BTreeSet<&Path> = repository_targets
        .iter()
        .flat_map(|(_, targets)| targets.iter().map(PathBuf::as_path))
        .collect();
    let mut unique = BTreeSet::new();
    for (path, _) in rendered {
        if !allowed.contains(path.as_path()) || !unique.insert(path.as_path()) {
            return Err(format!(
                "renderer attempted an unvalidated or duplicate write target: {}",
                path.display()
            ));
        }
    }
    for (path, value) in rendered {
        fs::write(path, value)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
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
    validate_index_state(name, root)?;
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
    let protected_main = remote_main(name, &expected_origin)?;
    let mut ancestry = local_git_command(root);
    let status = ancestry
        .args(["merge-base", "--is-ancestor", &protected_main, &head])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("renderer ancestry check failed: {error}"))?;
    if !status.success() {
        return Err(format!(
            "renderer write root is not based on current protected main: {name} main={} head={head}",
            protected_main
        ));
    }
    if !git_local_output(
        root,
        &["rev-list", "--merges", &format!("{protected_main}..{head}")],
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
        "images/agent-sandbox/jankurai-installation-receipt.json",
        "ops/agent-sandbox/smoke.sh",
        "ops/ci/test-governed-jankurai.sh",
        "scripts/ci-doctor.sh",
        "crates/jeryu-api/src/ci_bridge.rs",
        "crates/jeryu-api/tests/jankurai_governance.rs",
        "crates/jeryu-repogate/tests/ci_lanes.rs",
        "docs/release.md",
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

    let mut repository_targets = Vec::new();
    for name in &repos {
        let root = repo_root(tool_root, &family_root, name, &overrides);
        if !root.is_dir() {
            continue;
        }
        let mut targets = consumer_paths(&root)?;
        if name == "jeryu-tool" {
            targets.push(root.join("generated/jankurai-pin.env"));
            targets.sort();
            targets.dedup();
        }
        repository_targets.push((root, targets));
    }
    if !args.check {
        validate_write_targets(&repository_targets)?;
    }

    let authority = manifest_authority(tool_root, !args.check)?;
    let receipt = sandbox_receipt(&pin, &authority)?;
    let context = RenderContext {
        authority,
        image_receipt_sha256: sha256_bytes(receipt.as_bytes())?,
    };

    let mut changed = Vec::new();
    let mut rendered_changes = Vec::new();
    if repo_set.contains("jeryu-tool") {
        let path = repo_root(tool_root, &family_root, "jeryu-tool", &overrides)
            .join("generated/jankurai-pin.env");
        if fs::read_to_string(&path).ok().as_deref() != Some(&pin.env_text()) {
            changed.push(path.clone());
            if !args.check {
                rendered_changes.push((path, pin.env_text()));
            }
        }
    }
    for (_, targets) in &repository_targets {
        for path in targets {
            if path.ends_with("generated/jankurai-pin.env") {
                continue;
            }
            let original = fs::read_to_string(path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            let rendered = render_consumer(path, &pin, &function, &context)?;
            if rendered != original {
                changed.push(path.clone());
                if !args.check {
                    rendered_changes.push((path.clone(), rendered));
                }
            }
        }
    }
    if !args.check {
        apply_rendered_changes(&repository_targets, &rendered_changes)?;
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

    #[cfg(unix)]
    fn run_fixture_git(root: &Path, args: &[&str]) {
        let mut command = local_git_command(root);
        let status = command.args(args).status().expect("run fixture Git");
        assert!(status.success(), "fixture Git failed: {args:?}");
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
        assert_eq!(
            sha256_bytes(b"abc").expect("SHA-256"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(unix)]
    #[test]
    fn protected_main_commit_accepts_harness_contract_base_without_origin() {
        let root = test_root("contract-base");
        run_fixture_git(&root, &["init", "-q"]);
        run_fixture_git(&root, &["config", "user.name", "Jeryu Test"]);
        run_fixture_git(&root, &["config", "user.email", "jeryu-test@invalid"]);
        fs::write(root.join("marker.txt"), "fixture\n").expect("write fixture");
        run_fixture_git(&root, &["add", "marker.txt"]);
        run_fixture_git(&root, &["commit", "-q", "-m", "fixture"]);
        let head = git_local_output(&root, &["rev-parse", "HEAD"]).expect("head");
        // SAFETY: test-only env mutation in an isolated process.
        unsafe {
            env::set_var("JAIN_CONTRACT_BASE_REF", &head);
        }
        let resolved = protected_main_commit(
            &root,
            "jeryu-tool",
            "http://127.0.0.1:8787/git/jeryu/jeryu-tool.git",
            false,
        )
        .expect("harness contract base");
        assert_eq!(resolved, head);
        unsafe {
            env::remove_var("JAIN_CONTRACT_BASE_REF");
        }
        fs::remove_dir_all(root).expect("remove test root");
    }

    #[cfg(unix)]
    #[test]
    fn protected_main_commit_rejects_release_without_contract_base_or_origin() {
        let root = test_root("release-no-base");
        run_fixture_git(&root, &["init", "-q"]);
        unsafe {
            env::set_var("JAIN_RELEASE_CI", "1");
        }
        let error = protected_main_commit(
            &root,
            "jeryu-tool",
            "http://127.0.0.1:8787/git/jeryu/jeryu-tool.git",
            false,
        )
        .expect_err("release without origin must fail closed");
        assert!(error.contains("JAIN_CONTRACT_BASE_REF"));
        unsafe {
            env::remove_var("JAIN_RELEASE_CI");
        }
        fs::remove_dir_all(root).expect("remove test root");
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

    #[cfg(unix)]
    #[test]
    fn rendered_changes_reject_hidden_index_state_without_mutating_bytes() {
        let root = test_root("hidden-index");
        let status = Command::new(GIT_BIN)
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .expect("git init");
        assert!(status.success());
        run_fixture_git(&root, &["config", "user.name", "Jeryu Test"]);
        run_fixture_git(&root, &["config", "user.email", "jeryu-test@invalid"]);
        let target = root.join("generated.txt");
        fs::write(&target, "original\n").expect("write tracked target");
        run_fixture_git(&root, &["add", "generated.txt"]);
        run_fixture_git(&root, &["commit", "-q", "-m", "fixture"]);

        let targets = vec![(root.clone(), vec![target.clone()])];
        validate_write_targets(&targets).expect("ordinary tracked target");

        for (set_flag, clear_flag) in [
            ("--assume-unchanged", "--no-assume-unchanged"),
            ("--skip-worktree", "--no-skip-worktree"),
        ] {
            run_fixture_git(&root, &["update-index", set_flag, "generated.txt"]);
            let hostile = format!("hidden by {set_flag}\n");
            fs::write(&target, &hostile).expect("write hostile hidden bytes");
            assert!(
                git_local_output(&root, &["status", "--porcelain"])
                    .expect("hidden status")
                    .is_empty(),
                "fixture change was not hidden by {set_flag}"
            );
            assert!(validate_index_state("fixture", &root).is_err());
            assert!(validate_write_target(&root, &target).is_err());
            let replacement = vec![(target.clone(), "rendered\n".to_owned())];
            assert!(apply_rendered_changes(&targets, &replacement).is_err());
            assert_eq!(
                fs::read_to_string(&target).expect("read target after rejection"),
                hostile
            );
            run_fixture_git(&root, &["update-index", clear_flag, "generated.txt"]);
            fs::write(&target, "original\n").expect("restore target");
            validate_write_targets(&targets).expect("restored ordinary target");
        }

        fs::remove_dir_all(root).expect("remove test root");
    }
}
