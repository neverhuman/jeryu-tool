#!/usr/bin/env bash
# Install the governed Jankurai auditor from an immutable local-forge identity.
set -euo pipefail
umask 077

die() {
  printf 'install-jankurai: %s\n' "$*" >&2
  exit 1
}

sha256_file() {
  sha256sum "$1" | awk '{print $1}'
}

require_hex() {
  local name="$1" value="$2" length="$3"
  [[ "${value}" =~ ^[0-9a-f]{${length}}$ ]] || die "invalid ${name}"
}

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
default_pin_env="${here}/../generated/jankurai-pin.env"
test_mode="${JERYU_INSTALL_TEST_MODE:-0}"
pin_env="${JERYU_PIN_ENV:-${default_pin_env}}"
if [[ "${pin_env}" != "${default_pin_env}" && "${test_mode}" != "1" ]]; then
  die "a non-canonical pin file is allowed only in explicit test mode"
fi
[[ -r "${pin_env}" ]] || die "generated pin is missing: ${pin_env}"
# shellcheck source=/dev/null
source "${pin_env}"

required_pin_vars=(
  JANKURAI_REPO JANKURAI_TAG JANKURAI_REV JANKURAI_VERSION JANKURAI_SEMVER
  JANKURAI_SOURCE_TREE JANKURAI_SOURCE_ARCHIVE_SHA256 JANKURAI_CARGO_LOCK_SHA256
  JANKURAI_BINARY_SHA256 JANKURAI_RUST_TOOLCHAIN JANKURAI_RUSTC_VERSION
  JANKURAI_CARGO_VERSION JANKURAI_TARGET_TRIPLE JANKURAI_BUILD_MODE
)
for name in "${required_pin_vars[@]}"; do
  [[ -n "${!name:-}" ]] || die "generated pin is missing ${name}"
done
[[ "${JANKURAI_REPO}" == "http://127.0.0.1:8787/git/jeryu/jankurai.git" ]] ||
  die "unapproved Jankurai source: ${JANKURAI_REPO}"
[[ "${JANKURAI_TAG}" != "v1.6.11-deadlang-precision" ]] ||
  die "burned historical tag is not a release source"
require_hex JANKURAI_REV "${JANKURAI_REV}" 40
require_hex JANKURAI_SOURCE_TREE "${JANKURAI_SOURCE_TREE}" 40
require_hex JANKURAI_SOURCE_ARCHIVE_SHA256 "${JANKURAI_SOURCE_ARCHIVE_SHA256}" 64
require_hex JANKURAI_CARGO_LOCK_SHA256 "${JANKURAI_CARGO_LOCK_SHA256}" 64
require_hex JANKURAI_BINARY_SHA256 "${JANKURAI_BINARY_SHA256}" 64
[[ "${JANKURAI_BUILD_MODE}" == "cargo-install-locked-offline-path-v1" ]] ||
  die "unsupported build mode: ${JANKURAI_BUILD_MODE}"

install_root="${JERYU_INSTALL_ROOT:-/home/ubuntu/.jeryu}"
if [[ "${install_root}" != "/home/ubuntu/.jeryu" && "${test_mode}" != "1" ]]; then
  die "governed installation root must be /home/ubuntu/.jeryu"
