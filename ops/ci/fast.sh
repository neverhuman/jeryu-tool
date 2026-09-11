#!/usr/bin/env bash
# Deterministic fast lane: the pin must always match the manifest.
set -euo pipefail
source ops/ci/lib.sh
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  # Public GitHub origin is neverhuman/jeryu-tool, not git.neverhuman.org.
  # Compare generated pin fields to tool-manifest.toml without renderer custody.
  python3 - <<'PY'
import pathlib, re, sys
root = pathlib.Path(".")
manifest = root.joinpath("tool-manifest.toml").read_text()
lib = root.joinpath("ops/ci/lib.sh").read_text()
def field(name):
    m = re.search(rf'^{re.escape(name)}\s*=\s*"([^"]+)"', manifest, re.M)
    if not m:
        sys.exit(f"missing manifest field {name}")
    return m.group(1)
pairs = {
    "JERYU_JANKURAI_SOURCE_REV": field("rev"),
    "JERYU_JANKURAI_SOURCE_TAG": field("tag"),
    "JERYU_JANKURAI_VERSION": field("version"),
    "JERYU_JANKURAI_SHA256": field("binary_sha256"),
    "JERYU_JANKURAI_SOURCE_TREE": field("source_tree"),
}
for env, expected in pairs.items():
    m = re.search(rf'export {env}="([^"]+)"', lib)
    if not m or m.group(1) != expected:
        sys.exit(f"pin drift: {env} expected {expected!r} got {m.group(1) if m else None!r}")
print("fast ok: generated pin fields match tool-manifest.toml (github-actions field compare)")
PY
  exit 0
fi
bash ops/render-tool-manifest.sh --check --repo jeryu-tool
printf 'fast ok: jeryu-tool consumers match tool-manifest.toml\n'
