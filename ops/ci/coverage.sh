#!/usr/bin/env bash
set -euo pipefail
source ops/ci/lib.sh
cd "$REPO_ROOT"
require_jankurai
for tool in cargo-llvm-cov jq; do
  require_tool "$tool"
done

[[ -z "$(git status --porcelain=v1 --untracked-files=all)" ]] || {
  printf 'coverage requires a clean committed source tree\n' >&2
  exit 1
}
for path in target target/llvm-cov target/jankurai target/jankurai/coverage; do
  if [[ -L "$path" || ( -e "$path" && ! -d "$path" ) ]]; then
    printf 'coverage output is not a physical directory: %s\n' "$path" >&2
    exit 1
  fi
  [[ -d "$path" ]] || mkdir -m 0750 -- "$path"
  [[ "$(realpath -e -- "$path")" == "$REPO_ROOT/$path" ]] || {
    printf 'coverage output is aliased: %s\n' "$path" >&2
    exit 1
  }
done

version="$(cargo llvm-cov --version)"
[[ "$version" == 'cargo-llvm-cov 0.8.7' ]] || {
  printf 'expected cargo-llvm-cov 0.8.7, got %s\n' "$version" >&2
  exit 1
}
rm -f -- target/llvm-cov/lcov.info target/jankurai/coverage/rust-lcov.info
cargo llvm-cov --locked --offline -p jeryu-tool-control --all-targets \
  --lcov --output-path target/llvm-cov/lcov.info
[[ -s target/llvm-cov/lcov.info ]]
install -m 0444 -- target/llvm-cov/lcov.info target/jankurai/coverage/rust-lcov.info
jankurai coverage audit . --config agent/coverage-sources.toml \
  --json target/jankurai/coverage/coverage-audit.json \
  --md target/jankurai/coverage/coverage-audit.md
jq -e '.summary.status == "pass" and .summary.hard_findings == 0' \
  target/jankurai/coverage/coverage-audit.json >/dev/null
printf 'coverage ok: jeryu-tool-control\n'
