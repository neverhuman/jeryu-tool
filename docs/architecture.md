# Architecture

`jeryu-tool` is the **audit control plane** for the jeryu family. It owns no
product code and ships no binary; it governs how the family pins, installs,
forces, and reports on the jankurai auditor.

## Components

- **`tool-manifest.toml`** — the single source of truth: the jankurai pin
  (`repo`/`rev`/`tag`/`version`/`semver`), per-profile score floors, and per-tool
  default modes.
- **`ops/render_tool_manifest.py`** (via `ops/render-tool-manifest.sh`) — the
  generator. It propagates the pin from the manifest into every family consumer
  (CI scripts, workflow envs, sandbox Dockerfiles, each repo's
  `required_tool_version`) and emits `generated/jankurai-pin.env`. `--check` is
  the drift lane that fails CI if any consumer diverged from the manifest.
- **`ops/install-jankurai.sh`** — installs the jeryu-owned binary to
  `~/.jeryu/bin/jankurai` (the host global; every CI lane resolves the auditor
  through `JERYU_JANKURAI_BIN`).
- **`policy/default-audit-policy.toml`** — the fallback policy the forge uses to
  force-score repos that carry no policy of their own.

## Data flow

`tool-manifest.toml` → generator → all family consumers (committed, drift-checked).
A version bump is a one-line manifest edit plus a regenerate; the drift lane
blocks a half-applied bump. The auditor binary itself lives in
`neverhuman/jankurai`; the forced-scoring runtime lives in the consuming repos
(forge in jeryu-core/jeryu-deploy, git guard in jeryu-release-ops, sandbox image
in jeryu-ci-runner).
