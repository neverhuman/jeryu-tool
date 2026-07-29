#!/usr/bin/env bash
# Canonical release-authoritative local PR gate for jeryu-tool. split-host-ci
# posts `jeryu-tool/required` from this script. The GitHub workflow is an
# explicitly non-authoritative static mirror because it cannot reach local Jeryu.
set -euo pipefail

# BEGIN GENERATED JANKURAI PIN — DO NOT EDIT
export JERYU_JANKURAI_SOURCE_REPO="http://127.0.0.1:8787/git/jeryu/jankurai.git"
export JERYU_JANKURAI_VERSION="jankurai 1.6.11"
export JERYU_JANKURAI_SHA256="59f4ec903c75a869de365145432a97bdf574b517ce64ad9613088437a9f37b4b"
export JERYU_JANKURAI_SOURCE_REV="4dfbdfa3585f1928d5f996d7b5e14608dff14a03"
export JERYU_JANKURAI_SOURCE_TAG="v1.6.11-deadlang-precision-split.2"
export JERYU_JANKURAI_SOURCE_TREE="7e5d501aa6f0ee6ced9a48c6288a9943d0b9573c"
export JERYU_JANKURAI_SOURCE_ARCHIVE_SHA256="1aa3d178dec0fbb8d0657dd465ea6fda830ffc4ec1f65560b7b7d1682fd87e69"
export JERYU_JANKURAI_CARGO_LOCK_SHA256="b9acb981c326226a687d0b6703e4f7ee303148e9e1a6dda1aa03d77988820f6a"
export JERYU_JANKURAI_RUST_TOOLCHAIN="1.95.0"
export JERYU_JANKURAI_RUSTC_VERSION="rustc 1.95.0 (59807616e 2026-04-14)"
export JERYU_JANKURAI_CARGO_VERSION="cargo 1.95.0 (f2d3ce0bd 2026-03-21)"
export JERYU_JANKURAI_TARGET_TRIPLE="x86_64-unknown-linux-gnu"
export JERYU_JANKURAI_BUILD_MODE="oci-vendor-locked-offline-workspace-member-v2"
# END GENERATED JANKURAI PIN

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# Release-full CI accepts only the root broker's fixed, PATH-selected auditor.
# A local premerge manifest PR may instead qualify its exact pinned candidate in
# /tmp, with a content-addressed diagnostic receipt, without granting release
# authority or mutating an installed auditor.
unset JERYU_GOVERNED_JANKURAI_BIN JERYU_JANKURAI_BIN JERYU_JANKURAI_RECEIPT \
  JERYU_JANKURAI_RECEIPT_SHA256 JERYU_JANKURAI_ALLOW_TEST_RECEIPT
host_bin="$(command -v jankurai 2>/dev/null || true)"
host_version=""
host_sha=""
if [[ -f "${host_bin}" && ! -L "${host_bin}" ]]; then
  host_version="$("${host_bin}" --version 2>/dev/null || true)"
  host_sha="$(sha256sum "${host_bin}" 2>/dev/null | awk '{print $1}' || true)"
fi
candidate_root=""
if [[ "${JAIN_RELEASE_CI:-0}" == "1" ]]; then
  source ops/ci/lib.sh
  require_jankurai
  qualification_mode="release-broker"
elif [[ "${host_version}" == "${JERYU_JANKURAI_VERSION}" &&
        "${host_sha}" == "${JERYU_JANKURAI_SHA256}" ]]; then
  source ops/ci/lib.sh
  require_jankurai
  qualification_mode="receipt-bound-host"
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
  candidate_bin="${JERYU_JANKURAI_BIN:?candidate qualification did not select Jankurai}"
  candidate_bin_dir="$(dirname "${candidate_bin}")"
  export PATH="${candidate_bin_dir}:${PATH}"
  source ops/ci/lib.sh
  require_jankurai
  qualification_mode="premerge-candidate"
fi
printf '[pr-ci] jankurai mode=%s bin=%s receipt=%s receipt_sha256=%s\n' \
  "${qualification_mode}" "${JERYU_GOVERNED_JANKURAI_BIN}" \
  "${JERYU_JANKURAI_RECEIPT:-not-product-visible}" \
  "${JERYU_JANKURAI_RECEIPT_SHA256:-not-product-visible}" >&2

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
