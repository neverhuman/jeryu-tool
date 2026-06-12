#!/usr/bin/env bash
# Propagate the jankurai pin from tool-manifest.toml into every family consumer.
#
#   ops/render-tool-manifest.sh           # rewrite consumers in place
#   ops/render-tool-manifest.sh --check   # CI drift lane: exit 1 if any consumer drifted
#
# Thin wrapper around render_tool_manifest.py (Python does the parsing + idempotent
# regex rewrites; tomllib ships with python3.11+ on the family hosts/images).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "${here}/render_tool_manifest.py" "$@"
