#!/usr/bin/env python3
"""Validate tools-registry.toml + tasks/ and emit the golden-box summary.

Two modes:
  registry_summary.py            -> print the summary JSON to stdout
  registry_summary.py --check    -> validate only (used by `just check`)

This is the CLI/finder-side computation of the same numbers the forge serves at
GET /api/v1/tools/registry/summary. Both read tools-registry.toml; keep the
aggregation here in sync with the Rust handler (it is intentionally trivial:
sums and distinct-repo counts).
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python < 3.11
    import tomli as tomllib  # type: ignore

REPO_ROOT = Path(__file__).resolve().parent.parent
REGISTRY = REPO_ROOT / "tools-registry.toml"
TASKS_DIR = REPO_ROOT / "tasks"

VALID_KINDS = {
    "rust-crate",
    "ts-lib",
    "react-component",
    "vite-plugin",
    "shell-lib",
}
VALID_TOOL_STATUS = {"proposed", "building", "published", "deprecated"}
VALID_TASK_STATUS = {"open", "in-progress", "done"}

REQUIRED_TOOL_FIELDS = (
    "id",
    "name",
    "kind",
    "status",
    "description",
    "adopting_repos",
    "candidate_repos",
    "loc_saved",
    "loc_saved_estimate",
)


class RegistryError(SystemExit):
    """Raised with a non-zero exit when the registry is malformed."""


def _fail(message: str) -> "RegistryError":
    return RegistryError(f"tools-registry: {message}")


def _load_registry() -> dict:
    if not REGISTRY.exists():
        raise _fail(f"missing {REGISTRY.name}")
    data = tomllib.loads(REGISTRY.read_text())
    if str(data.get("schema_version", "")) != "1":
        raise _fail("schema_version must be \"1\"")
    return data


def _validate_tool(tool: dict, seen_ids: set[str]) -> None:
    for field in REQUIRED_TOOL_FIELDS:
        if field not in tool:
            raise _fail(f"tool {tool.get('id', '?')!r} missing {field!r}")
    tool_id = tool["id"]
    if not isinstance(tool_id, str) or not tool_id:
        raise _fail("every [[tool]] needs a non-empty string id")
    if tool_id in seen_ids:
        raise _fail(f"duplicate tool id {tool_id!r}")
    seen_ids.add(tool_id)
    if tool["kind"] not in VALID_KINDS:
        raise _fail(f"tool {tool_id!r} has invalid kind {tool['kind']!r}")
    if tool["status"] not in VALID_TOOL_STATUS:
        raise _fail(f"tool {tool_id!r} has invalid status {tool['status']!r}")
    for field in ("adopting_repos", "candidate_repos"):
        if not isinstance(tool[field], list):
            raise _fail(f"tool {tool_id!r} field {field!r} must be a list")
    for field in ("loc_saved", "loc_saved_estimate"):
        value = tool[field]
        if not isinstance(value, int) or value < 0:
            raise _fail(f"tool {tool_id!r} field {field!r} must be a non-negative integer")
    if tool["status"] == "published" and not tool.get("source"):
        raise _fail(f"published tool {tool_id!r} must declare a non-empty source")


def _load_tasks(tool_ids: set[str]) -> list[dict]:
    tasks: list[dict] = []
    seen_task_ids: set[str] = set()
    if not TASKS_DIR.is_dir():
        return tasks
    for path in sorted(TASKS_DIR.glob("*.toml")):
        task = tomllib.loads(path.read_text())
        task_id = task.get("id")
        if not task_id:
            raise _fail(f"{path.name} missing id")
        if task_id in seen_task_ids:
            raise _fail(f"duplicate task id {task_id!r}")
        seen_task_ids.add(task_id)
        status = task.get("status")
        if status not in VALID_TASK_STATUS:
            raise _fail(f"task {task_id!r} has invalid status {status!r}")
        tool_id = task.get("tool_id")
        if tool_id not in tool_ids:
            raise _fail(f"task {task_id!r} references unknown tool_id {tool_id!r}")
        tasks.append(task)
    return tasks


def build_summary() -> dict:
    data = _load_registry()
    tools = data.get("tool", [])
    seen_ids: set[str] = set()
    for tool in tools:
        _validate_tool(tool, seen_ids)
    tasks = _load_tasks(seen_ids)

    adopting_repos: set[str] = set()
    candidate_repos: set[str] = set()
    for tool in tools:
        adopting_repos.update(tool["adopting_repos"])
        candidate_repos.update(tool["candidate_repos"])

    status_counts = {status: 0 for status in VALID_TOOL_STATUS}
    for tool in tools:
        status_counts[tool["status"]] += 1

    open_tasks = sum(1 for task in tasks if task.get("status") in {"open", "in-progress"})

    return {
        "tool_count": len(tools),
        "published_count": status_counts["published"],
        "building_count": status_counts["building"],
        "proposed_count": status_counts["proposed"],
        "deprecated_count": status_counts["deprecated"],
        "adopting_repo_count": len(adopting_repos),
        "candidate_repo_count": len(candidate_repos),
        "open_task_count": open_tasks,
        "realized_loc_saved": sum(tool["loc_saved"] for tool in tools),
        "anticipated_loc_saved": sum(tool["loc_saved_estimate"] for tool in tools),
        "tools": [
            {
                "id": tool["id"],
                "name": tool["name"],
                "kind": tool["kind"],
                "status": tool["status"],
                "adopting_repo_count": len(tool["adopting_repos"]),
                "candidate_repo_count": len(tool["candidate_repos"]),
                "loc_saved": tool["loc_saved"],
                "loc_saved_estimate": tool["loc_saved_estimate"],
            }
            for tool in tools
        ],
    }


def main(argv: list[str]) -> int:
    check_only = "--check" in argv[1:]
    summary = build_summary()
    if check_only:
        print(
            f"registry ok: {summary['tool_count']} tool(s), "
            f"{summary['open_task_count']} open task(s), "
            f"{summary['anticipated_loc_saved']} LOC anticipated, "
            f"{summary['realized_loc_saved']} LOC realized"
        )
    else:
        print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
