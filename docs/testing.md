# Testing

`jeryu-tool` is validated by unit and integration tests for its Rust registry
and renderer plus deterministic gates over the manifest and generated output.

## Local gate

Run the full gate with one command:

```
just
```

Or run the lanes individually (same scripts CI runs — see `agent/proof-lanes.toml`):

- `just fast` — assert every family consumer's pin matches `tool-manifest.toml`
  (`ops/render-tool-manifest.sh --check`).
- `just check` — the manifest, registry tools, and task files match their exact
  closed key sets and field relationships; the generated pin is current; Rust
  tests and warnings-denied Clippy pass locked and offline; and every shell
  entrypoint is syntactically valid.
- `just score` — the pinned jankurai audit over this repo (writes `.jankurai/`).
- `just security` — gitleaks / actionlint / committed-`.env` checks.

`scripts/ci-local.sh` runs `fast` + `check`; `scripts/ci-doctor.sh` runs `score`;
`ops/git-hooks/pre-push` runs `fast` + `check` + `score` before any push.

## Agent-readable control errors

`jeryu-toolctl` maps its closed `Usage`, `Registry`, and `Renderer` error
variants to one typed repair record. Every failure prints non-empty `purpose`,
`reason`, `common_fixes`, `docs_url`, and `repair_hint` fields after the exact
source error. `common_fixes` is a pipe-delimited closed list, `docs_url` points
to repository-local guidance, and `repair_hint` names the command or lane to
rerun. `every_control_error_has_closed_agent_repair_guidance` proves the
contract exhaustively; `registry_check_and_closed_arguments` proves the
rendered failure at the process boundary.

Rerun the focused regression with:

```
cargo test --locked --offline -p jeryu-tool-control \
  every_control_error_has_closed_agent_repair_guidance
cargo test --locked --offline -p jeryu-tool-control \
  registry_check_and_closed_arguments
```

## Drift test

The load-bearing test is the pin drift check. Editing `tool-manifest.toml` and
running `ops/render-tool-manifest.sh` must update every consumer; `--check` must
then be green. Reverting any one consumer by hand must make `--check` fail.

Write-mode hostiles additionally prove that only
`/home/ubuntu/jain-split/jeryu-split/<repo>` can be mutated, the repository has
one ordinary local `.git` directory and no additional registered worktrees,
checkout-local Git execution is disabled, and the credential is read only from
an absolute stable mode-0600 owner-held single-link regular file. Authentication
is supplied only to the fixed `/usr/bin/git`, repository-independent
local-forge protected-main read, after all local checkout checks have passed.
