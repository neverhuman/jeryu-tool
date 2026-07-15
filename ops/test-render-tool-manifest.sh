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
dirty_head="$(git -C "${dirty}" rev-parse HEAD)"
printf '# dirty\n' >> "${dirty}/ops/ci/lib.sh"
expect_failure "dirty root" "must start clean" \
  bash "${renderer}" --repo jeryu --repo-root "jeryu=${dirty}" \
    --expected-head "jeryu=${dirty_head}"

wrong_origin="${tmp}/wrong-origin"
init_repo "${wrong_origin}" "http://example.invalid/jeryu.git"
wrong_origin_head="$(git -C "${wrong_origin}" rev-parse HEAD)"
expect_failure "wrong origin" "non-canonical origin" \
  bash "${renderer}" --repo jeryu --repo-root "jeryu=${wrong_origin}" \
    --expected-head "jeryu=${wrong_origin_head}"

unrelated="${tmp}/unrelated"
init_repo "${unrelated}" "${canonical}"
unrelated_head="$(git -C "${unrelated}" rev-parse HEAD)"
expect_failure "unrelated head" "not based on current protected main" \
  bash "${renderer}" --repo jeryu --repo-root "jeryu=${unrelated}" \
    --expected-head "jeryu=${unrelated_head}"

repo_head="$(git -C "${repo_root}" rev-parse HEAD)"
expect_failure "missing expected head" "requires --expected-head" \
  bash "${renderer}" --repo jeryu-tool --repo-root "jeryu-tool=${repo_root}"
expect_failure "malformed expected head" "invalid --expected-head" \
  bash "${renderer}" --repo jeryu-tool --repo-root "jeryu-tool=${repo_root}" \
    --expected-head "jeryu-tool=not-a-sha"
expect_failure "duplicate expected head" "duplicate --expected-head" \
  bash "${renderer}" --repo jeryu-tool --repo-root "jeryu-tool=${repo_root}" \
    --expected-head "jeryu-tool=${repo_head}" --expected-head "jeryu-tool=${repo_head}"
expect_failure "unselected expected head" "without matching --repo" \
  bash "${renderer}" --repo jeryu-tool --repo-root "jeryu-tool=${repo_root}" \
    --expected-head "jeryu=${repo_head}"

# A clean canonical linear descendant is still the wrong worktree when its
# handed-off SHA names the protected-main parent. Refuse before touching bytes.
wrong_descendant="${tmp}/wrong-descendant"
git clone -q "${canonical}" "${wrong_descendant}"
git -C "${wrong_descendant}" config user.name renderer-test
git -C "${wrong_descendant}" config user.email renderer-test@localhost
printf 'unrelated descendant\n' > "${wrong_descendant}/wrong-descendant.txt"
git -C "${wrong_descendant}" add wrong-descendant.txt
git -C "${wrong_descendant}" commit -q -m 'unrelated descendant'
handed_off_head="$(git -C "${wrong_descendant}" rev-parse HEAD^)"
descendant_before="$(sha256sum "${wrong_descendant}/ops/ci/lib.sh" | awk '{print $1}')"
expect_failure "wrong clean descendant" "write root HEAD mismatch" \
  bash "${renderer}" --repo jeryu --repo-root "jeryu=${wrong_descendant}" \
    --expected-head "jeryu=${handed_off_head}"
descendant_after="$(sha256sum "${wrong_descendant}/ops/ci/lib.sh" | awk '{print $1}')"
[[ "${descendant_before}" == "${descendant_after}" ]] ||
  fail "HEAD mismatch mutated the consumer before refusal"

# A consumer-only write cannot repair or otherwise touch the renderer owner's
# generated env as an implicit side effect.
renderer_fixture="${tmp}/renderer-owner"
mkdir -p "${renderer_fixture}/ops" "${renderer_fixture}/generated"
cp "${here}/render-tool-manifest.sh" "${renderer_fixture}/ops/"
cp "${here}/render_tool_manifest.py" "${renderer_fixture}/ops/"
cp "${repo_root}/tool-manifest.toml" "${renderer_fixture}/"
printf 'deliberately stale owner pin\n' > "${renderer_fixture}/generated/jankurai-pin.env"
scoped_consumer="${tmp}/scoped-consumer"
git clone -q "${canonical}" "${scoped_consumer}"
scoped_head="$(git -C "${scoped_consumer}" rev-parse HEAD)"
bash "${renderer_fixture}/ops/render-tool-manifest.sh" --repo jeryu \
  --repo-root "jeryu=${scoped_consumer}" --expected-head "jeryu=${scoped_head}" >/dev/null
[[ "$(cat "${renderer_fixture}/generated/jankurai-pin.env")" == \
  "deliberately stale owner pin" ]] || fail "consumer render mutated manifest-owner pin"

# The exact clean manifest-owner checkout is a valid explicit no-op write root.
bash "${renderer}" --repo jeryu-tool --repo-root "jeryu-tool=${repo_root}" \
  --expected-head "jeryu-tool=${repo_head}" >/dev/null
[[ -z "$(git -C "${repo_root}" status --porcelain --untracked-files=all)" ]] ||
  fail "validated no-op render dirtied the manifest-owner checkout"

printf 'render-tool-manifest tests passed: keyed-future custody exact-head scope success\n'
