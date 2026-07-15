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
external_pin="${tmp}/external-pin.env"
make_pin "${good_pin}" "${good_sha}"
make_pin "${bad_digest_pin}" "$(printf '0%.0s' {1..64})"
sed 's#^JANKURAI_REPO=.*#JANKURAI_REPO="https://github.com/neverhuman/jankurai.git"#' \
  "${good_pin}" > "${external_pin}"

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
jq -e '
  .test_mode == true and .source.verification == "test-fixture" and
  .governance.status == "diagnostic-candidate" and
  .governance.protected_main == false and
  .governance.protection_policy == "not-applicable" and
  (.governance.manifest_commit | test("^[0-9a-f]{40}$")) and
  (.governance.manifest_tree | test("^[0-9a-f]{40}$")) and
  (.governance.manifest_sha256 | test("^[0-9a-f]{64}$"))
' "${receipt}" >/dev/null || fail "test receipt claims authoritative governance"
jq '.build.git_terminal_prompt = true' "${receipt}" > "${tmp}/tampered-receipt.json"
mv "${tmp}/tampered-receipt.json" "${receipt}"
expect_failure "receipt identity mismatch" run_test_install "${root}" "${good_pin}" "${good}" \
  JERYU_INSTALL_TEST_INTERRUPT_BEFORE_RENAME=1
[[ "$(sha "${root}/bin/jankurai")" == "${good_sha}" ]] ||
  fail "receipt mismatch changed target"

# External source pins are refused before any source command or install mutation.
external_root="${tmp}/external"
expect_failure "external source" run_test_install "${external_root}" "${external_pin}" "${good}"
[[ ! -e "${external_root}/bin/jankurai" ]] || fail "external source installed a target"

# The real-source seam always disables HTTP redirects and constrains proxy bypass
# to loopback. A Git test double proves the exact environment before refusing.
fake_git="${tmp}/git-redirect-guard"
# The single-quoted lines intentionally defer expansion to the generated test double.
# shellcheck disable=SC2016
printf '%s\n' \
  '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  '[[ "${GIT_CONFIG_COUNT:-}" == "2" ]]' \
  '[[ "${GIT_CONFIG_KEY_1:-}" == "http.followRedirects" ]]' \
  '[[ "${GIT_CONFIG_VALUE_1:-}" == "false" ]]' \
  '[[ "${NO_PROXY:-}" == "127.0.0.1,localhost,::1" ]]' \
  'printf seen > "${JERYU_REDIRECT_TEST_LOG}"' \
  'exit 88' > "${fake_git}"
chmod 755 "${fake_git}"
redirect_root="${tmp}/redirect"
expect_failure "redirect guard" env \
  JERYU_INSTALL_TEST_MODE=1 JERYU_INSTALL_ROOT="${redirect_root}" \
  JERYU_PIN_ENV="${good_pin}" JERYU_INSTALL_TEST_GIT_BIN="${fake_git}" \
  JERYU_REDIRECT_TEST_LOG="${tmp}/redirect-seen" bash "${installer}"
[[ "$(cat "${tmp}/redirect-seen")" == "seen" ]] || fail "redirect guard was not applied"
[[ ! -e "${redirect_root}/bin/jankurai" ]] || fail "redirect refusal installed a target"

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

# A pre-existing content-addressed rollback name with corrupt bytes is rejected
# before the governed target can be replaced.
corrupt_root="${tmp}/corrupt-rollback"
mkdir -p "${corrupt_root}/bin" "${corrupt_root}/rollback/jankurai"
cp "${old}" "${corrupt_root}/bin/jankurai"
printf 'corrupt rollback bytes\n' > "${corrupt_root}/rollback/jankurai/${old_sha}"
expect_failure "corrupt rollback artifact" \
  run_test_install "${corrupt_root}" "${good_pin}" "${good}"
[[ "$(sha "${corrupt_root}/bin/jankurai")" == "${old_sha}" ]] ||
  fail "corrupt rollback artifact changed target"

printf 'install-jankurai tests passed: success idempotency receipt-governance external-source redirect-guard wrong-digest wrong-version offline interruption rollback corrupt-rollback\n'
