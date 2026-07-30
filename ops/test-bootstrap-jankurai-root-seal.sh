#!/usr/bin/env bash
# Hostile tests for the one-attempt, receipt-bound root-seal bootstrap.
set -euo pipefail
umask 077

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bootstrap="${here}/bootstrap-jankurai-root-seal.sh"
tmp="$(mktemp -d /tmp/test-bootstrap-jankurai-root-seal.XXXXXX)"
cleanup() {
  rm -rf -- "$tmp"
}
trap cleanup EXIT

fail() {
  printf 'test-bootstrap-jankurai-root-seal: %s\n' "$*" >&2
  exit 1
}

expect_failure() {
  local description="$1" pattern="$2"
  shift 2
  if "$@" >"$tmp/failure.log" 2>&1; then
    fail "$description unexpectedly succeeded"
  fi
  grep -Fq "$pattern" "$tmp/failure.log" || {
    sed -n '1,120p' "$tmp/failure.log" >&2
    fail "$description did not report the expected failure"
  }
}

repo="$tmp/repo"
remote="$tmp/origin.git"
install="$tmp/install"
state="$tmp/state"
evidence="$tmp/evidence"
mkdir -p "$repo" "$install" "$evidence"
git init -q --bare "$remote"
git -C "$repo" init -q
git -C "$repo" config user.name 'Bootstrap Test'
git -C "$repo" config user.email bootstrap-test@example.invalid
git -C "$repo" checkout -q -b codex/bootstrap-test
printf 'fixture manifest\n' >"$repo/tool-manifest.toml"
git -C "$repo" add tool-manifest.toml
git -C "$repo" commit -q -m 'test: bootstrap fixture'
git -C "$repo" remote add origin "$remote"
git -C "$repo" push -q -u origin HEAD
head_sha="$(git -C "$repo" rev-parse HEAD)"
tree_sha="$(git -C "$repo" rev-parse 'HEAD^{tree}')"
manifest_sha="$(sha256sum "$repo/tool-manifest.toml" | awk '{print $1}')"
control_ref=refs/heads/codex/bootstrap-test

candidate="$evidence/jankurai"
cat >"$candidate" <<'CANDIDATE'
#!/usr/bin/env bash
printf 'jankurai test-candidate\n'
CANDIDATE
chmod 0755 "$candidate"
candidate_sha="$(sha256sum "$candidate" | awk '{print $1}')"

predecessor_source="$tmp/predecessor"
cat >"$predecessor_source" <<'PREDECESSOR'
#!/usr/bin/env bash
printf 'jankurai protected-predecessor\n'
PREDECESSOR
chmod 0555 "$predecessor_source"
predecessor_sha="$(sha256sum "$predecessor_source" | awk '{print $1}')"
cp "$predecessor_source" "$install/jankurai"
chmod 0555 "$install/jankurai"

pin_env="$tmp/pin.env"
cat >"$pin_env" <<EOF
export JERYU_JANKURAI_SOURCE_REPO="fixture://jankurai"
export JERYU_JANKURAI_VERSION="jankurai test-candidate"
export JERYU_JANKURAI_SHA256="$candidate_sha"
export JERYU_JANKURAI_SOURCE_REV="$(printf '1%.0s' {1..40})"
export JERYU_JANKURAI_SOURCE_TAG="v-test"
export JERYU_JANKURAI_SOURCE_TREE="$(printf '2%.0s' {1..40})"
export JERYU_JANKURAI_SOURCE_ARCHIVE_SHA256="$(printf '3%.0s' {1..64})"
export JERYU_JANKURAI_CARGO_LOCK_SHA256="$(printf '4%.0s' {1..64})"
export JERYU_JANKURAI_RUSTC_VERSION="rustc test"
export JERYU_JANKURAI_CARGO_VERSION="cargo test"
export JERYU_JANKURAI_TARGET_TRIPLE="x86_64-unknown-linux-gnu"
export JERYU_JANKURAI_BUILD_MODE="test-closed-build"
export JERYU_JANKURAI_PACKAGE_PATH="crates/jankurai"
export JERYU_JANKURAI_BUILDER_IMAGE="fixture@sha256:$(printf '5%.0s' {1..64})"
export JERYU_JANKURAI_BUILDER_IMAGE_ID="sha256:$(printf '5%.0s' {1..64})"
export JERYU_JANKURAI_LINKER_VERSION="ld test"
export JERYU_JANKURAI_GLIBC_VERSION="glibc test"
export JERYU_JANKURAI_VENDOR_FILES_SHA256="$(printf '6%.0s' {1..64})"
export JERYU_JANKURAI_VENDOR_FILE_COUNT="1"
export JERYU_JANKURAI_CARGO_CONFIG_SHA256="$(printf '7%.0s' {1..64})"
export JERYU_JANKURAI_BUILD_ENVIRONMENT="offline"
export JERYU_JANKURAI_RUSTFLAGS="--test"
export JERYU_JANKURAI_BUILD_COMMAND="cargo test"
export JERYU_JANKURAI_BUILD_CONTEXT_SHA256="$(printf '8%.0s' {1..64})"
EOF
# shellcheck disable=SC1090
source "$pin_env"

