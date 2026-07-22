#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=ops/ci/lib.sh
source "${repo_root}/ops/ci/lib.sh"
require_jankurai

source_head="$(git -C "${repo_root}" rev-parse HEAD)"
sandbox="$(mktemp -d /tmp/jeryu-tool-doctor-controls.XXXXXX)"
cleanup() {
  rm -rf -- "${sandbox}"
}
trap cleanup EXIT

git clone --no-local --quiet --no-checkout "${repo_root}" "${sandbox}/repo"
git -C "${sandbox}/repo" checkout --quiet --detach "${source_head}"
cd "${sandbox}/repo"

# Doctor requires current score artifacts; generate them from the exact detached
# commit before exercising the control-file diagnostics.
bash ops/ci/score.sh >/dev/null
jankurai doctor --fail-on medium >/dev/null

expect_missing_high() {
  local path="$1"
  local output
  output="${sandbox}/missing-$(basename "${path}").log"
  mv "${path}" "${sandbox}/held-file"
  if jankurai doctor --fail-on high >"${output}" 2>&1; then
    printf 'doctor unexpectedly accepted missing %s\n' "${path}" >&2
    exit 1
  fi
  grep -F "high: file:${path}" "${output}" >/dev/null
  mv "${sandbox}/held-file" "${path}"
}

expect_schema_medium() {
  local path="$1"
  local diagnostic="$2"
  local replacement="$3"
  local output
  output="${sandbox}/schema-$(basename "${path}").log"
  cp "${path}" "${sandbox}/held-file"
  printf '%s\n' "${replacement}" >"${path}"
  if jankurai doctor --fail-on medium >"${output}" 2>&1; then
    printf 'doctor unexpectedly accepted invalid schema for %s\n' "${path}" >&2
    exit 1
  fi
  grep -F "medium: ${diagnostic}" "${output}" >/dev/null
  mv "${sandbox}/held-file" "${path}"
}

expect_missing_high agent/standard-version.toml
expect_missing_high agent/security-policy.toml
expect_missing_high tools/security-lane.sh

expect_schema_medium \
  agent/standard-version.toml \
  standard-version-schema \
  'standard = "jankurai"'
expect_schema_medium \
  agent/security-policy.toml \
  security-policy-schema \
  'schema_version = "1.0.0"'
expect_schema_medium \
  agent/boundaries.toml \
  boundaries-manifest-schema \
  '[queues]'
expect_schema_medium \
  agent/generated-zones.toml \
  generated-zones-schema \
  'zones = []'

printf 'hostile fixture\n' >.env
if bash tools/security-lane.sh >"${sandbox}/security-failure.log" 2>&1; then
  printf 'security wrapper unexpectedly accepted a forbidden .env file\n' >&2
  exit 1
fi
grep -F '"tool":"jeryu-tool-security"' "${sandbox}/security-failure.log" >/dev/null
grep -F '"status":"failed"' "${sandbox}/security-failure.log" >/dev/null
rm -f -- .env

jankurai doctor --fail-on medium >/dev/null
git diff --exit-code -- .
printf 'doctor controls hostile tests ok\n'
