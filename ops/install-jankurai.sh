#!/usr/bin/env bash
# Install the jeryu-OWNED, pinned jankurai binary to ~/.jeryu/bin/jankurai.
#
# This is the HOST side of "jeryu manages jankurai globally". Every host CI lane
# resolves the auditor through JERYU_JANKURAI_BIN; pointing that at ~/.jeryu/bin
# (see jeryu-deploy ops/ci/common.sh) means one jeryu-managed binary serves the
# whole family — no per-repo `cargo install`, no ~/.local/bin vs ~/.cargo/bin
# shadowing. The sandbox image keeps its OWN baked copy (it is `--network none`);
# both come from the same tool-manifest.toml pin and are kept in lockstep by the
# drift check, so host == image == manifest.
#
#   ops/install-jankurai.sh            # install/verify the pinned binary
#   JERYU_INSTALL_DIR=/x ops/...       # override the install prefix (default ~/.jeryu)
#
# Standalone jankurai users are unaffected: they keep their own ~/.local/bin
# install via jankurai's installer; this only manages the jeryu-owned copy.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Pin truth: prefer the generated env, fall back to rendering it from the manifest.
pin_env="${here}/../generated/jankurai-pin.env"
if [ ! -f "${pin_env}" ]; then
  python3 "${here}/render_tool_manifest.py" >/dev/null
fi
# shellcheck source=/dev/null
source "${pin_env}"

install_dir="${JERYU_INSTALL_DIR:-$HOME/.jeryu}/bin"
target="${install_dir}/jankurai"
mkdir -p "${install_dir}"

if [ -x "${target}" ] && "${target}" --version | grep -qx "${JANKURAI_VERSION}"; then
  echo "jeryu jankurai already current: ${JANKURAI_VERSION} at ${target}"
  exit 0
fi

# Build into a scratch CARGO_HOME so a half-written ~/.cargo is never the source of
# truth, then place the binary atomically under ~/.jeryu/bin.
scratch="$(mktemp -d)"
trap 'rm -rf "${scratch}"' EXIT
echo "installing pinned ${JANKURAI_VERSION} from ${JANKURAI_REPO}@${JANKURAI_REV}"
GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
  CARGO_HOME="${scratch}/cargo" \
  cargo install --git "${JANKURAI_REPO}" --rev "${JANKURAI_REV}" --locked \
    --root "${scratch}/out" --bin jankurai jankurai
install -m755 "${scratch}/out/bin/jankurai" "${target}"
"${target}" --version | grep -qx "${JANKURAI_VERSION}"
echo "jeryu jankurai ok: ${JANKURAI_VERSION} at ${target}"
