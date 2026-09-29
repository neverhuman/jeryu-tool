#!/usr/bin/env bash
# Prove the score gate reads the findings themselves, not just the summary.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

work="$(mktemp -d -p "${TMPDIR:-/tmp}" score-report-test.XXXXXXXX)"
cleanup() { rm -rf -- "$work"; }
trap cleanup EXIT HUP INT TERM

policy="$work/audit-policy.toml"
printf 'minimum_score = 65\n' >"$policy"

write_report() {
  jq -S "$2" <<'BASE' >"$1"
{
  "score": 87,
  "caps_applied": [],
  "findings": [{"severity": "medium", "hardness": "soft"}],
  "decision": {"hard_findings": 0, "soft_findings": 1}
}
BASE
}

expect_pass() {
  local label="$1" filter="$2"
  write_report "$work/report.json" "$filter"
  bash ops/ci/score-report.sh "$work/report.json" "$policy" >/dev/null || {
    printf 'score report unexpectedly rejected: %s\n' "$label" >&2
    exit 1
  }
}

expect_fail() {
  local label="$1" filter="$2"
  write_report "$work/report.json" "$filter"
  if bash ops/ci/score-report.sh "$work/report.json" "$policy" >/dev/null 2>&1; then
    printf 'score report unexpectedly accepted: %s\n' "$label" >&2
    exit 1
  fi
}

expect_pass 'clean advisory report' '.'
expect_pass 'soft findings at every allowed severity' \
  '.findings = [{"severity":"medium","hardness":"soft"},{"severity":"low"},{"severity":"info"}]'

# The summary claims a clean run; the findings say otherwise.
expect_fail 'high finding under a zero hard-finding summary' \
  '.findings += [{"severity": "high", "hardness": "soft"}]'
expect_fail 'critical finding under a zero hard-finding summary' \
  '.findings += [{"severity": "critical"}]'
expect_fail 'hard finding marked medium' \
  '.findings += [{"severity": "medium", "hardness": "hard"}]'
expect_fail 'unknown finding severity' '.findings += [{"severity": "blocker"}]'
expect_fail 'finding that is not an object' '.findings += ["high"]'
expect_fail 'findings list missing' 'del(.findings)'
expect_fail 'findings list not an array' '.findings = {}'

expect_fail 'summary hard-finding count above zero' '.decision.hard_findings = 1'
expect_fail 'top-level hard-finding count above zero' '.hard_findings = 2'
expect_fail 'negative hard-finding count' '.decision.hard_findings = -1'
expect_fail 'decision missing' 'del(.decision)'

expect_fail 'caps applied' '.caps_applied = ["coverage"]'
expect_fail 'alternate caps field populated' '.caps = ["coverage"]'
expect_fail 'caps_applied missing' 'del(.caps_applied)'

expect_fail 'score below the policy floor' '.score = 64'
expect_fail 'fractional score' '.score = 86.5'
expect_fail 'score missing' 'del(.score)'

printf 'minimum_score = 40\n' >"$policy"
expect_fail 'policy floor below the maintained minimum' '.'
printf 'floor = 65\n' >"$policy"
expect_fail 'policy without minimum_score' '.'
printf 'minimum_score = 65\n' >"$policy"

printf 'score report contract ok\n'
