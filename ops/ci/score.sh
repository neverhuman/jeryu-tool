#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
require_jankurai
require_tool jq

required=(
  agent/owner-map.json
  agent/test-map.json
  agent/generated-zones.toml
  agent/proof-lanes.toml
  agent/audit-policy.toml
  agent/boundaries.toml
  agent/JANKURAI_STANDARD.md
  agent/tool-adoption.toml
  agent/coverage-sources.toml
  schemas/repair-queue.schema.json
  schemas/repair-receipt.schema.json
)
for path in "${required[@]}"; do
  [[ -s "$path" ]] || { printf 'missing quality metadata: %s\n' "$path" >&2; exit 1; }
done
mkdir -p .jankurai target/jankurai
bash ops/ci/tool-adoption.sh
jankurai audit . --full --mode advisory --policy agent/audit-policy.toml --json .jankurai/repo-score.json --md .jankurai/repo-score.md
bash ops/ci/score-report.sh .jankurai/repo-score.json agent/audit-policy.toml
cp .jankurai/repo-score.json target/jankurai/repo-score.json
cp .jankurai/repo-score.md target/jankurai/repo-score.md
printf 'score ok\n'
