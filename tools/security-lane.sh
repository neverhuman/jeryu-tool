#!/usr/bin/env bash
set -u -o pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}" || exit 1

status=0
bash ops/ci/security.sh "$@" || status=$?
step_status=ran
if (( status != 0 )); then
  step_status=failed
fi
printf '%s\n' \
  "jankurai-security-step={\"label\":\"jeryu-tool-security\",\"tool\":\"jeryu-tool-security\",\"shell_command\":\"bash ops/ci/security.sh\",\"status\":\"${step_status}\",\"advisory\":false,\"exit_code\":${status}}"
exit "${status}"
