#!/usr/bin/env bash
# Structure check: the manifest parses, the generated pin is current, and every
# shell/python entrypoint is syntactically valid.
set -euo pipefail
source ops/ci/lib.sh

python3 - <<'PY'
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib
m = tomllib.loads(Path("tool-manifest.toml").read_text())
j = m.get("jankurai", {})
for key in ("repo", "rev", "tag", "version", "semver"):
    if not j.get(key):
        raise SystemExit(f"tool-manifest.toml [jankurai] missing {key!r}")
if not m.get("floors"):
    raise SystemExit("tool-manifest.toml missing [floors]")
PY

bash ops/render-tool-manifest.sh --check

# Reusable-tool registry: parse + validate tools-registry.toml and tasks/.
python3 ops/registry_summary.py --check

python3 -m py_compile ops/render_tool_manifest.py ops/registry_summary.py
for script in ops/*.sh ops/ci/*.sh; do
  [[ -e "$script" ]] || continue
  bash -n "$script"
done
printf 'check ok: %s\n' "$(pwd)"
