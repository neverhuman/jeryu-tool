#!/usr/bin/env bash
# The builder image is gated on its repository digest, never on the
# store-specific local image ID.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf -- "${scratch}"' EXIT

die() { printf 'build-jankurai-hermetic: %s\n' "$*" >&2; exit 1; }
# shellcheck source=/dev/null
source <(sed -n '/^# BEGIN verify_builder_image$/,/^# END verify_builder_image$/p' \
  "${here}/build-jankurai-hermetic.sh")

pinned="rust@sha256:$(printf 'd%.0s' {1..64})"
other="rust@sha256:$(printf 'e%.0s' {1..64})"

fake_docker() { # name, repo-digest lines, inspect exit status
  local path="${scratch}/$1"
  printf '#!/usr/bin/env bash\nprintf "%%b" "%s"\nexit %s\n' "$2" "$3" >"${path}"
  chmod 755 "${path}"
  printf '%s' "${path}"
}

expect_ok() { ( verify_builder_image "$1" "$2" ) || { echo "FAIL: $3" >&2; exit 1; }; }
expect_err() {
  local out
  if out="$( ( verify_builder_image "$1" "$2" ) 2>&1 )"; then
    echo "FAIL: $3 was accepted" >&2; exit 1
  fi
  grep -F "$4" <<<"${out}" >/dev/null || { echo "FAIL: $3: ${out}" >&2; exit 1; }
}

# containerd image store: the registry records the short repository digest
expect_ok "$(fake_docker containerd "${pinned}\n" 0)" "${pinned}" "containerd store"
# overlay2 store on another host: same content, qualified repository digest
expect_ok "$(fake_docker overlay2 "docker.io/library/${pinned}\n" 0)" "${pinned}" "overlay2 store"
# the image carries a different repository digest
expect_err "$(fake_docker wrong "${other}\n" 0)" "${pinned}" "wrong digest" \
  "repository digest mismatch"
# the image is absent
expect_err "$(fake_docker absent "" 1)" "${pinned}" "absent image" \
  "pinned builder image is unavailable"
# a mutable tag is never a pinned builder
expect_err "$(fake_docker tag "${pinned}\n" 0)" "rust:1.95.0" "mutable tag" \
  "not a repository digest reference"

printf 'builder image identity ok: repository digest gates, local ID ignored\n'