for kind in publisher sandbox; do
  jq -n --arg schema "test-$kind/v1" --arg digest "$predecessor_sha" \
    '{schema_version:$schema,jankurai_sha256:$digest,
      retained_authority:"protected-main"}' \
    >"$install/host-ci-$kind.config.json"
  chmod 0600 "$install/host-ci-$kind.config.json"
done
publisher="$install/host-ci-publisher.config.json"
sandbox="$install/host-ci-sandbox.config.json"
publisher_original_sha="$(sha256sum "$publisher" | awk '{print $1}')"
sandbox_original_sha="$(sha256sum "$sandbox" | awk '{print $1}')"

runner="$tmp/runner"
cat >"$runner" <<'RUNNER'
#!/usr/bin/env bash
set -euo pipefail
[[ "$#" == 5 && "$1" == jeryu && "$2" == jeryu-tool \
  && "$3" == "$MOCK_HEAD" && "$4" == "$JERYU_BOOTSTRAP_REPO_ROOT" \
  && "$5" == jeryu-tool/required ]]
[[ "$(sha256sum "$JERYU_BOOTSTRAP_INSTALL_DIR/jankurai" | awk '{print $1}')" \
  == "$MOCK_CANDIDATE_SHA" ]]
[[ "$(jq -er '.jankurai_sha256' \
  "$JERYU_BOOTSTRAP_INSTALL_DIR/host-ci-publisher.config.json")" \
  == "$MOCK_CANDIDATE_SHA" ]]
[[ "$(jq -er '.jankurai_sha256' \
  "$JERYU_BOOTSTRAP_INSTALL_DIR/host-ci-sandbox.config.json")" \
  == "$MOCK_CANDIDATE_SHA" ]]
if [[ -n "${MOCK_READY_FILE:-}" ]]; then
  : >"$MOCK_READY_FILE"
fi
if [[ -n "${MOCK_SLEEP_SECONDS:-}" ]]; then
  sleep "$MOCK_SLEEP_SECONDS"
fi
exit "${MOCK_EXIT_CODE:-0}"
RUNNER
chmod 0755 "$runner"

