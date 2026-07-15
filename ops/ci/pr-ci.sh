#!/usr/bin/env bash
# Canonical release-authoritative local PR gate for jeryu-tool. split-host-ci
# posts `jeryu-tool/required` from this script. The GitHub workflow is an
# explicitly non-authoritative static mirror because it cannot reach local Jeryu.
set -euo pipefail

# BEGIN GENERATED JANKURAI PIN — DO NOT EDIT
export JERYU_GOVERNED_JANKURAI_BIN="${JERYU_JANKURAI_BIN:-/home/ubuntu/.jeryu/bin/jankurai}"
export JERYU_JANKURAI_SOURCE_REPO="http://127.0.0.1:8787/git/jeryu/jankurai.git"
export JERYU_JANKURAI_VERSION="jankurai 1.6.11"
export JERYU_JANKURAI_SHA256="fdb42e5fa7d9851c0729e59bf1e582c895aa9cfc03a7175b420c6025d2fd014e"
export JERYU_JANKURAI_SOURCE_REV="dface7397fe24d46b0b1885ddd5782c34edbff49"
export JERYU_JANKURAI_SOURCE_TAG="v1.6.11-deadlang-precision-split.1"
export JERYU_JANKURAI_SOURCE_TREE="34a8a1fb59bc4ebfadf12c45d95f169d06acc781"
export JERYU_JANKURAI_SOURCE_ARCHIVE_SHA256="2fbca5d04083e3c8d32f383d5b6b4520b8911690b26968c6fbcb210e1202b938"
export JERYU_JANKURAI_CARGO_LOCK_SHA256="b9acb981c326226a687d0b6703e4f7ee303148e9e1a6dda1aa03d77988820f6a"
export JERYU_JANKURAI_RUST_TOOLCHAIN="1.95.0"
export JERYU_JANKURAI_RUSTC_VERSION="rustc 1.95.0 (59807616e 2026-04-14)"
export JERYU_JANKURAI_CARGO_VERSION="cargo 1.95.0 (f2d3ce0bd 2026-03-21)"
export JERYU_JANKURAI_TARGET_TRIPLE="x86_64-unknown-linux-gnu"
export JERYU_JANKURAI_BUILD_MODE="cargo-install-locked-offline-path-v1"
# END GENERATED JANKURAI PIN

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# A premerge manifest PR may qualify its exact pinned candidate in /tmp, with a
# content-addressed diagnostic receipt, without mutating the governed host path.
# Once the reviewed candidate has been installed, the exact-head required run
# automatically selects /home/ubuntu/.jeryu and requires a production receipt.
unset JERYU_GOVERNED_JANKURAI_BIN JERYU_JANKURAI_BIN JERYU_JANKURAI_RECEIPT \
  JERYU_JANKURAI_RECEIPT_SHA256 JERYU_JANKURAI_ALLOW_TEST_RECEIPT
host_bin="/home/ubuntu/.jeryu/bin/jankurai"
host_version=""
host_sha=""
if [[ -f "${host_bin}" && ! -L "${host_bin}" ]]; then
  host_version="$("${host_bin}" --version 2>/dev/null || true)"
  host_sha="$(sha256sum "${host_bin}" 2>/dev/null | awk '{print $1}' || true)"
fi
candidate_root=""
if [[ "${host_version}" == "${JERYU_JANKURAI_VERSION}" &&
      "${host_sha}" == "${JERYU_JANKURAI_SHA256}" ]]; then
  export JERYU_JANKURAI_BIN="${host_bin}"
  source ops/ci/lib.sh
  require_jankurai
  qualification_mode="governed-host"
else
  if [[ "${JERYU_TOOL_REQUIRE_GOVERNED_HOST:-0}" == "1" ]]; then
    printf 'governed-host Jankurai required: version=%s sha256=%s\n' \
      "${host_version:-missing}" "${host_sha:-missing}" >&2
    exit 1
  fi
  candidate_root="$(mktemp -d /tmp/jeryu-tool-premerge-candidate.XXXXXX)"
  trap 'rm -rf "${candidate_root}"' EXIT
  evidence_dir="${repo_root}/target/jankurai/premerge-candidate"
  rm -rf "${evidence_dir}"
  mkdir -p "${evidence_dir}"
  "${repo_root}/ops/qualify-jankurai-candidate.sh" "${candidate_root}" "${evidence_dir}"
  mapfile -t candidate_envs < <(find "${evidence_dir}" -maxdepth 1 -type f -name '*.env' -print)
  [[ "${#candidate_envs[@]}" -eq 1 ]] || {
    printf 'expected exactly one candidate qualification environment\n' >&2
    exit 1
  }
  # shellcheck source=/dev/null
  source "${candidate_envs[0]}"
  source ops/ci/lib.sh
  require_jankurai
  qualification_mode="premerge-candidate"
fi
printf '[pr-ci] jankurai mode=%s receipt=%s receipt_sha256=%s\n' \
  "${qualification_mode}" "${JERYU_JANKURAI_RECEIPT}" \
  "${JERYU_JANKURAI_RECEIPT_SHA256}" >&2

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
