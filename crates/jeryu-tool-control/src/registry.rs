use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const VALID_KINDS: [&str; 6] = [
    "rust-crate",
    "ts-lib",
    "react-component",
    "vite-plugin",
    "shell-lib",
    "jankurai-tool",
];
const VALID_TOOL_STATUS: [&str; 4] = ["proposed", "building", "published", "deprecated"];
const VALID_TASK_STATUS: [&str; 3] = ["open", "in-progress", "done"];
const TOP_LEVEL_FIELDS: [&str; 2] = ["schema_version", "tool"];
const TOOL_FIELDS: [&str; 10] = [
    "id",
    "name",
    "kind",
    "status",
    "source",
    "description",
    "adopting_repos",
    "candidate_repos",
    "loc_saved",
    "loc_saved_estimate",
];
const DISCOVERED_TOOL_FIELDS: [&str; 11] = [
    "id",
    "name",
    "kind",
    "status",
    "source",
    "description",
    "origin_cluster",
    "adopting_repos",
    "candidate_repos",
    "loc_saved",
    "loc_saved_estimate",
];
const TASK_FIELDS: [&str; 8] = [
    "id",
    "tool_id",
    "title",
    "status",
    "origin_cluster",
    "anticipated_loc_saved",
    "target_repos",
    "rollout",
];

fn fail(message: impl AsRef<str>) -> String {
    format!("tools-registry: {}", message.as_ref())
}

fn string<'a>(table: &'a toml::Table, field: &str, context: &str) -> Result<&'a str, String> {
    table
        .get(field)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| fail(format!("{context} field {field:?} must be a string")))
}

fn strings(table: &toml::Table, field: &str, context: &str) -> Result<Vec<String>, String> {
    let values = table
        .get(field)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| fail(format!("{context} field {field:?} must be a list")))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| fail(format!("{context} field {field:?} must contain strings")))
        })
        .collect()
}

fn integer(table: &toml::Table, field: &str, context: &str) -> Result<u64, String> {
    let value = table
        .get(field)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| {
            fail(format!(
                "{context} field {field:?} must be a non-negative integer"
            ))
        })?;
    u64::try_from(value).map_err(|_| {
        fail(format!(
            "{context} field {field:?} must be a non-negative integer"
        ))
    })
}

fn exact_keys(table: &toml::Table, expected: &[&str], context: &str) -> Result<(), String> {
    let actual: BTreeSet<&str> = table.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = expected.iter().copied().collect();
    if actual != expected {
        let missing: Vec<&str> = expected.difference(&actual).copied().collect();
        let unknown: Vec<&str> = actual.difference(&expected).copied().collect();
        return Err(fail(format!(
            "{context} key set mismatch: missing={missing:?} unknown={unknown:?}"
        )));
    }
    Ok(())
}

fn exact_tool_keys(table: &toml::Table, context: &str) -> Result<(), String> {
    let actual: BTreeSet<&str> = table.keys().map(String::as_str).collect();
    let ordinary: BTreeSet<&str> = TOOL_FIELDS.iter().copied().collect();
    let discovered: BTreeSet<&str> = DISCOVERED_TOOL_FIELDS.iter().copied().collect();
    if actual == ordinary || actual == discovered {
        return Ok(());
    }
    let allowed: BTreeSet<&str> = ordinary.union(&discovered).copied().collect();
    let missing: Vec<&str> = ordinary.difference(&actual).copied().collect();
    let unknown: Vec<&str> = actual.difference(&allowed).copied().collect();
    Err(fail(format!(
        "{context} key set mismatch: missing={missing:?} unknown={unknown:?}"
    )))
}

fn nonempty_text(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(char::is_control)
}

fn valid_slug(value: &str) -> bool {
    Regex::new(r"^[a-z0-9][a-z0-9-]{0,127}$")
        .expect("constant regex")
        .is_match(value)
}