receipt_stage="$evidence/receipt-stage.json"
jq -n -S \
  --arg remote "$JERYU_JANKURAI_SOURCE_REPO" \
  --arg commit "$JERYU_JANKURAI_SOURCE_REV" \
  --arg tag "$JERYU_JANKURAI_SOURCE_TAG" \
  --arg source_tree "$JERYU_JANKURAI_SOURCE_TREE" \
  --arg archive "$JERYU_JANKURAI_SOURCE_ARCHIVE_SHA256" \
  --arg lock "$JERYU_JANKURAI_CARGO_LOCK_SHA256" \
  --arg rustc "$JERYU_JANKURAI_RUSTC_VERSION" \
  --arg cargo "$JERYU_JANKURAI_CARGO_VERSION" \
  --arg triple "$JERYU_JANKURAI_TARGET_TRIPLE" \
  --arg mode "$JERYU_JANKURAI_BUILD_MODE" \
  --arg package_path "$JERYU_JANKURAI_PACKAGE_PATH" \
  --arg builder_image "$JERYU_JANKURAI_BUILDER_IMAGE" \
  --arg builder_image_id "$JERYU_JANKURAI_BUILDER_IMAGE_ID" \
  --arg linker "$JERYU_JANKURAI_LINKER_VERSION" \
  --arg glibc "$JERYU_JANKURAI_GLIBC_VERSION" \
  --arg vendor "$JERYU_JANKURAI_VENDOR_FILES_SHA256" \
  --arg vendor_count "$JERYU_JANKURAI_VENDOR_FILE_COUNT" \
  --arg cargo_config "$JERYU_JANKURAI_CARGO_CONFIG_SHA256" \
  --arg environment "$JERYU_JANKURAI_BUILD_ENVIRONMENT" \
  --arg rustflags "$JERYU_JANKURAI_RUSTFLAGS" \
  --arg command "$JERYU_JANKURAI_BUILD_COMMAND" \
  --arg context "$JERYU_JANKURAI_BUILD_CONTEXT_SHA256" \
  --arg digest "$candidate_sha" \
  --arg version "$JERYU_JANKURAI_VERSION" \
  --arg path "$candidate" \
  --arg manifest_repo "$remote" \
  --arg manifest_commit "$head_sha" \
  --arg manifest_tree "$tree_sha" \
  --arg manifest_sha "$manifest_sha" \
  '{schema:"jeryu.jankurai-installation/v2",timestamp:"test",operator:"test",
    run_id:"test",test_mode:true,
    source:{remote:$remote,commit:$commit,tag:$tag,tree:$source_tree,
      archive_sha256:$archive,cargo_lock_sha256:$lock,
      verification:"diagnostic-candidate"},
    build:{rustc:$rustc,cargo:$cargo,target_triple:$triple,mode:$mode,
      package_path:$package_path,builder_image:$builder_image,
      builder_image_id:$builder_image_id,linker:$linker,glibc:$glibc,
      vendor_files_sha256:$vendor,vendor_file_count:$vendor_count,
      cargo_config_sha256:$cargo_config,environment:$environment,rustflags:$rustflags,
      command:$command,context_sha256:$context,cargo_net_offline:true,
      closed_vendor:true,network_none:true,read_only_root:true,non_root:true,
      capabilities_dropped:true,no_new_privileges:true,
      container_engine_path:"/usr/bin/docker",git_global_config_disabled:true,
      git_system_config_disabled:true,git_http_follow_redirects:false,
      git_terminal_prompt:false,jankurai_update_check:false,
      network_scope:"local-forge-source-plus-closed-vendor-network-none",
      no_proxy:"127.0.0.1,localhost,::1"},
    governance:{status:"diagnostic-candidate",manifest_repo:$manifest_repo,
      manifest_commit:$manifest_commit,manifest_tree:$manifest_tree,
      manifest_sha256:$manifest_sha,protected_main:false,
      protection_policy:"not-applicable"},
    binary:{sha256:$digest,version_output:$version},
    installation:{path:$path,atomic:true},conclusion:"success"}' \
  >"$receipt_stage"
receipt_sha="$(sha256sum "$receipt_stage" | awk '{print $1}')"
receipt="$evidence/$receipt_sha.json"
mv "$receipt_stage" "$receipt"

new_attempt() {
  printf '%s\n' "$(date +%s%N)-$RANDOM-$BASHPID" |
    sha256sum | awk '{print $1}'
}

make_request() {
  local output="$1" attempt="$2" filter="${3:-.}"
  jq -n -S \
    --arg attempt "$attempt" \
    --arg ref "$control_ref" \
    --arg head "$head_sha" \
    --arg tree "$tree_sha" \
    --arg candidate "$candidate" \
    --arg receipt "$receipt" \
    --arg receipt_sha "$receipt_sha" \
    '{schema:"jeryu.jankurai-root-seal-bootstrap/v1",attempt_id:$attempt,
      ref:$ref,head_sha:$head,tree_sha:$tree,candidate_path:$candidate,
      candidate_receipt_path:$receipt,
      candidate_receipt_sha256:$receipt_sha,
      created_at_epoch:990,expires_at_epoch:1100}' |
    jq "$filter" >"$output"
}

bootstrap_env=(
    JERYU_BOOTSTRAP_TEST_MODE=1 \
    JERYU_BOOTSTRAP_REPO_ROOT="$repo" \
    JERYU_BOOTSTRAP_INSTALL_DIR="$install" \
    JERYU_BOOTSTRAP_STATE_ROOT="$state" \
    JERYU_BOOTSTRAP_RUNNER="$runner" \
    JERYU_BOOTSTRAP_REMOTE="$remote" \
    JERYU_BOOTSTRAP_PIN_ENV="$pin_env" \
    JERYU_BOOTSTRAP_EXPECTED_PREDECESSOR_SHA256="$predecessor_sha" \
    JERYU_BOOTSTRAP_TEST_ALLOW_HEAD_REUSE=1 \
    JERYU_BOOTSTRAP_NOW=1000 \
    MOCK_HEAD="$head_sha" MOCK_CANDIDATE_SHA="$candidate_sha"
)
invoke() {
  env "${bootstrap_env[@]}" "$bootstrap" "$@"
}

