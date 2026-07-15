#!/usr/bin/env bash
# Custody and fail-closed tests for render-tool-manifest.sh.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${here}/.." && pwd)"
renderer="${here}/render-tool-manifest.sh"
tmp="$(mktemp -d /tmp/test-render-tool-manifest.XXXXXX)"
trap 'rm -rf "${tmp}"' EXIT

fail() {
  printf 'test-render-tool-manifest: %s\n' "$*" >&2
  exit 1
}

expect_failure() {
  local description="$1" pattern="$2"
  shift 2
  if "$@" >"${tmp}/failure.log" 2>&1; then
    fail "${description}: command unexpectedly succeeded"
  fi
  grep -Fq "${pattern}" "${tmp}/failure.log" || {
    sed -n '1,80p' "${tmp}/failure.log" >&2
    fail "${description}: expected failure text was absent"
  }
}

# A fabricated future predecessor proves rendering is keyed/shape-based rather
# than a one-time string replacement for 1.6.10.
TEST_TMP="${tmp}" RENDERER_PY="${here}/render_tool_manifest.py" python3 - <<'PY'
import importlib.util
import json
import os
from pathlib import Path

spec = importlib.util.spec_from_file_location("render_tool_manifest", os.environ["RENDERER_PY"])
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)
pin = module.load_pin()
root = Path(os.environ["TEST_TMP"])
workflow = root / ".github" / "workflows" / "future.yml"
workflow.parent.mkdir(parents=True)
workflow.write_text(
    "name: future\nenv:\n"
    "  JANKURAI_REPO: \"https://github.com/neverhuman/jankurai.git\"\n"
    "  JANKURAI_TAG: \"v9.9.9-deadlang-precision-split.9\"\n"
    f"  JANKURAI_REV: \"{'f' * 40}\"\n\n"
    "jobs: {}\n"
)
rendered = module.render_consumer(workflow, pin)
for name, key in module.PIN_ENV_FIELDS:
    assert f"  {name}: {json.dumps(pin[key])}" in rendered, name
assert "9.9.9" not in rendered
assert "github.com/neverhuman/jankurai" not in rendered

doc = root / "docs" / "testing.md"
doc.parent.mkdir()
doc.write_text(
    "Jankurai 9.9.9 / jankurai 9.9.9 / "
    "v9.9.9-deadlang-precision-split.9 / "
    "https://github.com/neverhuman/jankurai.git\n"
)
rendered_doc = module.render_consumer(doc, pin)
assert "9.9.9" not in rendered_doc
assert pin["version"] in rendered_doc
assert pin["tag"] in rendered_doc
assert pin["repo"] in rendered_doc
PY

init_repo() {
  local root="$1" origin="$2"
  git init -q "${root}"
  git -C "${root}" config user.name renderer-test
  git -C "${root}" config user.email renderer-test@localhost
  git -C "${root}" remote add origin "${origin}"
  mkdir -p "${root}/ops/ci"
  printf '#!/usr/bin/env bash\nset -euo pipefail\n' > "${root}/ops/ci/lib.sh"
  git -C "${root}" add ops/ci/lib.sh
  git -C "${root}" commit -q -m baseline
}

# No-argument mode detects family drift but never writes it.
family="${tmp}/family"
mkdir -p "${family}/jeryu/ops/ci"
printf '#!/usr/bin/env bash\nset -euo pipefail\n' > "${family}/jeryu/ops/ci/lib.sh"
before="$(sha256sum "${family}/jeryu/ops/ci/lib.sh" | awk '{print $1}')"
expect_failure "unscoped check-only" "unscoped renderer invocation is check-only" \
  bash "${renderer}" --family-root "${family}"
after="$(sha256sum "${family}/jeryu/ops/ci/lib.sh" | awk '{print $1}')"
[[ "${before}" == "${after}" ]] || fail "unscoped invocation mutated a consumer"

expect_failure "missing explicit root" "requires an explicit --repo-root" \
  bash "${renderer}" --repo jeryu

canonical="http://127.0.0.1:8787/git/jeryu/jeryu.git"
dirty="${tmp}/dirty"
init_repo "${dirty}" "${canonical}"
printf '# dirty\n' >> "${dirty}/ops/ci/lib.sh"
expect_failure "dirty root" "must start clean" \
  bash "${renderer}" --repo jeryu --repo-root "jeryu=${dirty}"

wrong_origin="${tmp}/wrong-origin"
init_repo "${wrong_origin}" "http://example.invalid/jeryu.git"
expect_failure "wrong origin" "non-canonical origin" \
  bash "${renderer}" --repo jeryu --repo-root "jeryu=${wrong_origin}"

unrelated="${tmp}/unrelated"
init_repo "${unrelated}" "${canonical}"
expect_failure "unrelated head" "not based on current protected main" \
  bash "${renderer}" --repo jeryu --repo-root "jeryu=${unrelated}"

# The exact clean manifest-owner checkout is a valid explicit no-op write root.
bash "${renderer}" --repo jeryu-tool --repo-root "jeryu-tool=${repo_root}" >/dev/null
[[ -z "$(git -C "${repo_root}" status --porcelain --untracked-files=all)" ]] ||
  fail "validated no-op render dirtied the manifest-owner checkout"

printf 'render-tool-manifest tests passed: keyed-future unscoped missing-root dirty origin ancestry success\n'