fn valid_origin_cluster(value: &str) -> bool {
    value.strip_prefix("toolbuild-").is_some_and(|digest| {
        digest.len() == 16
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn validate_unique_repos(values: &[String], context: &str) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    for value in values {
        if !valid_slug(value) {
            return Err(fail(format!(
                "{context} contains invalid repository {value:?}"
            )));
        }
        if !unique.insert(value) {
            return Err(fail(format!(
                "{context} contains duplicate repository {value:?}"
            )));
        }
    }
    Ok(())
}

fn load_toml(path: &Path) -> Result<toml::Value, String> {
    let text =
        fs::read_to_string(path).map_err(|error| fail(format!("{}: {error}", path.display())))?;
    toml::from_str(&text).map_err(|error| fail(format!("{}: {error}", path.display())))
}

fn build_summary(root: &Path) -> Result<Value, String> {
    let registry_path = root.join("tools-registry.toml");
    if !registry_path.exists() {
        return Err(fail("missing tools-registry.toml"));
    }
    let data = load_toml(&registry_path)?;
    let top = data
        .as_table()
        .ok_or_else(|| fail("tools-registry.toml must contain a top-level table"))?;
    exact_keys(top, &TOP_LEVEL_FIELDS, "tools-registry.toml top-level")?;
    if top.get("schema_version").and_then(toml::Value::as_str) != Some("1") {
        return Err(fail("schema_version must be \"1\""));
    }
    let tools = top
        .get("tool")
        .and_then(toml::Value::as_array)
        .cloned()
        .ok_or_else(|| fail("tools-registry.toml field \"tool\" must be a list"))?;
    let mut seen_ids = BTreeSet::new();
    let mut adopting = BTreeSet::new();
    let mut candidates = BTreeSet::new();
    let mut counts: BTreeMap<&str, u64> = VALID_TOOL_STATUS
        .into_iter()
        .map(|name| (name, 0))
        .collect();
    let mut rows = Vec::new();
    let mut realized = 0_u64;
    let mut anticipated = 0_u64;

    for value in &tools {
        let table = value
            .as_table()
            .ok_or_else(|| fail("every [[tool]] must be a table"))?;
        let display_id = table.get("id").and_then(toml::Value::as_str).unwrap_or("?");
        exact_tool_keys(table, &format!("tool {display_id:?}"))?;
        let id = string(table, "id", "tool")?;
        if !valid_slug(id) {
            return Err(fail(format!("tool id {id:?} is not a canonical slug")));
        }
        if !seen_ids.insert(id.to_owned()) {
            return Err(fail(format!("duplicate tool id {id:?}")));
        }
        let name = string(table, "name", &format!("tool {id:?}"))?;
        let description = string(table, "description", &format!("tool {id:?}"))?;
        let source = string(table, "source", &format!("tool {id:?}"))?;
        if !nonempty_text(name)
            || !nonempty_text(description)
            || source.chars().any(char::is_control)
        {
            return Err(fail(format!(
                "tool {id:?} has invalid name, description, or source text"
            )));
        }
        let kind = string(table, "kind", &format!("tool {id:?}"))?;
        if !VALID_KINDS.contains(&kind) {
            return Err(fail(format!("tool {id:?} has invalid kind {kind:?}")));
        }
        let status = string(table, "status", &format!("tool {id:?}"))?;
        if !VALID_TOOL_STATUS.contains(&status) {
            return Err(fail(format!("tool {id:?} has invalid status {status:?}")));
        }
        if table.get("origin_cluster").is_some_and(|value| {
            value
                .as_str()
                .is_none_or(|origin| !valid_origin_cluster(origin))
        }) {
            return Err(fail(format!(
                "tool {id:?} origin_cluster must be toolbuild- plus 16 lowercase hex characters"
            )));
        }
        if status == "published" && source.is_empty() {
            return Err(fail(format!(
                "published tool {id:?} must declare a non-empty source"
            )));
        }
        let adopting_repos = strings(table, "adopting_repos", &format!("tool {id:?}"))?;
        let candidate_repos = strings(table, "candidate_repos", &format!("tool {id:?}"))?;
        validate_unique_repos(&adopting_repos, &format!("tool {id:?} adopting_repos"))?;
        validate_unique_repos(&candidate_repos, &format!("tool {id:?} candidate_repos"))?;
        if adopting_repos
            .iter()
            .any(|repo| candidate_repos.contains(repo))
        {
            return Err(fail(format!(
                "tool {id:?} cannot list one repository as both adopting and candidate"
            )));
        }
        adopting.extend(adopting_repos.iter().cloned());
        candidates.extend(candidate_repos.iter().cloned());
        let loc_saved = integer(table, "loc_saved", &format!("tool {id:?}"))?;
        let loc_estimate = integer(table, "loc_saved_estimate", &format!("tool {id:?}"))?;
        realized = realized
            .checked_add(loc_saved)
            .ok_or_else(|| fail("realized LOC sum overflow"))?;
        anticipated = anticipated
            .checked_add(loc_estimate)
            .ok_or_else(|| fail("anticipated LOC sum overflow"))?;
        *counts.get_mut(status).expect("validated status") += 1;
        rows.push(json!({
            "adopting_repo_count": adopting_repos.len(),
            "candidate_repo_count": candidate_repos.len(),
            "id": id,
            "kind": kind,
            "loc_saved": loc_saved,
            "loc_saved_estimate": loc_estimate,
            "name": name,
            "status": status,
        }));
    }

    let tasks_dir = root.join("tasks");
    let mut task_ids = BTreeSet::new();
    let mut open_tasks = 0_u64;
    if tasks_dir.is_dir() {
        let mut paths = fs::read_dir(&tasks_dir)
            .map_err(|error| fail(format!("{}: {error}", tasks_dir.display())))?
            .map(|entry| entry.map(|item| item.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| fail(format!("{}: {error}", tasks_dir.display())))?;
        paths.retain(|path| path.extension().and_then(|value| value.to_str()) == Some("toml"));
        paths.sort();
        for path in paths {
            let task = load_toml(&path)?;
            let table = task
                .as_table()
                .ok_or_else(|| fail(format!("{} must contain a table", path.display())))?;
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            exact_keys(table, &TASK_FIELDS, &format!("task file {filename}"))?;
            let id = string(table, "id", &format!("task file {filename}"))?;
            if id.len() != 4
                || !id.bytes().all(|byte| byte.is_ascii_digit())
                || !filename.starts_with(&format!("{id}-"))
            {
                return Err(fail(format!(
                    "task {id:?} must use a four-digit id matching its filename"
                )));
            }
            if !task_ids.insert(id.to_owned()) {
                return Err(fail(format!("duplicate task id {id:?}")));
            }
            let status = string(table, "status", &format!("task {id:?}"))?;
            if !VALID_TASK_STATUS.contains(&status) {
                return Err(fail(format!("task {id:?} has invalid status {status:?}")));
            }
            let tool_id = string(table, "tool_id", &format!("task {id:?}"))?;
            if !valid_slug(tool_id) {
                return Err(fail(format!("task {id:?} has invalid tool_id")));
            }
            if !seen_ids.contains(tool_id) {
                return Err(fail(format!(
                    "task {id:?} references unknown tool_id {tool_id:?}"
                )));
            }
            for field in ["title", "origin_cluster"] {
                if !nonempty_text(string(table, field, &format!("task {id:?}"))?) {
                    return Err(fail(format!("task {id:?} has invalid {field}")));
                }
            }
            if !valid_origin_cluster(string(table, "origin_cluster", &format!("task {id:?}"))?) {
                return Err(fail(format!(
                    "task {id:?} origin_cluster must be toolbuild- plus 16 lowercase hex characters"
                )));
            }
            integer(table, "anticipated_loc_saved", &format!("task {id:?}"))?;
            let target_repos = strings(table, "target_repos", &format!("task {id:?}"))?;
            validate_unique_repos(&target_repos, &format!("task {id:?} target_repos"))?;
            if target_repos.is_empty() {
                return Err(fail(format!(
                    "task {id:?} must declare at least one target repository"
                )));
            }
            let rollout = strings(table, "rollout", &format!("task {id:?}"))?;
            if rollout.is_empty() || rollout.iter().any(|step| !nonempty_text(step)) {
                return Err(fail(format!(
                    "task {id:?} rollout must contain non-empty text steps"
                )));
            }
            if matches!(status, "open" | "in-progress") {
                open_tasks += 1;
            }
        }
    }

    let mut summary = Map::new();
    summary.insert("anticipated_loc_saved".to_owned(), json!(anticipated));
    summary.insert("adopting_repo_count".to_owned(), json!(adopting.len()));
    summary.insert("building_count".to_owned(), json!(counts["building"]));
    summary.insert("candidate_repo_count".to_owned(), json!(candidates.len()));
    summary.insert("deprecated_count".to_owned(), json!(counts["deprecated"]));
    summary.insert("open_task_count".to_owned(), json!(open_tasks));
    summary.insert("proposed_count".to_owned(), json!(counts["proposed"]));
    summary.insert("published_count".to_owned(), json!(counts["published"]));
    summary.insert("realized_loc_saved".to_owned(), json!(realized));
    summary.insert("tool_count".to_owned(), json!(tools.len()));
    summary.insert("tools".to_owned(), Value::Array(rows));
    Ok(Value::Object(summary))
}

pub fn run(root: &Path, args: &[String]) -> Result<i32, String> {
    if args.iter().any(|arg| arg != "--check") {
        return Err("registry-summary accepts only --check".to_owned());
    }
    let summary = build_summary(root)?;
    if args.iter().any(|arg| arg == "--check") {
        println!(
            "registry ok: {} tool(s), {} open task(s), {} LOC anticipated, {} LOC realized",
            summary["tool_count"],
            summary["open_task_count"],
            summary["anticipated_loc_saved"],
            summary["realized_loc_saved"]
        );
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary)
                .map_err(|error| format!("failed to serialize registry summary: {error}"))?
        );
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "jeryu-tool-registry-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("tasks")).expect("create test root");
        root
    }

    fn copy_canonical(root: &Path) {
        let canonical = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        fs::copy(
            canonical.join("tools-registry.toml"),
            root.join("tools-registry.toml"),
        )
        .expect("copy registry");
        for entry in fs::read_dir(canonical.join("tasks")).expect("read tasks") {
            let entry = entry.expect("task entry");
            fs::copy(entry.path(), root.join("tasks").join(entry.file_name())).expect("copy task");
        }
    }

    #[test]
    fn canonical_registry_is_valid() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let summary = build_summary(&root).expect("canonical registry");
        assert!(summary["tool_count"].as_u64().unwrap_or(0) > 0);
        assert_eq!(
            summary["tools"].as_array().map(Vec::len),
            summary["tool_count"].as_u64().map(|value| value as usize)
        );
    }

    #[test]
    fn registry_and_task_key_sets_are_closed() {
        let unknown_tool = test_root("unknown-tool");
        copy_canonical(&unknown_tool);
        let registry = fs::read_to_string(unknown_tool.join("tools-registry.toml"))
            .expect("read registry fixture")
            .replacen(
                "id = \"tuiwright\"",
                "id = \"tuiwright\"\nunknown_field = \"forbidden\"",
                1,
            );
        fs::write(unknown_tool.join("tools-registry.toml"), registry).expect("write hostile");
        assert!(build_summary(&unknown_tool).is_err());
        fs::remove_dir_all(unknown_tool).expect("remove fixture");

        let unknown_task = test_root("unknown-task");
        copy_canonical(&unknown_task);
        let task_path = unknown_task.join("tasks/0001-jeryu-ci-shell-lib.toml");
        let task = fs::read_to_string(&task_path)
            .expect("read task fixture")
            .replacen(
                "id = \"0001\"",
                "id = \"0001\"\nunknown_field = \"forbidden\"",
                1,
            );
        fs::write(&task_path, task).expect("write hostile task");
        assert!(build_summary(&unknown_task).is_err());
        fs::remove_dir_all(unknown_task).expect("remove fixture");
    }
}