assert_restored() {
  [[ "$(sha256sum "$install/jankurai" | awk '{print $1}')" == "$predecessor_sha" ]] ||
    fail 'protected predecessor binary was not restored'
  [[ "$(sha256sum "$publisher" | awk '{print $1}')" == "$publisher_original_sha" \
    && "$(sha256sum "$sandbox" | awk '{print $1}')" == "$sandbox_original_sha" ]] ||
    fail 'protected predecessor configs were not restored byte-for-byte'
  [[ ! -e "$state/active.json" ]] || fail 'active recovery marker survived restoration'
  if [[ -d "${state}-runner-custody" ]] \
    && find "${state}-runner-custody" -mindepth 1 -maxdepth 1 -print -quit |
      grep -q .; then
    fail 'root-held runner materialization survived restoration'
  fi
}

result_for_attempt() {
  local attempt="$1"
  local -a results
  mapfile -t results < <(find "$state/attempts/$attempt" -maxdepth 1 -type f \
    -regextype posix-extended -regex '.*/[0-9a-f]{64}\.json' -print)
  [[ "${#results[@]}" == 1 ]] ||
    fail "attempt $attempt did not publish exactly one content-addressed result"
  [[ "$(sha256sum "${results[0]}" | awk '{print $1}').json" == \
    "$(basename "${results[0]}")" ]] ||
    fail "attempt $attempt result content address is invalid"
  printf '%s\n' "${results[0]}"
}

request="$evidence/request-success.json"
success_attempt="$(new_attempt)"
make_request "$request" "$success_attempt"
invoke "$request" >"$tmp/success.log"
grep -Fq 'predecessor_restored=true' "$tmp/success.log"
assert_restored
result="$(result_for_attempt "$success_attempt")"
jq -e --arg predecessor "$predecessor_sha" --arg candidate "$candidate_sha" '
  .conclusion == "success" and .seal_exit_code == 0
  and .predecessor_restored == true
  and .predecessor_sha256 == $predecessor
  and .candidate_sha256 == $candidate
  and (.runner_sha256 | test("^[0-9a-f]{64}$"))
' "$result" >/dev/null

one_head_state="$tmp/one-head-state"
one_head_request="$evidence/request-one-head.json"
make_request "$one_head_request" "$(new_attempt)"
env "${bootstrap_env[@]/JERYU_BOOTSTRAP_STATE_ROOT=$state/JERYU_BOOTSTRAP_STATE_ROOT=$one_head_state}" \
  JERYU_BOOTSTRAP_TEST_ALLOW_HEAD_REUSE=0 \
  "$bootstrap" "$one_head_request" >"$tmp/one-head-success.log"
assert_restored
second_head_request="$evidence/request-one-head-reuse.json"
make_request "$second_head_request" "$(new_attempt)"
expect_failure 'second attempt for exact head' \
  'exact head already consumed its sole root-seal attempt' \
  env "${bootstrap_env[@]/JERYU_BOOTSTRAP_STATE_ROOT=$state/JERYU_BOOTSTRAP_STATE_ROOT=$one_head_state}" \
    JERYU_BOOTSTRAP_TEST_ALLOW_HEAD_REUSE=0 \
    "$bootstrap" "$second_head_request"

expect_failure 'attempt reuse' 'attempt identifier was already consumed' \
  invoke "$request"

bad="$evidence/request-extra-command.json"
make_request "$bad" "$(new_attempt)" '.command=["/bin/true"]'
expect_failure 'caller command injection' \
  'bootstrap request schema or closed fields are invalid' invoke "$bad"
expect_failure 'extra positional command' 'usage:' invoke "$request" /bin/true

bad_candidate="$evidence/jankurai-wrong"
printf '#!/usr/bin/env bash\nprintf "jankurai test-candidate\\n"\n#wrong\n' \
  >"$bad_candidate"
