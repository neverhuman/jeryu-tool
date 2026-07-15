#!/usr/bin/env bash
# Canonical local PR gate for jeryu-tool. split-host-ci prefers this script and
# posts the `jeryu-tool/required` check-run from its exit status;
# .github/workflows/ci.yml runs the same lanes on the GitHub mirror so the two
# surfaces cannot diverge.
set -euo pipefail

# BEGIN GENERATED JANKURAI PIN — DO NOT EDIT
export JERYU_GOVERNED_JANKURAI_BIN="${JERYU_JANKURAI_BIN:-/home/ubuntu/.jeryu/bin/jankurai}"
export JERYU_JANKURAI_VERSION="jankurai 1.6.11"
export JERYU_JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"
export JERYU_JANKURAI_SOURCE_REV="dface7397fe24d46b0b1885ddd5782c34edbff49"
export JERYU_JANKURAI_SOURCE_TAG="v1.6.11-deadlang-precision-split.1"
export JERYU_JANKURAI_SOURCE_TREE="34a8a1fb59bc4ebfadf12c45d95f169d06acc781"
export JERYU_JANKURAI_SOURCE_ARCHIVE_SHA256="2fbca5d04083e3c8d32f383d5b6b4520b8911690b26968c6fbcb210e1202b938"
export JERYU_JANKURAI_CARGO_LOCK_SHA256="b9acb981c326226a687d0b6703e4f7ee303148e9e1a6dda1aa03d77988820f6a"
export JERYU_JANKURAI_RUST_TOOLCHAIN="1.95.0"
export JERYU_JANKURAI_TARGET_TRIPLE="x86_64-unknown-linux-gnu"
# END GENERATED JANKURAI PIN

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# Resolve and verify the absolute governed auditor before any lane runs.
source ops/ci/lib.sh
require_jankurai

# The manifest PR proves its own generated consumers first. After each protected
# consumer lands, the release lane runs the unscoped family check over canonical mains.
echo "[pr-ci] jankurai pin drift check (manifest-owner self scope)" >&2
bash ops/render-tool-manifest.sh --check --repo jeryu-tool

echo "[pr-ci] standard lanes" >&2
bash ops/ci/fast.sh
bash ops/ci/check.sh
bash ops/ci/score.sh
bash ops/ci/security.sh
bash ops/ci/artifact_support.sh
echo "[pr-ci] jeryu-tool OK" >&2
