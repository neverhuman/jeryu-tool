use crate::pin::{
    PIN_ENV_FIELDS, PIN_MARKER_BEGIN, PIN_MARKER_END, Pin, WORKFLOW_PIN_MARKER_BEGIN,
    WORKFLOW_PIN_MARKER_END,
};
use regex::{Captures, Regex};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const MANIFEST_REPO: &str = "http://127.0.0.1:8787/git/jeryu/jeryu-tool.git";
const SANDBOX_JANKURAI_PATH: &str = "/opt/rust/cargo/bin/jankurai";
const SANDBOX_RECEIPT_ROOT: &str = "/opt/jeryu/receipts/jankurai/sha256";

#[derive(Debug, Clone)]
pub(crate) struct ManifestAuthority {
    pub commit: String,
    pub tree: String,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderContext {
    pub authority: ManifestAuthority,
    pub image_receipt_sha256: String,
}

pub(crate) fn sandbox_receipt(pin: &Pin, authority: &ManifestAuthority) -> Result<String, String> {
    let receipt = serde_json::json!({
        "binary": {
            "sha256": pin.get("binary_sha256"),
            "version_output": pin.get("version"),
        },
        "build": {
            "builder_image": pin.get("builder_image"),
            "builder_image_id": pin.get("builder_image_id"),
            "capabilities_dropped": true,
            "cargo": pin.get("cargo_version"),
            "cargo_config_sha256": pin.get("cargo_config_sha256"),
            "cargo_net_offline": true,
            "closed_vendor": true,
            "command": pin.get("build_command"),
            "container_engine_path": "/usr/bin/docker",
            "context_sha256": pin.get("build_context_sha256"),
            "environment": pin.get("build_environment"),
            "git_global_config_disabled": true,
            "git_http_follow_redirects": false,
            "git_system_config_disabled": true,
            "git_terminal_prompt": false,
            "glibc": pin.get("glibc_version"),
            "jankurai_update_check": false,
            "linker": pin.get("linker_version"),
            "mode": pin.get("build_mode"),
            "network_none": true,
            "network_scope": "local-forge-source-plus-closed-vendor-network-none",
            "no_new_privileges": true,
            "no_proxy": "127.0.0.1,localhost,::1",
            "non_root": true,
            "package_path": pin.get("package_path"),
            "read_only_root": true,
            "rustc": pin.get("rustc_version"),
            "rustflags": pin.get("rustflags"),
            "target_triple": pin.get("target_triple"),
            "vendor_file_count": pin.get("vendor_file_count"),
            "vendor_files_sha256": pin.get("vendor_files_sha256"),
        },
        "conclusion": "success",
        "governance": {
            "manifest_commit": authority.commit,
            "manifest_repo": MANIFEST_REPO,
            "manifest_sha256": authority.sha256,
            "manifest_tree": authority.tree,
            "protected_main": true,
            "protection_policy": "immutable-main-v1",
            "status": "governed",
        },
        "installation": {
            "atomic": true,
            "path": SANDBOX_JANKURAI_PATH,
        },
        "operator": "jeryu-agent-sandbox-build",
        "run_id": format!("agent-sandbox-jankurai-{}", pin.get("semver")),
        "schema": "jeryu.jankurai-installation/v2",
        "source": {
            "archive_sha256": pin.get("source_archive_sha256"),
            "cargo_lock_sha256": pin.get("cargo_lock_sha256"),
            "commit": pin.get("rev"),
            "remote": pin.get("repo"),
            "tag": pin.get("tag"),
            "tree": pin.get("source_tree"),
            "verification": "release-authoritative",
        },
        "test_mode": false,
    });
    serde_json::to_string_pretty(&receipt)
        .map(|text| format!("{text}\n"))
        .map_err(|error| format!("failed to render sandbox Jankurai receipt: {error}"))
}

pub(crate) fn require_function(tool_root: &Path) -> Result<String, String> {
    let path = tool_root.join("ops/render-assets/require-jankurai.sh");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if !text.starts_with("require_jankurai() {\n") || !text.ends_with("}\n") {
        return Err(format!(
            "invalid governed Jankurai function template: {}",
            path.display()
        ));
    }
    Ok(text.trim_end().to_owned())
}

pub(crate) fn ensure_script(pin: &Pin, function: &str) -> String {
    format!(
        concat!(
            "#!/usr/bin/env bash\n",
            "# GENERATED Jankurai verifier. Installation is owned only by jeryu-tool.\n",
            "set -euo pipefail\n\n",
            "{}\n\n",
            "{function}\n\n",
            "require_jankurai\n",
            "printf 'governed jankurai ok: %s sha256=%s at %s\\n' \\\n",
            "  \"${{JERYU_JANKURAI_VERSION}}\" \"${{JERYU_JANKURAI_SHA256}}\" ",
            "\"${{JERYU_GOVERNED_JANKURAI_BIN}}\"\n",
        ),
        pin.shell_block(),
        function = function,
    )
}

pub(crate) fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("constant renderer regex")
}