chmod 0755 "$bad_candidate"
bad="$evidence/request-wrong-digest.json"
make_request "$bad" "$(new_attempt)"
jq --arg path "$bad_candidate" '.candidate_path=$path' "$bad" >"$bad.next"
mv "$bad.next" "$bad"
expect_failure 'wrong candidate digest' \
  'qualified candidate digest differs from the reviewed pin' invoke "$bad"

bad="$evidence/request-wrong-ref.json"
make_request "$bad" "$(new_attempt)" '.ref="refs/heads/codex/wrong-ref"'
expect_failure 'wrong ref' 'bootstrap request ref differs from the checked-out branch' \
  invoke "$bad"
bad="$evidence/request-wrong-head.json"
make_request "$bad" "$(new_attempt)" '.head_sha=("a"*40)'
expect_failure 'wrong head' 'bootstrap request head or tree differs from the checkout' \
  invoke "$bad"
bad="$evidence/request-wrong-tree.json"
make_request "$bad" "$(new_attempt)" '.tree_sha=("b"*40)'
expect_failure 'wrong tree' 'bootstrap request head or tree differs from the checkout' \
  invoke "$bad"
bad="$evidence/request-expired.json"
make_request "$bad" "$(new_attempt)" \
  '.created_at_epoch=800 | .expires_at_epoch=900'
expect_failure 'expired request' 'bootstrap request is expired' invoke "$bad"
bad="$evidence/request-long-lease.json"
make_request "$bad" "$(new_attempt)" \
  '.created_at_epoch=100 | .expires_at_epoch=1100'
expect_failure 'overlong request' 'bootstrap request is expired' invoke "$bad"

tampered_receipt_stage="$evidence/tampered-receipt-stage.json"
jq '.governance.protected_main=true' "$receipt" >"$tampered_receipt_stage"
tampered_receipt_sha="$(sha256sum "$tampered_receipt_stage" | awk '{print $1}')"
tampered_receipt="$evidence/$tampered_receipt_sha.json"
mv "$tampered_receipt_stage" "$tampered_receipt"
bad="$evidence/request-tampered-receipt.json"
make_request "$bad" "$(new_attempt)"
jq --arg path "$tampered_receipt" --arg sha "$tampered_receipt_sha" \
  '.candidate_receipt_path=$path | .candidate_receipt_sha256=$sha' \
  "$bad" >"$bad.next"
mv "$bad.next" "$bad"
expect_failure 'tampered receipt' \
  'candidate qualification receipt does not bind the exact candidate' invoke "$bad"

failed_attempt="$(new_attempt)"
bad="$evidence/request-failed-command.json"
make_request "$bad" "$failed_attempt"
# Invoke directly so the expected nonzero result remains inspectable.
set +e
MOCK_EXIT_CODE=23 invoke "$bad" >"$tmp/failed-command.log" 2>&1
failed_rc=$?
set -e
[[ "$failed_rc" == 23 ]] || fail 'failed seal command exit code was not preserved'
assert_restored
jq -e '.conclusion == "failure" and .seal_exit_code == 23
  and .predecessor_restored == true' \
  "$(result_for_attempt "$failed_attempt")" >/dev/null

replacement_request="$evidence/request-candidate-replacement.json"
make_request "$replacement_request" "$(new_attempt)"
ready="$tmp/replacement.ready"
release="$tmp/replacement.release"
cp "$candidate" "$tmp/candidate.backup"
env "${bootstrap_env[@]}" \
  JERYU_BOOTSTRAP_TEST_PAUSE_READY_FILE="$ready" \
  JERYU_BOOTSTRAP_TEST_PAUSE_RELEASE_FILE="$release" \
  "$bootstrap" "$replacement_request" >"$tmp/candidate-replacement.log" 2>&1 &
replacement_pid=$!
while [[ ! -e "$ready" ]]; do read -r -t 0.05 _ </dev/null || true; done
printf '#!/usr/bin/env bash\nexit 0\n' >"$tmp/candidate.hostile"
chmod 0755 "$tmp/candidate.hostile"
mv -fT "$tmp/candidate.hostile" "$candidate"
: >"$release"
if wait "$replacement_pid"; then
  fail 'candidate replacement hostile unexpectedly succeeded'
fi
grep -Fq 'qualified candidate replacement detected' \
  "$tmp/candidate-replacement.log"
