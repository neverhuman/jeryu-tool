#!/usr/bin/env bash
# Propagate the jankurai pin from tool-manifest.toml into every family consumer.
#
#   ops/render-tool-manifest.sh --check   # family drift lane; never writes
#   ops/render-tool-manifest.sh --repo NAME --repo-root NAME=/absolute/path \
#     --expected-head NAME=40_HEX_SHA
#                                        # explicit, custody-checked write
#
# Thin wrapper around render_tool_manifest.py (Python does the parsing + idempotent
# regex rewrites; tomllib ships with python3.11+ on the family hosts/images).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "${here}/render_tool_manifest.py" "$@"