fn replace_pin_block(text: &str, pin: &Pin) -> Result<String, String> {
    let begin_pattern = regex(&format!(r"(?m)^{}$", regex::escape(PIN_MARKER_BEGIN)));
    let end_pattern = regex(&format!(r"(?m)^{}$", regex::escape(PIN_MARKER_END)));
    let begin_matches: Vec<_> = begin_pattern.find_iter(text).collect();
    let end_matches: Vec<_> = end_pattern.find_iter(text).collect();
    match (begin_matches.as_slice(), end_matches.as_slice()) {
        ([], []) => {
            let set_pattern = regex(r"(?m)^set -euo pipefail[ \t]*$");
            let set_matches: Vec<_> = set_pattern.find_iter(text).collect();
            if set_matches.is_empty() {
                return Ok(text.to_owned());
            }
            if set_matches.len() != 1 {
                return Err(format!(
                    "unmarked Jankurai pin consumer must contain exactly one \
`set -euo pipefail`; found {}",
                    set_matches.len()
                ));
            }
            let insertion = set_matches[0].end();
            Ok(format!(
                "{}\n\n{}{}",
                &text[..insertion],
                pin.shell_block(),
                &text[insertion..]
            ))
        }
        ([begin], [end]) if begin.start() < end.start() => {
            let mut rendered = String::with_capacity(text.len() + pin.shell_block().len());
            rendered.push_str(&text[..begin.start()]);
            rendered.push_str(&pin.shell_block());
            rendered.push_str(&text[end.end()..]);
            Ok(rendered)
        }
        _ => Err(format!(
            "Jankurai pin consumer has malformed generated markers: begin={} end={}",
            begin_matches.len(),
            end_matches.len()
        )),
    }
}

