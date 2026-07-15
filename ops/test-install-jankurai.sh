#!/usr/bin/env bash
# Transaction and refusal tests for install-jankurai.sh. No governed host path is touched.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
installer="${here}/install-jankurai.sh"
canonical_pin="${here}/../generated/jankurai-pin.env"
tmp="$(mktemp -d /tmp/test-install-jankurai.XXXXXX)"
trap 'rm -rf "${tmp}"' EXIT

fail() {
  printf 'test-install-jankurai: %s\n' "$*" >&2
  exit 1
}

sha() {
  sha256sum "$1" | awk '{print $1}'
}

make_mock() {
  local path="$1" version="$2"
  printf '#!/usr/bin/env bash\nprintf '\''%%s\\n'\'' %q\n' "${version}" > "${path}"
  chmod 755 "${path}"
}

make_pin() {
  local output="$1" digest="$2"
  sed "s/^JANKURAI_BINARY_SHA256=.*/JANKURAI_BINARY_SHA256=\"${digest}\"/" \
    "${canonical_pin}" > "${output}"
}

run_test_install() {
  local root="$1" pin="$2" binary="$3"
  shift 3
  env JERYU_INSTALL_TEST_MODE=1 JERYU_INSTALL_ROOT="${root}" \
    JERYU_PIN_ENV="${pin}" JERYU_INSTALL_TEST_PREBUILT_BINARY="${binary}" \
    JERYU_RUN_ID="installer-test-$$" "$@" bash "${installer}"
}

expect_failure() {
  local description="$1"
  shift
  if "$@" >"${tmp}/failure.log" 2>&1; then
    fail "${description}: command unexpectedly succeeded"
  fi
}

good="${tmp}/good-jankurai"
wrong="${tmp}/wrong-jankurai"
old="${tmp}/old-jankurai"
make_mock "${good}" "jankurai 1.6.11"
make_mock "${wrong}" "jankurai 1.6.10"
make_mock "${old}" "jankurai 1.6.9"
good_sha="$(sha "${good}")"
old_sha="$(sha "${old}")"
good_pin="${tmp}/good-pin.env"
bad_digest_pin="${tmp}/bad-digest-pin.env"
make_pin "${good_pin}" "${good_sha}"
make_pin "${bad_digest_pin}" "$(printf '0%.0s' {1..64})"

# Successful transaction and receipt-bound idempotency.
root="${tmp}/success"
run_test_install "${root}" "${good_pin}" "${good}" >/dev/null
[[ "$(sha "${root}/bin/jankurai")" == "${good_sha}" ]] || fail "success digest mismatch"
receipt_count="$(find "${root}/receipts/jankurai/sha256" -type f -name '*.json' | wc -l)"
run_test_install "${root}" "${good_pin}" "${good}" >/dev/null
[[ "$(find "${root}/receipts/jankurai/sha256" -type f -name '*.json' | wc -l)" == "${receipt_count}" ]] ||
  fail "idempotent run created a new receipt"

# A receipt with any fixed build flag changed is not accepted as idempotent.
receipt="$(find "${root}/receipts/jankurai/sha256" -type f -name '*.json' -print -quit)"
jq '.build.git_terminal_prompt = true' "${receipt}" > "${tmp}/tampered-receipt.json"
mv "${tmp}/tampered-receipt.json" "${receipt}"
expect_failure "receipt identity mismatch" run_test_install "${root}" "${good_pin}" "${good}" \
  JERYU_INSTALL_TEST_INTERRUPT_BEFORE_RENAME=1
[[ "$(sha "${root}/bin/jankurai")" == "${good_sha}" ]] ||
  fail "receipt mismatch changed target"

# Wrong digest and wrong version are rejected before replacing the target.
before="$(sha "${root}/bin/jankurai")"
expect_failure "wrong digest" run_test_install "${root}" "${bad_digest_pin}" "${good}"
[[ "$(sha "${root}/bin/jankurai")" == "${before}" ]] || fail "wrong digest changed target"
wrong_root="${tmp}/wrong-version"
expect_failure "wrong version" run_test_install "${wrong_root}" "${good_pin}" "${wrong}"
[[ ! -e "${wrong_root}/bin/jankurai" ]] || fail "wrong version installed a target"

# An empty dependency cache proves the real source path cannot fetch while offline.
offline_root="${tmp}/offline"
mkdir -p "${tmp}/empty-cargo/registry"
if env JERYU_INSTALL_TEST_MODE=1 JERYU_INSTALL_ROOT="${offline_root}" \
  JERYU_CARGO_CACHE_SEED="${tmp}/empty-cargo" JERYU_RUN_ID="offline-test-$$" \
  bash "${installer}" >"${tmp}/offline.log" 2>&1; then
  fail "offline fetch test unexpectedly succeeded"
fi
[[ ! -e "${offline_root}/bin/jankurai" ]] || fail "offline failure installed a target"
grep -Eqi 'offline|no matching package|failed to download' "${tmp}/offline.log" ||
  fail "offline refusal did not report an offline dependency failure"

# Interruption before rename leaves the previous binary byte-identical.
interrupt_root="${tmp}/interrupt"
mkdir -p "${interrupt_root}/bin"
cp "${old}" "${interrupt_root}/bin/jankurai"
expect_failure "interrupted install" run_test_install "${interrupt_root}" "${good_pin}" "${good}" \
  JERYU_INSTALL_TEST_INTERRUPT_BEFORE_RENAME=1
[[ "$(sha "${interrupt_root}/bin/jankurai")" == "${old_sha}" ]] ||
  fail "interrupted install changed target"

# A post-rename failure restores the previous content-addressed rollback artifact.
rollback_root="${tmp}/rollback"
mkdir -p "${rollback_root}/bin"
cp "${old}" "${rollback_root}/bin/jankurai"
expect_failure "post-rename rollback" run_test_install "${rollback_root}" "${good_pin}" "${good}" \
  JERYU_INSTALL_TEST_FAIL_AFTER_RENAME=1
[[ "$(sha "${rollback_root}/bin/jankurai")" == "${old_sha}" ]] ||
  fail "rollback did not restore previous binary"
[[ "$(sha "${rollback_root}/rollback/jankurai/${old_sha}")" == "${old_sha}" ]] ||
  fail "rollback artifact is missing or corrupt"

printf 'install-jankurai tests passed: success idempotency receipt-mismatch wrong-digest wrong-version offline interruption rollback\n'
