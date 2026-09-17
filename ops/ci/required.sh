#!/usr/bin/env bash
set -euo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# Hosted `just required` is the merge gate. Use the same Jankurai
# qualification as pr-ci: --candidate only for premerge, fail-closed
# installed/release-broker authority when JAIN_RELEASE_CI=1 or the
# host already matches the pin. Do not demand an installed-authority
# receipt for a manifest that is not on protected main yet.
bash ops/ci/pr-ci.sh
bash ops/ci/repair-receipt-test.sh
bash ops/ci/contract-drift.sh
printf 'required ok: jeryu-tool\n'