fn replace_require_function(text: &str, function: &str) -> String {
    regex(r"(?ms)^require_jankurai\(\) \{.*?^\}")
        .replace(text, |_: &Captures<'_>| function.to_owned())
        .into_owned()
}

fn replace_workflow_pin(text: &str, pin: &Pin) -> String {
    let Some(start) = text.find("env:\n") else {
        return text.to_owned();
    };
    if start > 0 && !text[..start].ends_with('\n') {
        return text.to_owned();
    }
    let body_start = start + "env:\n".len();
    let mut body_end = body_start;
    for line in text[body_start..].split_inclusive('\n') {
        if !line.starts_with("  ") {
            break;
        }
        body_end += line.len();
    }
    let known: BTreeSet<&str> = PIN_ENV_FIELDS.iter().map(|(name, _)| *name).collect();
    let mut retained: Vec<String> = Vec::new();
    let mut generated = false;
    for line in text[body_start..body_end].lines() {
        let stripped = line.trim();
        if stripped == WORKFLOW_PIN_MARKER_BEGIN {
            generated = true;
            continue;
        }
        if stripped == WORKFLOW_PIN_MARKER_END {
            generated = false;
            continue;
        }
        if generated {
            continue;
        }
        let key = stripped.split_once(':').map_or(stripped, |(key, _)| key);
        if !known.contains(key) {
            retained.push(line.to_owned());
        }
    }
    retained.push(pin.workflow_block());
    format!(
        "{}{}\n{}",
        &text[..body_start],
        retained.join("\n"),
        &text[body_end..]
    )
}

pub(crate) fn semantic_identity_rules(text: &str, pin: &Pin) -> String {
    let text = regex(
        r"(?:https://github\.com/neverhuman/jankurai\.git|http://127\.0\.0\.1:8787/git/jeryu/jankurai\.git)",
    )
    .replace_all(text, pin.get("repo"));
    let text = regex(r"\bv\d+\.\d+\.\d+-deadlang-precision(?:-split\.\d+)?\b")
        .replace_all(&text, pin.get("tag"));
    let text = regex(r"\bjankurai \d+\.\d+\.\d+\b").replace_all(&text, pin.get("version"));
    regex(r"\bJankurai \d+\.\d+\.\d+\b")
        .replace_all(&text, pin.get("version").replace("jankurai", "Jankurai"))
        .into_owned()
}

fn replace_native_tool_block(text: &str, pin: &Pin) -> String {
    let marker = "[[tools]]";
    let mut output = String::new();
    let mut offset = 0;
    while let Some(relative) = text[offset..].find(marker) {
        let start = offset + relative;
        output.push_str(&text[offset..start]);
        let next = text[start + marker.len()..]
            .find(&format!("\n{marker}"))
            .map_or(text.len(), |value| start + marker.len() + value);
        let block = &text[start..next];
        if regex(r#"(?m)^id\s*=\s*"jankurai"\s*$"#).is_match(block) {
            let block = regex(r#"(version\s*=\s*")[^"]*(")"#)
                .replace_all(block, format!("${{1}}{}${{2}}", pin.get("semver")));
            let block = regex(r#"(observed_path\s*=\s*")[^"]*(")"#)
                .replace_all(&block, "${1}/home/ubuntu/.jeryu/bin/jankurai${2}");
            output.push_str(&regex(r#"(observed_digest\s*=\s*")[^"]*(")"#).replace_all(
                &block,
                format!("${{1}}sha256:{}${{2}}", pin.get("binary_sha256")),
            ));
        } else {
            output.push_str(block);
        }
        offset = next;
    }
    output.push_str(&text[offset..]);
    output
}

fn remove_install_step(text: &str) -> String {
    let Some(start) = text.find("      - name: Install pinned jankurai\n") else {
        return text.to_owned();
    };
    let remaining = &text[start + 1..];
    let next = remaining
        .find("\n      - name:")
        .or_else(|| remaining.find("\n      #"))
        .map_or(text.len(), |value| start + 1 + value + 1);
    format!(
        "{}      - name: Verify governed jankurai\n        run: |\n          source \
ops/ci/lib.sh\n          require_jankurai\n{}",
        &text[..start],
        &text[next..]
    )
}

fn replace_rust_string_constant(text: &str, name: &str, value: &str) -> String {
    let pattern = regex(&format!(
        r#"(?ms)(const {}: &str\s*=\s*)"[^"]*"(\s*;)"#,
        regex::escape(name)
    ));
    let encoded = serde_json::to_string(value).expect("validated identity string");
    pattern
        .replace_all(text, |captures: &Captures<'_>| {
            format!("{}{}{}", &captures[1], encoded, &captures[2])
        })
        .into_owned()
}

fn replace_hex_on_marked_line(text: &str, marker: &str, width: usize, value: &str) -> String {
    let line = regex(&format!(r"(?m)^.*{}.*$", regex::escape(marker)));
    let digest = regex(&format!(r"[0-9a-f]{{{width}}}"));
    line.replace_all(text, |captures: &Captures<'_>| {
        digest.replace(&captures[0], value).into_owned()
    })
    .into_owned()
}

fn replace_receipt_path(text: &str, receipt_sha256: &str) -> String {
    regex(r"/opt/jeryu/receipts/jankurai/sha256/[0-9a-f]{64}\.json")
        .replace_all(
            text,
            format!("{SANDBOX_RECEIPT_ROOT}/{receipt_sha256}.json"),
        )
        .into_owned()
}

fn replace_ci_bridge_constants(text: &str, pin: &Pin, context: &RenderContext) -> String {
    let mut rendered = text.to_owned();
    for (name, key) in [
        ("GOVERNED_JANKURAI_VERSION", "version"),
        ("GOVERNED_JANKURAI_SHA256", "binary_sha256"),
        ("GOVERNED_JANKURAI_SOURCE_REPO", "repo"),
        ("GOVERNED_JANKURAI_SOURCE_TAG", "tag"),
        ("GOVERNED_JANKURAI_SOURCE_REV", "rev"),
        ("GOVERNED_JANKURAI_SOURCE_TREE", "source_tree"),
        (
            "GOVERNED_JANKURAI_SOURCE_ARCHIVE_SHA256",
            "source_archive_sha256",
        ),
        ("GOVERNED_JANKURAI_CARGO_LOCK_SHA256", "cargo_lock_sha256"),
        ("GOVERNED_JANKURAI_RUSTC_VERSION", "rustc_version"),
        ("GOVERNED_JANKURAI_CARGO_VERSION", "cargo_version"),
        ("GOVERNED_JANKURAI_TARGET_TRIPLE", "target_triple"),
        ("GOVERNED_JANKURAI_BUILD_MODE", "build_mode"),
    ] {
        rendered = replace_rust_string_constant(&rendered, name, pin.get(key));
    }
    for (name, value) in [
        (
            "GOVERNED_JANKURAI_MANIFEST_COMMIT",
            context.authority.commit.as_str(),
        ),
        (
            "GOVERNED_JANKURAI_MANIFEST_TREE",
            context.authority.tree.as_str(),
        ),
        (
            "GOVERNED_JANKURAI_MANIFEST_SHA256",
            context.authority.sha256.as_str(),
        ),
    ] {
        rendered = replace_rust_string_constant(&rendered, name, value);
    }
    rendered
}

fn replace_governance_test_constants(text: &str, pin: &Pin, context: &RenderContext) -> String {
    let mut rendered = text.to_owned();
    for (name, value) in [
        ("TAG", pin.get("tag")),
        ("REV", pin.get("rev")),
        ("TREE", pin.get("source_tree")),
        ("ARCHIVE_SHA256", pin.get("source_archive_sha256")),
        ("BINARY_SHA256", pin.get("binary_sha256")),
        ("MANIFEST_COMMIT", context.authority.commit.as_str()),
        ("MANIFEST_TREE", context.authority.tree.as_str()),
        ("MANIFEST_SHA256", context.authority.sha256.as_str()),
        (
            "IMAGE_RECEIPT_SHA256",
            context.image_receipt_sha256.as_str(),
        ),
    ] {
        rendered = replace_rust_string_constant(&rendered, name, value);
    }
    rendered
}

pub(crate) fn render_consumer(
    path: &Path,
    pin: &Pin,
    function: &str,
    context: &RenderContext,
) -> Result<String, String> {
    let mut text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let rel = path.to_string_lossy();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if name == "jankurai-installation-receipt.json" && rel.contains("agent-sandbox") {
        return sandbox_receipt(pin, &context.authority);
    }
    if name == "test-governed-jankurai.sh" {
        text = replace_hex_on_marked_line(
            &text,
            ".governance.manifest_commit",
            40,
            &context.authority.commit,
        );
        text = replace_hex_on_marked_line(
            &text,
            ".governance.manifest_tree",
            40,
            &context.authority.tree,
        );
        return Ok(replace_hex_on_marked_line(
            &text,
            ".governance.manifest_sha256",
            64,
            &context.authority.sha256,
        ));
    }
    if name == "ensure-jankurai.sh" {
        return Ok(ensure_script(pin, function));
    }
    if name == "lib.sh" && rel.contains("/ops/ci/") {
        return Ok(semantic_identity_rules(
            &replace_require_function(&replace_pin_block(&text, pin)?, function),
            pin,
        ));
    }
    if path.extension().and_then(|value| value.to_str()) == Some("sh") {
        text = replace_pin_block(&text, pin)?;
    }
    if matches!(name, "audit-policy.toml" | "default-audit-policy.toml") {
        return Ok(regex(r#"(required_tool_version\s*=\s*")[^"]*(")"#)
            .replace_all(&text, format!("${{1}}{}${{2}}", pin.get("semver")))
            .into_owned());
    }
    if name == "ci-lanes.toml" {
        return Ok(regex(r#"(jankurai_version\s*=\s*")[^"]*(")"#)
            .replace_all(&text, format!("${{1}}{}${{2}}", pin.get("version")))
            .into_owned());
    }
    if name == "native-cli-manifest.toml" {
        text = replace_native_tool_block(&text, pin);
    }
    if name == "ci_bridge.rs" {
        text = regex(r#"(required_tool_version\s*=\s*\\?")[^"\\]*(\\?")"#)
            .replace_all(&text, format!("${{1}}{}${{2}}", pin.get("semver")))
            .into_owned();
        text = replace_ci_bridge_constants(&text, pin, context);
    }
    if name == "jankurai_governance.rs" {
        return Ok(replace_governance_test_constants(&text, pin, context));
    }
    if name == "release.md" {
        text = regex(r"(?ms)(match SHA-256\s*`)[0-9a-f]{64}(`)")
            .replace_all(&text, format!("${{1}}{}${{2}}", pin.get("binary_sha256")))
            .into_owned();
    }
    if name.ends_with(".yml") && rel.contains("/workflows/") && text.contains("JANKURAI_") {
        text = replace_workflow_pin(&text, pin);
        text = regex(
            r"(?m)^      - name: Cache pinned auditor\n        uses:.*\n        with:\n          path:.*\n          key:.*\n",
        )
        .replace_all(&text, "")
        .into_owned();
        text = remove_install_step(&text);
    }
    if name == "Dockerfile" && rel.contains("agent-sandbox") {
        for (key, manifest_key) in PIN_ENV_FIELDS {
            let value = serde_json::to_string(pin.get(manifest_key)).expect("string JSON");
            let argument = regex(&format!(r"(?m)^ARG {}=.*$", regex::escape(key)));
            if argument.is_match(&text) {
                text = argument
                    .replace_all(&text, format!("ARG {key}={value}"))
                    .into_owned();
            } else {
                text = regex(r"(?m)^(ARG JANKURAI_VERSION=.*)$")
                    .replacen(&text, 1, |capture: &Captures<'_>| {
                        format!("{}\nARG {key}={value}", &capture[1])
                    })
                    .into_owned();
            }
        }
    }
    if name == "README.md" && rel.contains("agent-sandbox") {
        text = regex(r"\(\*\*\d+\.\d+\.\d+\*\*, (?:rev|identity)-locked\)")
            .replace_all(
                &text,
                format!("(**{}**, identity-locked)", pin.get("semver")),
            )
            .into_owned();
    }
    if name == "smoke.sh" && rel.contains("/agent-sandbox/") {
        text = regex(r"pinned \d+\.\d+\.\d+")
            .replace_all(&text, format!("pinned {}", pin.get("semver")))
            .into_owned();
        text = replace_hex_on_marked_line(
            &text,
            "sha256sum /opt/rust/cargo/bin/jankurai",
            64,
            pin.get("binary_sha256"),
        );
    }
    if name == "pr-ci.sh" {
        text = regex(r"(?ms)^# The pinned jankurai .*?^export PATH=.*?\n")
            .replace_all(&text, |_: &Captures<'_>| {
                "# Resolve and verify the absolute governed auditor before any lane runs.\n\
source ops/ci/lib.sh\nrequire_jankurai\n"
                    .to_owned()
            })
            .into_owned();
        text = regex(r#"(?m)^export PATH="\$\{CARGO_HOME:-\$HOME/\.cargo\}/bin:\$PATH"\n?"#)
            .replace_all(&text, "")
            .into_owned();
        if !regex(r"(?m)^\s*require_jankurai(?:\s|$)").is_match(&text) {
            text = text.replacen(
                "cd \"$repo_root\"\n",
                "cd \"$repo_root\"\n\n# Resolve and verify the absolute governed auditor before any lane runs.\n\
source ops/ci/lib.sh\nrequire_jankurai\n",
                1,
            );
        }
        text = text.replace(
            "  bash \"$JERYU_TOOL_RENDER\" --check\n",
            "  consumer_repo=\"$(awk -F'\\\"' '/^workspace =/ {print $2; exit}' agent/audit-policy.toml)\"\n\
  bash \"$JERYU_TOOL_RENDER\" --check --repo \"$consumer_repo\" \\\n\
    --repo-root \"$consumer_repo=$repo_root\"\n",
        );
        if !text.contains("JERYU_TOOL_RENDER=") {
            let block = "# Verify only this PR worktree against the family manifest; the post-rollout\n\
# control-plane check verifies the complete canonical family.\n\
JERYU_TOOL_RENDER=\"${JERYU_TOOL_RENDER:-$repo_root/../jeryu-tool/ops/render-tool-manifest.sh}\"\n\
if [ -x \"$JERYU_TOOL_RENDER\" ]; then\n\
  consumer_repo=\"$(awk -F'\\\"' '/^workspace =/ {print $2; exit}' agent/audit-policy.toml)\"\n\
  bash \"$JERYU_TOOL_RENDER\" --check --repo \"$consumer_repo\" \\\n\
    --repo-root \"$consumer_repo=$repo_root\"\n\
fi\n\n";
            text = text.replacen(
                "echo \"[pr-ci] (jobs=$JOBS) standard lanes\" >&2\n",
                &format!("{block}echo \"[pr-ci] (jobs=$JOBS) standard lanes\" >&2\n"),
                1,
            );
        }
    }
    if name == "ci-doctor.sh" && !text.contains("require_jankurai") {
        text = text.replacen(
            &format!("{PIN_MARKER_END}\n"),
            &format!(
                "{PIN_MARKER_END}\n\nsource \"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")/..\" \
&& pwd)/ops/ci/lib.sh\"\nrequire_jankurai\n"
            ),
            1,
        );
    }
    text = replace_receipt_path(&text, &context.image_receipt_sha256);
    Ok(semantic_identity_rules(&text, pin))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_context() -> RenderContext {
        RenderContext {
            authority: ManifestAuthority {
                commit: "a".repeat(40),
                tree: "b".repeat(40),
                sha256: "c".repeat(64),
            },
            image_receipt_sha256: "d".repeat(64),
        }
    }

    #[test]
    fn sandbox_receipt_is_v2_path_and_authority_bound() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pin = Pin::load(&root).expect("canonical pin");
        let context = fixture_context();
        let text = sandbox_receipt(&pin, &context.authority).expect("receipt");
        let receipt: serde_json::Value = serde_json::from_str(&text).expect("receipt JSON");
        assert_eq!(receipt["schema"], "jeryu.jankurai-installation/v2");
        assert_eq!(receipt["binary"]["sha256"], pin.get("binary_sha256"));
        assert_eq!(receipt["build"]["mode"], pin.get("build_mode"));
        assert_eq!(receipt["installation"]["path"], SANDBOX_JANKURAI_PATH);
        assert_eq!(receipt["governance"]["manifest_commit"], "a".repeat(40));
        assert_eq!(receipt["test_mode"], false);
    }

    #[test]
    fn targeted_constants_and_receipt_paths_are_replaced() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pin = Pin::load(&root).expect("canonical pin");
        let context = fixture_context();
        let rust = "const TAG: &str = \"old\";\nconst IMAGE_RECEIPT_SHA256: &str =\n    \"old\";\n";
        let rendered = replace_governance_test_constants(rust, &pin, &context);
        assert!(rendered.contains(pin.get("tag")));
        assert!(rendered.contains(&context.image_receipt_sha256));

        let path = format!("{SANDBOX_RECEIPT_ROOT}/{}.json", "0".repeat(64));
        assert_eq!(
            replace_receipt_path(&path, &context.image_receipt_sha256),
            format!(
                "{SANDBOX_RECEIPT_ROOT}/{}.json",
                context.image_receipt_sha256
            )
        );
    }

    #[test]
    fn pin_block_replacement_is_in_place_and_idempotent() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pin = Pin::load(&root).expect("canonical pin");
        let stale = pin
            .shell_block()
            .replace(pin.get("version"), "jankurai 0.0.0");
        let input = format!(
            "#!/usr/bin/env bash\nsource before-generated.sh\n\n{stale}\n\necho after-generated\n"
        );

        let once = replace_pin_block(&input, &pin).expect("first render");
        let twice = replace_pin_block(&once, &pin).expect("second render");
        assert_eq!(once, twice);
        assert_eq!(once.matches(PIN_MARKER_BEGIN).count(), 1);
        assert_eq!(once.matches(PIN_MARKER_END).count(), 1);
        assert!(once.contains("source before-generated.sh\n\n"));
        assert!(once.ends_with("\n\necho after-generated\n"));
        assert!(
            once.find("source before-generated.sh").unwrap() < once.find(PIN_MARKER_BEGIN).unwrap()
        );
        assert!(once.find(PIN_MARKER_END).unwrap() < once.find("echo after-generated").unwrap());

        let unmarked = "#!/usr/bin/env bash\nset -euo pipefail\necho retained\n";
        let inserted = replace_pin_block(unmarked, &pin).expect("insert missing block");
        assert!(inserted.contains(&format!("set -euo pipefail\n\n{}", pin.shell_block())));
        assert!(inserted.ends_with("\necho retained\n"));
        assert_eq!(
            replace_pin_block(&inserted, &pin).expect("render inserted block again"),
            inserted
        );

        let unowned = "#!/bin/sh\necho no-pin-consumer\n";
        assert_eq!(
            replace_pin_block(unowned, &pin).expect("leave unowned shell unchanged"),
            unowned
        );
    }

    #[test]
    fn pin_block_replacement_rejects_ambiguous_or_malformed_shell() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pin = Pin::load(&root).expect("canonical pin");
        let block = pin.shell_block();
        let malformed = [
            format!("#!/bin/sh\nset -euo pipefail\n{PIN_MARKER_BEGIN}\n"),
            format!("#!/bin/sh\nset -euo pipefail\n{PIN_MARKER_END}\n{PIN_MARKER_BEGIN}\n"),
            format!("#!/bin/sh\nset -euo pipefail\n{block}\n{block}\n"),
            "#!/bin/sh\nset -euo pipefail\nset -euo pipefail\n".to_owned(),
        ];

        for input in malformed {
            assert!(
                replace_pin_block(&input, &pin).is_err(),
                "accepted {input:?}"
            );
        }
    }
}
