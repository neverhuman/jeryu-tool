#!/usr/bin/env bash
# Canonical local PR gate for jeryu-tool. split-host-ci prefers this script and
# posts the `jeryu-tool/required` check-run from its exit status;
# .github/workflows/ci.yml runs the same lanes on the GitHub mirror so the two
# surfaces cannot diverge.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# The pinned jankurai lives in ~/.cargo/bin; stray builds shadow it elsewhere
# on this host. Resolve the pinned auditor first (see ops/ci/lib.sh).
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

# This repo OWNS tool-manifest.toml: fail fast if any family consumer drifted.
echo "[pr-ci] jankurai pin drift check (manifest owner)" >&2
bash ops/render-tool-manifest.sh --check

echo "[pr-ci] standard lanes" >&2
bash ops/ci/fast.sh
bash ops/ci/check.sh
bash ops/ci/score.sh
bash ops/ci/security.sh
bash ops/ci/artifact_support.sh
echo "[pr-ci] jeryu-tool OK" >&2
