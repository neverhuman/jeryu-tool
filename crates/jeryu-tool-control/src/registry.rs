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
const REQUIRED_TOOL_FIELDS: [&str; 9] = [
    "id",
    "name",
    "kind",
    "status",
    "description",
    "adopting_repos",
    "candidate_repos",
    "loc_saved",
    "loc_saved_estimate",
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
    if data.get("schema_version").and_then(toml::Value::as_str) != Some("1") {
        return Err(fail("schema_version must be \"1\""));
    }
    let tools = data
        .get("tool")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
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
        for field in REQUIRED_TOOL_FIELDS {
            if !table.contains_key(field) {
                return Err(fail(format!("tool {display_id:?} missing {field:?}")));
            }
        }
        let id = string(table, "id", "tool")?;
        if id.is_empty() {
            return Err(fail("every [[tool]] needs a non-empty string id"));
        }
        if !seen_ids.insert(id.to_owned()) {
            return Err(fail(format!("duplicate tool id {id:?}")));
        }
        let kind = string(table, "kind", &format!("tool {id:?}"))?;
        if !VALID_KINDS.contains(&kind) {
            return Err(fail(format!("tool {id:?} has invalid kind {kind:?}")));
        }
        let status = string(table, "status", &format!("tool {id:?}"))?;
        if !VALID_TOOL_STATUS.contains(&status) {
            return Err(fail(format!("tool {id:?} has invalid status {status:?}")));
        }
        if status == "published"
            && table
                .get("source")
                .and_then(toml::Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(fail(format!(
                "published tool {id:?} must declare a non-empty source"
            )));
        }
        let adopting_repos = strings(table, "adopting_repos", &format!("tool {id:?}"))?;
        let candidate_repos = strings(table, "candidate_repos", &format!("tool {id:?}"))?;
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
            "name": string(table, "name", &format!("tool {id:?}"))?,
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
            let id = table
                .get("id")
                .and_then(toml::Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    fail(format!(
                        "{} missing id",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ))
                })?;
            if !task_ids.insert(id.to_owned()) {
                return Err(fail(format!("duplicate task id {id:?}")));
            }
            let status = table
                .get("status")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            if !VALID_TASK_STATUS.contains(&status) {
                return Err(fail(format!("task {id:?} has invalid status {status:?}")));
            }
            let tool_id = table
                .get("tool_id")
                .and_then(toml::Value::as_str)
                .unwrap_or("");
            if !seen_ids.contains(tool_id) {
                return Err(fail(format!(
                    "task {id:?} references unknown tool_id {tool_id:?}"
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
}
