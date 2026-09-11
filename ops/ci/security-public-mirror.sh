#!/usr/bin/env bash
# Public GitHub mirror: static workflow lint and committed-secret shape only.
# Host security.sh (gitleaks, syft 1.40.0, cargo-deny) remains the governed lane.
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"
if [[ -n "$(find . -path './.git' -prune -o -name '.env' -type f -print -quit)" ]]; then
  printf 'committed .env file\n' >&2
  exit 1
fi
if ! command -v actionlint >/dev/null 2>&1; then
  work="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/actionlint-$$"
  mkdir -m 700 -p "$work"
  curl --proto "=https" --tlsv1.2 -fsSL -o "$work/actionlint.tar.gz"     https://github.com/rhysd/actionlint/releases/download/v1.7.7/actionlint_1.7.7_linux_amd64.tar.gz
  tar -xzf "$work/actionlint.tar.gz" -C "$work" actionlint
  export PATH="$work:$PATH"
fi
actionlint
printf 'security-public-mirror ok: actionlint and no committed .env\n'