fi
[[ "${install_root}" == /* ]] || die "installation root must be absolute"
install_root="${install_root%/}"
install_dir="${install_root}/bin"
target="${install_dir}/jankurai"
receipt_dir="${install_root}/receipts/jankurai/sha256"
rollback_dir="${install_root}/rollback/jankurai"
expected_target="$(realpath -m "${target}")"
[[ "${expected_target}" == "${target}" ]] || die "installation path traverses a symlink: ${target}"
if [[ -L "${target}" ]]; then
  die "governed binary must not be a symlink: ${target}"
fi

export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
export GIT_TERMINAL_PROMPT=0
export JANKURAI_NO_UPDATE_CHECK=1
export CARGO_NET_OFFLINE=true

token_file="${JERYU_FORGE_TOKEN_FILE:-/home/ubuntu/.jeryu/secrets/merge-token}"
[[ -r "${token_file}" ]] || die "local-forge credential is unavailable"
forge_token="$(tr -d '\n' < "${token_file}")"
[[ -n "${forge_token}" ]] || die "local-forge credential is empty"
forge_git() {
  GIT_CONFIG_COUNT=1 \
  GIT_CONFIG_KEY_0=http.extraHeader \
  GIT_CONFIG_VALUE_0="Authorization: Bearer ${forge_token}" \
    git "$@"
}

mkdir -p "${install_dir}" "${receipt_dir}" "${rollback_dir}"

matching_receipt() {
  local receipt expected_test=false expected_verification=release-authoritative
  if [[ "${test_mode}" == "1" ]]; then
    expected_test=true
  fi
  if [[ "${test_mode}" == "1" && -n "${JERYU_INSTALL_TEST_PREBUILT_BINARY:-}" ]]; then
    expected_verification=test-fixture
  fi
  for receipt in "${receipt_dir}"/*.json; do
    [[ -f "${receipt}" ]] || continue
    if jq -e \
      --arg remote "${JANKURAI_REPO}" \
      --arg commit "${JANKURAI_REV}" \
      --arg tag "${JANKURAI_TAG}" \
      --arg tree "${JANKURAI_SOURCE_TREE}" \
      --arg archive "${JANKURAI_SOURCE_ARCHIVE_SHA256}" \
      --arg lock "${JANKURAI_CARGO_LOCK_SHA256}" \
      --arg rustc "${JANKURAI_RUSTC_VERSION}" \
      --arg cargo "${JANKURAI_CARGO_VERSION}" \
      --arg triple "${JANKURAI_TARGET_TRIPLE}" \
      --arg mode "${JANKURAI_BUILD_MODE}" \
      --arg digest "${JANKURAI_BINARY_SHA256}" \
      --arg version "${JANKURAI_VERSION}" \
      --arg path "${target}" \
      --arg verification "${expected_verification}" \
      --argjson test_mode "${expected_test}" \
      '.schema == "jeryu.jankurai-installation/v1" and
       .source.remote == $remote and .source.commit == $commit and .source.tag == $tag and
       .source.tree == $tree and .source.archive_sha256 == $archive and
       .source.cargo_lock_sha256 == $lock and .source.verification == $verification and
       .build.rustc == $rustc and
       .build.cargo == $cargo and .build.target_triple == $triple and
       .build.mode == $mode and .build.cargo_net_offline == true and
       .build.dedicated_cargo_home == true and
       .build.git_global_config_disabled == true and
       .build.git_system_config_disabled == true and
       .build.git_terminal_prompt == false and
       .build.jankurai_update_check == false and
       .binary.sha256 == $digest and
       .binary.version_output == $version and .installation.path == $path and
       .installation.atomic == true and .conclusion == "success" and
       .test_mode == $test_mode' "${receipt}" >/dev/null 2>&1; then
      printf '%s' "${receipt}"
      return 0
    fi
  done
  return 1
}

if [[ -x "${target}" ]]; then
  existing_version="$("${target}" --version 2>/dev/null || true)"
  existing_sha="$(sha256_file "${target}")"
  if [[ "${existing_version}" == "${JANKURAI_VERSION}" &&
        "${existing_sha}" == "${JANKURAI_BINARY_SHA256}" ]]; then
    if receipt="$(matching_receipt)"; then
      receipt_digest="$(basename "${receipt}" .json)"
      [[ "$(sha256_file "${receipt}")" == "${receipt_digest}" ]] ||
        die "content-addressed receipt failed self-verification: ${receipt}"
      printf 'jeryu jankurai already current: %s sha256=%s receipt=%s\n' \
        "${JANKURAI_VERSION}" "${existing_sha}" "${receipt}"
      exit 0
    fi
  fi
fi

scratch="$(mktemp -d /tmp/jeryu-install-jankurai.XXXXXX)"
stage="${install_dir}/.jankurai.stage.$$"
previous_backup=""
previous_sha=""
target_replaced=0
success=0

rollback_target() {
  local restore="${install_dir}/.jankurai.rollback.$$"
  if [[ -n "${previous_backup}" && -f "${previous_backup}" ]]; then
    cp "${previous_backup}" "${restore}"
    chmod 755 "${restore}"
    sync -f "${restore}"
    mv -f "${restore}" "${target}"
  else
    rm -f "${target}"
  fi
  sync -f "${install_dir}"
}

finish() {
  local status=$?
  trap - EXIT
  if [[ "${status}" -ne 0 && "${target_replaced}" -eq 1 && "${success}" -ne 1 ]]; then
    rollback_target || true
  fi
  rm -f "${stage}"
  rm -rf "${scratch}"
  exit "${status}"
}
trap finish EXIT
trap 'exit 130' INT TERM HUP

actual_rustc="$(rustc "+${JANKURAI_RUST_TOOLCHAIN}" --version)"
actual_cargo="$(cargo "+${JANKURAI_RUST_TOOLCHAIN}" --version)"
actual_target="$(rustc "+${JANKURAI_RUST_TOOLCHAIN}" -vV | awk '/^host:/ {print $2}')"
[[ "${actual_rustc}" == "${JANKURAI_RUSTC_VERSION}" ]] ||
  die "rustc mismatch: got ${actual_rustc}, want ${JANKURAI_RUSTC_VERSION}"
[[ "${actual_cargo}" == "${JANKURAI_CARGO_VERSION}" ]] ||
  die "cargo mismatch: got ${actual_cargo}, want ${JANKURAI_CARGO_VERSION}"
[[ "${actual_target}" == "${JANKURAI_TARGET_TRIPLE}" ]] ||
  die "target mismatch: got ${actual_target}, want ${JANKURAI_TARGET_TRIPLE}"

candidate="${scratch}/out/bin/jankurai"
source_verification="release-authoritative"
if [[ "${test_mode}" == "1" && -n "${JERYU_INSTALL_TEST_PREBUILT_BINARY:-}" ]]; then
  [[ -x "${JERYU_INSTALL_TEST_PREBUILT_BINARY}" ]] || die "test binary is not executable"
  mkdir -p "$(dirname "${candidate}")"
  cp "${JERYU_INSTALL_TEST_PREBUILT_BINARY}" "${candidate}"
  source_verification="test-fixture"
else
  remote_tag="$(forge_git ls-remote --tags "${JANKURAI_REPO}" \
    "refs/tags/${JANKURAI_TAG}" "refs/tags/${JANKURAI_TAG}^{}" |
    awk -v direct="refs/tags/${JANKURAI_TAG}" -v peeled="refs/tags/${JANKURAI_TAG}^{}" '
      $2 == peeled { print $1; found = 1; exit }
      $2 == direct { direct_rev = $1 }
      END { if (!found && direct_rev != "") print direct_rev }
    ' | head -n 1)"
  [[ "${remote_tag}" == "${JANKURAI_REV}" ]] ||
    die "remote tag mismatch: got ${remote_tag:-missing}, want ${JANKURAI_REV}"

  forge_git -C "${scratch}" init -q source
  forge_git -C "${scratch}/source" remote add origin "${JANKURAI_REPO}"
  [[ "$(forge_git -C "${scratch}/source" remote get-url origin)" == "${JANKURAI_REPO}" ]] ||
    die "source remote changed during checkout"
  forge_git -C "${scratch}/source" fetch -q --no-tags --depth 1 origin \
    "refs/tags/${JANKURAI_TAG}:refs/tags/${JANKURAI_TAG}"
  forge_git -C "${scratch}/source" checkout -q --detach "${JANKURAI_REV}"
  [[ "$(forge_git -C "${scratch}/source" rev-parse HEAD)" == "${JANKURAI_REV}" ]] ||
    die "checked-out commit mismatch"
  [[ "$(forge_git -C "${scratch}/source" rev-parse "refs/tags/${JANKURAI_TAG}^{}")" == "${JANKURAI_REV}" ]] ||
    die "checked-out tag mismatch"
  [[ "$(forge_git -C "${scratch}/source" rev-parse "HEAD^{tree}")" == "${JANKURAI_SOURCE_TREE}" ]] ||
    die "source tree mismatch"
  archive_sha="$(forge_git -C "${scratch}/source" archive --format=tar HEAD | sha256sum | awk '{print $1}')"
  [[ "${archive_sha}" == "${JANKURAI_SOURCE_ARCHIVE_SHA256}" ]] ||
    die "source archive mismatch"
  [[ "$(sha256_file "${scratch}/source/Cargo.lock")" == "${JANKURAI_CARGO_LOCK_SHA256}" ]] ||
    die "Cargo.lock mismatch"
  [[ -z "$(forge_git -C "${scratch}/source" status --porcelain --untracked-files=all)" ]] ||
    die "source checkout is dirty before build"

  cache_seed="${JERYU_CARGO_CACHE_SEED:-/home/ubuntu/.cargo}"
  [[ -d "${cache_seed}/registry" ]] || die "offline Cargo cache seed is unavailable"
  mkdir -p "${scratch}/cargo"
  # Materialize the already-populated host cache under a genuinely private
  # scratch CARGO_HOME. Cargo runs offline; no shared cache inode can be changed
  # by the release build.
  cp -a "${cache_seed}/registry" "${scratch}/cargo/registry"
  if [[ -d "${cache_seed}/git" ]]; then
    cp -a "${cache_seed}/git" "${scratch}/cargo/git"
  fi

  mkdir -p "${scratch}/out" "${scratch}/target"
  remap_flags="--remap-path-prefix=${scratch}/source=/jankurai-build/source \
--remap-path-prefix=${scratch}/cargo=/jankurai-build/cargo \
--remap-path-prefix=${scratch}/target=/jankurai-build/target"
  CARGO_HOME="${scratch}/cargo" \
  CARGO_TARGET_DIR="${scratch}/target" \
  RUSTFLAGS="${remap_flags}" \
    cargo "+${JANKURAI_RUST_TOOLCHAIN}" install --locked --offline \
      --path "${scratch}/source/crates/jankurai" --root "${scratch}/out" --bin jankurai
  [[ -z "$(forge_git -C "${scratch}/source" status --porcelain --untracked-files=all)" ]] ||
    die "source checkout became dirty during build"
fi

candidate_version="$("${candidate}" --version 2>/dev/null || true)"
candidate_sha="$(sha256_file "${candidate}")"
[[ "${candidate_version}" == "${JANKURAI_VERSION}" ]] ||
  die "built version mismatch: got ${candidate_version:-missing}, want ${JANKURAI_VERSION}"
[[ "${candidate_sha}" == "${JANKURAI_BINARY_SHA256}" ]] ||
  die "built digest mismatch: got ${candidate_sha}, want ${JANKURAI_BINARY_SHA256}"

if [[ -e "${target}" ]]; then
  [[ -f "${target}" && ! -L "${target}" ]] || die "existing target is not a regular file"
  previous_sha="$(sha256_file "${target}")"
  previous_backup="${rollback_dir}/${previous_sha}"
  if [[ ! -f "${previous_backup}" ]]; then
    backup_stage="${rollback_dir}/.${previous_sha}.stage.$$"
    cp "${target}" "${backup_stage}"
    chmod 755 "${backup_stage}"
    [[ "$(sha256_file "${backup_stage}")" == "${previous_sha}" ]] || die "rollback copy mismatch"
    sync -f "${backup_stage}"
    mv "${backup_stage}" "${previous_backup}"
    sync -f "${rollback_dir}"
  fi
fi

cp "${candidate}" "${stage}"
chmod 755 "${stage}"
[[ "$(sha256_file "${stage}")" == "${JANKURAI_BINARY_SHA256}" ]] || die "staged digest mismatch"
sync -f "${stage}"
if [[ "${test_mode}" == "1" && "${JERYU_INSTALL_TEST_INTERRUPT_BEFORE_RENAME:-0}" == "1" ]]; then
  die "simulated interruption before atomic rename"
fi
mv -f "${stage}" "${target}"
target_replaced=1
sync -f "${install_dir}"
if [[ "${test_mode}" == "1" && "${JERYU_INSTALL_TEST_FAIL_AFTER_RENAME:-0}" == "1" ]]; then
  die "simulated post-rename failure"
fi

installed_version="$("${target}" --version 2>/dev/null || true)"
installed_sha="$(sha256_file "${target}")"
[[ "${installed_version}" == "${JANKURAI_VERSION}" ]] || die "installed version verification failed"
[[ "${installed_sha}" == "${JANKURAI_BINARY_SHA256}" ]] || die "installed digest verification failed"
[[ "$(realpath -m "${target}")" == "${target}" ]] || die "installed path verification failed"

timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
run_id="${JERYU_RUN_ID:-install-${timestamp}-$$}"
operator="${JERYU_OPERATOR:-${USER:-unknown}}"
receipt_stage="${scratch}/installation-receipt.json"
jq -n -S \
  --arg schema "jeryu.jankurai-installation/v1" \
  --arg timestamp "${timestamp}" \
  --arg operator "${operator}" \
  --arg run_id "${run_id}" \
  --arg remote "${JANKURAI_REPO}" \
  --arg commit "${JANKURAI_REV}" \
  --arg tag "${JANKURAI_TAG}" \
  --arg tree "${JANKURAI_SOURCE_TREE}" \
  --arg archive "${JANKURAI_SOURCE_ARCHIVE_SHA256}" \
  --arg lock "${JANKURAI_CARGO_LOCK_SHA256}" \
  --arg verification "${source_verification}" \
  --arg rustc "${actual_rustc}" \
  --arg cargo "${actual_cargo}" \
  --arg target_triple "${actual_target}" \
  --arg mode "${JANKURAI_BUILD_MODE}" \
  --arg binary_sha "${installed_sha}" \
  --arg version "${installed_version}" \
  --arg path "${target}" \
  --arg previous_sha "${previous_sha}" \
  --arg rollback_path "${previous_backup}" \
  --argjson test_mode "$([[ "${test_mode}" == "1" ]] && printf true || printf false)" \
  '{schema:$schema,timestamp:$timestamp,operator:$operator,run_id:$run_id,test_mode:$test_mode,
    source:{remote:$remote,commit:$commit,tag:$tag,tree:$tree,archive_sha256:$archive,
      cargo_lock_sha256:$lock,verification:$verification},
    build:{rustc:$rustc,cargo:$cargo,target_triple:$target_triple,mode:$mode,
      cargo_net_offline:true,dedicated_cargo_home:true,git_global_config_disabled:true,
      git_system_config_disabled:true,git_terminal_prompt:false,jankurai_update_check:false},
    binary:{sha256:$binary_sha,version_output:$version},
    installation:{path:$path,atomic:true,previous_binary_sha256:$previous_sha,
      rollback_artifact:$rollback_path},conclusion:"success"}' > "${receipt_stage}"
receipt_sha="$(sha256_file "${receipt_stage}")"
receipt_path="${receipt_dir}/${receipt_sha}.json"
if [[ ! -f "${receipt_path}" ]]; then
  receipt_install_stage="${receipt_dir}/.${receipt_sha}.stage.$$"
  cp "${receipt_stage}" "${receipt_install_stage}"
  sync -f "${receipt_install_stage}"
  mv "${receipt_install_stage}" "${receipt_path}"
  sync -f "${receipt_dir}"
fi
[[ "$(sha256_file "${receipt_path}")" == "${receipt_sha}" ]] || die "receipt content address mismatch"

success=1
printf 'jeryu jankurai installed: %s sha256=%s path=%s receipt=%s\n' \
  "${installed_version}" "${installed_sha}" "${target}" "${receipt_path}"