mv -fT "$tmp/candidate.backup" "$candidate"
chmod 0755 "$candidate"
assert_restored

replacement_request="$evidence/request-candidate-content-drift.json"
make_request "$replacement_request" "$(new_attempt)"
rm -f "$ready" "$release"
cp "$candidate" "$tmp/candidate.backup"
env "${bootstrap_env[@]}" \
  JERYU_BOOTSTRAP_TEST_PAUSE_READY_FILE="$ready" \
  JERYU_BOOTSTRAP_TEST_PAUSE_RELEASE_FILE="$release" \
  "$bootstrap" "$replacement_request" >"$tmp/candidate-content-drift.log" 2>&1 &
replacement_pid=$!
while [[ ! -e "$ready" ]]; do read -r -t 0.05 _ </dev/null || true; done
printf '\n# same-inode candidate content drift\n' >>"$candidate"
: >"$release"
if wait "$replacement_pid"; then
  fail 'candidate same-inode content drift hostile unexpectedly succeeded'
fi
grep -Fq 'qualified candidate content drift detected' \
  "$tmp/candidate-content-drift.log"
cp "$tmp/candidate.backup" "$candidate"
rm -f "$tmp/candidate.backup"
chmod 0755 "$candidate"
assert_restored

replacement_request="$evidence/request-broker-replacement.json"
make_request "$replacement_request" "$(new_attempt)"
rm -f "$ready" "$release"
env "${bootstrap_env[@]}" \
  JERYU_BOOTSTRAP_TEST_PAUSE_READY_FILE="$ready" \
  JERYU_BOOTSTRAP_TEST_PAUSE_RELEASE_FILE="$release" \
  "$bootstrap" "$replacement_request" >"$tmp/replacement.log" 2>&1 &
replacement_pid=$!
while [[ ! -e "$ready" ]]; do read -r -t 0.05 _ </dev/null || true; done
printf 'hostile broker replacement\n' >"$install/jankurai.hostile"
chmod 0555 "$install/jankurai.hostile"
mv -fT "$install/jankurai.hostile" "$install/jankurai"
: >"$release"
if wait "$replacement_pid"; then
  fail 'broker replacement hostile unexpectedly succeeded'
fi
grep -Fq 'broker or config replacement detected' "$tmp/replacement.log"
assert_restored

replacement_request="$evidence/request-config-replacement.json"
make_request "$replacement_request" "$(new_attempt)"
rm -f "$ready" "$release"
env "${bootstrap_env[@]}" \
  JERYU_BOOTSTRAP_TEST_PAUSE_READY_FILE="$ready" \
  JERYU_BOOTSTRAP_TEST_PAUSE_RELEASE_FILE="$release" \
  "$bootstrap" "$replacement_request" >"$tmp/config-replacement.log" 2>&1 &
replacement_pid=$!
while [[ ! -e "$ready" ]]; do read -r -t 0.05 _ </dev/null || true; done
printf '{"hostile":true}\n' >"$install/publisher.hostile"
chmod 0600 "$install/publisher.hostile"
mv -fT "$install/publisher.hostile" "$publisher"
: >"$release"
if wait "$replacement_pid"; then
  fail 'config replacement hostile unexpectedly succeeded'
fi
grep -Fq 'broker or config replacement detected' "$tmp/config-replacement.log"
assert_restored

replacement_request="$evidence/request-runner-replacement.json"
make_request "$replacement_request" "$(new_attempt)"
rm -f "$ready" "$release"
cp "$runner" "$tmp/runner.backup"
env "${bootstrap_env[@]}" \
  JERYU_BOOTSTRAP_TEST_PAUSE_READY_FILE="$ready" \
  JERYU_BOOTSTRAP_TEST_PAUSE_RELEASE_FILE="$release" \
  "$bootstrap" "$replacement_request" >"$tmp/runner-replacement.log" 2>&1 &
replacement_pid=$!
while [[ ! -e "$ready" ]]; do read -r -t 0.05 _ </dev/null || true; done
printf '#!/usr/bin/env bash\nexit 0\n' >"$tmp/runner.hostile"
chmod 0755 "$tmp/runner.hostile"
mv -fT "$tmp/runner.hostile" "$runner"
: >"$release"
if wait "$replacement_pid"; then
  fail 'runner replacement hostile unexpectedly succeeded'
