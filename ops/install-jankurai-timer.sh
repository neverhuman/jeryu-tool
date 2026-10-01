#!/usr/bin/env bash
# install-jankurai-timer.sh - install jeryu-jankurai-install.timer for the current
# user, bound to the jeryu-tool checkout this script is run from (it must be the
# canonical checkout the installer accepts). Writes ~/.config/jeryu/jankurai-install.env
# (mode 600) on first run; set JERYU_FORGE_TOKEN_FILE there, then run this again.
# Env: JERYU_SYSTEMCTL (default systemctl; tests point it at a stand-in).
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
systemctl="${JERYU_SYSTEMCTL:-systemctl}"
units="${HOME}/.config/systemd/user"
env_file="${HOME}/.config/jeryu/jankurai-install.env"
mkdir -p "${units}" "$(dirname "${env_file}")"
if [[ ! -e "${env_file}" ]]; then
  install -m 600 /dev/null "${env_file}"
  echo "created ${env_file}: set JERYU_FORGE_TOKEN_FILE (and optionally JERYU_JANKURAI_ROOT_COPY), then run this again"
fi
sed "s#@JERYU_TOOL_CHECKOUT@#${repo}#" "${repo}/ops/systemd/jeryu-jankurai-install.service" \
  >"${units}/jeryu-jankurai-install.service"
install -m 644 "${repo}/ops/systemd/jeryu-jankurai-install.timer" "${units}/"
"${systemctl}" --user daemon-reload
if ! grep -qE '^JERYU_FORGE_TOKEN_FILE=.+' "${env_file}"; then
  echo "the timer stays off until ${env_file} sets JERYU_FORGE_TOKEN_FILE" >&2
  exit 0
fi
"${systemctl}" --user enable --now jeryu-jankurai-install.timer
echo "jeryu-jankurai-install.timer enabled for ${repo}"
