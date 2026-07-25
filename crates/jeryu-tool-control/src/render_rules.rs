use crate::pin::{
    PIN_ENV_FIELDS, PIN_MARKER_BEGIN, PIN_MARKER_END, Pin, WORKFLOW_PIN_MARKER_BEGIN,
    WORKFLOW_PIN_MARKER_END,
};
use regex::{Captures, Regex};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

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

fn replace_pin_block(text: &str, pin: &Pin) -> String {
    let marked = regex(&format!(
        r"(?ms)\n*^{}$.*?^{}$\n*",
        regex::escape(PIN_MARKER_BEGIN),
        regex::escape(PIN_MARKER_END)
    ));
    let stripped = marked.replace_all(text, "\n");
    let set = regex(r"(?m)^set -euo pipefail\s*$");
    let Some(found) = set.find(&stripped) else {
        return stripped.into_owned();
    };
    format!(
        "{}\n\n{}\n\n{}",
        &stripped[..found.end()],
        pin.shell_block(),
        stripped[found.end()..].trim_start()
    )
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

pub(crate) fn render_consumer(path: &Path, pin: &Pin, function: &str) -> Result<String, String> {
    let mut text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let rel = path.to_string_lossy();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if name == "ensure-jankurai.sh" {
        return Ok(ensure_script(pin, function));
    }
    if name == "lib.sh" && rel.contains("/ops/ci/") {
        return Ok(semantic_identity_rules(
            &replace_require_function(&replace_pin_block(&text, pin), function),
            pin,
        ));
    }
    if path.extension().and_then(|value| value.to_str()) == Some("sh") {
        text = replace_pin_block(&text, pin);
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
    Ok(semantic_identity_rules(&text, pin))
}
