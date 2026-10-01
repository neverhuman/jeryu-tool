#!/usr/bin/env bash
# auto-install-jankurai.sh - keep this host's governed Jankurai on the pin of
# record. One pass, run by jeryu-jankurai-install.timer from the canonical
# jeryu-tool checkout this script lives in:
#   1. fast-forward that checkout to protected main, only when it is clean and on
#      main (anything else is skipped and said, never forced);
#   2. run ops/install-jankurai.sh, which installs the pinned binary or confirms
#      it, and refreshes the authority stamp require_jankurai compares against;
#   3. say so when a root-held copy beside it lags (replacing it needs root).
# Env (from the unit's EnvironmentFile): JERYU_FORGE_TOKEN_FILE (required by the
# installer), JERYU_JANKURAI_ROOT_COPY (optional path of a root-held copy).
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
say() { printf 'auto-install-jankurai: %s\n' "$*"; }

if [[ "$(git -C "${repo}" branch --show-current)" != main ]]; then
  say "skipped: ${repo} is not on main"; exit 0
fi
if [[ -n "$(git -C "${repo}" status --porcelain)" ]]; then
  say "skipped: ${repo} has local changes"; exit 0
fi
git -C "${repo}" fetch -q origin main
if ! git -C "${repo}" merge -q --ff-only origin/main; then
  say "skipped: ${repo} main cannot fast-forward to origin/main"; exit 0
fi
bash "${repo}/ops/install-jankurai.sh"

root_copy="${JERYU_JANKURAI_ROOT_COPY:-}"
if [[ -n "${root_copy}" && -f "${root_copy}" ]]; then
  pinned="$(awk -F'"' '/^\[jankurai\]/ {s = 1; next} /^\[/ {s = 0} s && /^binary_sha256[ \t]*=/ {print $2; exit}' "${repo}/tool-manifest.toml")"
  actual="$(sha256sum "${root_copy}" | awk '{print $1}')"
  if [[ "${actual}" != "${pinned}" ]]; then
    say "WARNING root-held copy ${root_copy} is ${actual}, pin is ${pinned}: as root, install -D -m0555 -o root \$HOME/.jeryu/bin/jankurai ${root_copy}"
  fi
fi
