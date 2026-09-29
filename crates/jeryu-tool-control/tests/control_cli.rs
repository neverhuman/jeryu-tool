use std::path::Path;
use std::process::Command;
#[cfg(unix)]
use std::{
    os::unix::fs::PermissionsExt,
    time::{SystemTime, UNIX_EPOCH},
};

fn command(root: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_jeryu-toolctl"))
        .args(["--tool-root", root.to_str().expect("UTF-8 test root")])
        .args(arguments)
        .output()
        .expect("run jeryu-toolctl")
}

#[test]
fn registry_check_and_closed_arguments() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = command(&root, &["registry-summary", "--check"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("registry ok: "));
    assert!(output.stderr.is_empty());

    let rejected = command(&root, &["registry-summary", "--unknown"]);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.starts_with("error: unexpected argument \'--unknown\' found"));
    assert!(stderr.contains("Usage: jeryu-toolctl --tool-root <PATH> registry-summary"));
    assert!(stderr.contains("purpose: select one closed jeryu-tool control command"));
    assert!(stderr.contains("reason: the command line is outside the accepted schema"));
    assert!(stderr.contains("run jeryu-toolctl --help for the accepted commands"));
    assert!(stderr.contains("docs_url: docs/toolctl.md"));
}

#[test]
fn a_registry_refusal_keeps_its_own_repair_block() {
    // An empty root has no tools-registry.toml, so the refusal comes from the
    // registry itself rather than from argument parsing.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let rejected = command(&root, &["registry-summary", "--check"]);
    assert!(!rejected.status.success());
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains("missing tools-registry.toml"));
    assert!(stderr.contains("purpose: validate the canonical reusable-tool registry"));
    assert!(stderr.contains("docs_url: docs/tools-registry.md"));
    assert!(stderr.contains(
        "repair_hint: run ops/registry-summary.sh --check after correcting the named input"
    ));
}

#[test]
fn help_lists_every_command_and_the_hidden_askpass_mode() {
    let output = Command::new(env!("CARGO_BIN_EXE_jeryu-toolctl"))
        .arg("--help")
        .output()
        .expect("run jeryu-toolctl --help");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    for command in [
        "emit-ensure-script",
        "registry-summary",
        "render-tool-manifest",
    ] {
        assert!(help.contains(command), "--help omits {command}");
    }
    assert!(help.contains("JERYU_TOOL_GIT_ASKPASS"));
    assert!(help.contains("--tool-root <PATH>"));
}

#[test]
fn the_renderer_flags_are_all_documented_in_help() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = command(&root, &["render-tool-manifest", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    for flag in [
        "--check",
        "--candidate",
        "--repo <NAME>",
        "--repo-root <NAME=PATH>",
        "--expected-head <NAME=SHA>",
        "--family-root <PATH>",
    ] {
        assert!(help.contains(flag), "renderer help omits {flag}");
    }
}

#[test]
fn a_missing_tool_root_is_a_usage_refusal() {
    let rejected = Command::new(env!("CARGO_BIN_EXE_jeryu-toolctl"))
        .arg("registry-summary")
        .output()
        .expect("run jeryu-toolctl without --tool-root");
    assert!(!rejected.status.success());
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains("--tool-root <PATH>"));
    assert!(stderr.contains("common_fixes: pass --tool-root before the command"));
}

#[test]
fn emit_ensure_script_takes_no_arguments() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rejected = command(&root, &["emit-ensure-script", "extra"]);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(stderr.contains("unexpected argument"));
}

#[test]
fn emitted_verifier_is_bound_to_the_static_template() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = command(&root, &["emit-ensure-script"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let verifier = String::from_utf8(output.stdout).expect("UTF-8 verifier");
    let template = std::fs::read_to_string(root.join("ops/render-assets/require-jankurai.sh"))
        .expect("static verifier template");
    assert!(verifier.contains(template.trim_end()));
    assert!(verifier.contains("/opt/jain-ci/authority/release-bin/jankurai"));
    assert!(verifier.contains("# BEGIN GENERATED JANKURAI PIN"));
}

#[cfg(unix)]
#[test]
fn hosted_askpass_is_path_only_and_prompt_bound() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let fixture_root =
        std::env::temp_dir().join(format!("jeryu-tool-askpass-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&fixture_root).expect("create askpass fixture");
    let token_file = fixture_root.join("token");
    std::fs::write(&token_file, "fixture-hosted-token\n").expect("write fixture token");
    std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o600))
        .expect("set fixture token mode");

    let accepted = Command::new(env!("CARGO_BIN_EXE_jeryu-toolctl"))
        .env("JERYU_TOOL_GIT_ASKPASS", "1")
        .env("JERYU_FORGE_TOKEN_FILE", &token_file)
        .arg("Password for 'https://git@git.neverhuman.org/git/jeryu/jeryu-tool.git': ")
        .output()
        .expect("run hosted askpass");
    assert!(accepted.status.success());
    assert_eq!(accepted.stdout, b"fixture-hosted-token\n");
    assert!(accepted.stderr.is_empty());

    let rejected = Command::new(env!("CARGO_BIN_EXE_jeryu-toolctl"))
        .env("JERYU_TOOL_GIT_ASKPASS", "1")
        .env("JERYU_FORGE_TOKEN_FILE", &token_file)
        .arg("Password for 'https://git@git.neverhuman.org/git/veox/jeryu-tool.git': ")
        .output()
        .expect("run hostile askpass");
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(rejected.stderr.is_empty());

    std::fs::remove_dir_all(fixture_root).expect("remove askpass fixture");
}