fi
grep -Fq 'host-CI runner replacement detected' "$tmp/runner-replacement.log"
mv -fT "$tmp/runner.backup" "$runner"
chmod 0755 "$runner"
assert_restored

replacement_request="$evidence/request-runner-content-drift.json"
make_request "$replacement_request" "$(new_attempt)"
rm -f "$ready" "$release"
cp "$runner" "$tmp/runner.backup"
env "${bootstrap_env[@]}" \
  JERYU_BOOTSTRAP_TEST_PAUSE_READY_FILE="$ready" \
  JERYU_BOOTSTRAP_TEST_PAUSE_RELEASE_FILE="$release" \
  "$bootstrap" "$replacement_request" >"$tmp/runner-content-drift.log" 2>&1 &
replacement_pid=$!
while [[ ! -e "$ready" ]]; do read -r -t 0.05 _ </dev/null || true; done
printf '\n# same-inode runner content drift\n' >>"$runner"
: >"$release"
if wait "$replacement_pid"; then
  fail 'runner same-inode content drift hostile unexpectedly succeeded'
fi
grep -Fq 'host-CI runner content drift detected' \
  "$tmp/runner-content-drift.log"
cp "$tmp/runner.backup" "$runner"
rm -f "$tmp/runner.backup"
chmod 0755 "$runner"
assert_restored

interrupt_attempt="$(new_attempt)"
interrupt_request="$evidence/request-interrupt.json"
make_request "$interrupt_request" "$interrupt_attempt"
interrupt_ready="$tmp/interrupt.ready"
env "${bootstrap_env[@]}" MOCK_READY_FILE="$interrupt_ready" MOCK_SLEEP_SECONDS=30 \
  "$bootstrap" "$interrupt_request" >"$tmp/interrupt.log" 2>&1 &
interrupt_pid=$!
while [[ ! -e "$interrupt_ready" ]]; do read -r -t 0.05 _ </dev/null || true; done
kill -TERM "$interrupt_pid"
if wait "$interrupt_pid"; then
  fail 'interrupted transaction unexpectedly succeeded'
fi
assert_restored
grep -Fq 'exit-trap-restoration' \
  "$state/attempts/$interrupt_attempt/restoration-status"

kill_attempt="$(new_attempt)"
kill_request="$evidence/request-kill.json"
make_request "$kill_request" "$kill_attempt"
kill_ready="$tmp/kill.ready"
env "${bootstrap_env[@]}" MOCK_READY_FILE="$kill_ready" MOCK_SLEEP_SECONDS=30 \
  "$bootstrap" "$kill_request" >"$tmp/kill.log" 2>&1 &
kill_pid=$!
while [[ ! -e "$kill_ready" ]]; do read -r -t 0.05 _ </dev/null || true; done
kill -KILL "$kill_pid"
wait "$kill_pid" 2>/dev/null || true
[[ -e "$state/active.json" ]] ||
  fail 'kill hostile did not leave the durable recovery marker'
[[ "$(sha256sum "$install/jankurai" | awk '{print $1}')" == "$candidate_sha" ]] ||
  fail 'kill hostile did not exercise candidate-live recovery'
recovery_request="$evidence/request-recovery.json"
make_request "$recovery_request" "$(new_attempt)"
invoke "$recovery_request" >"$tmp/recovery.log"
assert_restored
grep -Fq 'interrupted-recovery' "$state/attempts/$kill_attempt/restoration-status"

grep -Fq '/usr/local/libexec/jain' "$bootstrap" ||
  fail 'fixed production broker install root is absent'
grep -Fq 'production bootstrap is root-only' "$bootstrap" ||
  fail 'root-only production authority is absent'
grep -Fq 'production override is forbidden' "$bootstrap" ||
  fail 'caller broker/command overrides are not rejected'
grep -Fq 'root-seal runner is not checked out at protected SplitOps main' "$bootstrap" ||
  fail 'protected runner authority validation is absent'
grep -Fq 'protected_main' "${here}/install-jankurai.sh" ||
  fail 'production installer protected-main validation is absent'
grep -Fq 'release broker Jankurai rejects caller receipt authority' \
  "${here}/ci/lib.sh" ||
  fail 'ordinary release broker still accepts caller receipt authority'

printf 'root-seal bootstrap tests passed: digest ref head tree receipt expiry reuse command replacement same-inode-drift held-execution interruption recovery restoration\n'
