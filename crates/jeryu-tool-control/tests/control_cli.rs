use std::path::Path;
use std::process::Command;

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
    assert!(stderr.starts_with("registry-summary accepts only --check\n"));
    assert!(stderr.contains("repair_hint:"));
    assert!(stderr.contains("docs_url: docs/tools-registry.md"));
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
