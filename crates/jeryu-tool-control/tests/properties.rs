use proptest::prelude::*;
use std::path::Path;
use std::process::Command;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn registry_rejects_arbitrary_unknown_flags(suffix in "[a-z]{1,16}") {
        prop_assume!(suffix != "check");
        prop_assume!(suffix != "help");
        prop_assume!(suffix != "version");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = Command::new(env!("CARGO_BIN_EXE_jeryu-toolctl"))
            .args(["--tool-root", root.to_str().expect("UTF-8 test root")])
            .args(["registry-summary", &format!("--{suffix}")])
            .output()
            .expect("run jeryu-toolctl");
        prop_assert!(!output.status.success());
        prop_assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = format!("error: unexpected argument '--{suffix}' found");
        prop_assert!(stderr.starts_with(&expected));
        prop_assert!(stderr.contains("Usage: jeryu-toolctl --tool-root <PATH> registry-summary"));
        prop_assert!(stderr.contains("repair_hint:"));
        prop_assert!(stderr.contains("docs_url: docs/toolctl.md"));
    }

    #[test]
    fn an_unknown_command_is_refused_with_the_accepted_ones(name in "[a-z][a-z-]{0,16}") {
        prop_assume!(!["emit-ensure-script", "registry-summary", "render-tool-manifest"]
            .contains(&name.as_str()));
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = Command::new(env!("CARGO_BIN_EXE_jeryu-toolctl"))
            .args(["--tool-root", root.to_str().expect("UTF-8 test root")])
            .arg(&name)
            .output()
            .expect("run jeryu-toolctl");
        prop_assert!(!output.status.success());
        prop_assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        let expected = format!("unrecognized subcommand '{name}'");
        prop_assert!(stderr.contains(&expected));
        prop_assert!(stderr.contains("run jeryu-toolctl --help for the accepted commands"));
    }
}
