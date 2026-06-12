#!/usr/bin/env bash
# Deterministic fast lane: the pin must always match the manifest.
set -euo pipefail
source ops/ci/lib.sh
bash ops/render-tool-manifest.sh --check
printf 'fast ok: pin matches tool-manifest.toml\n'
