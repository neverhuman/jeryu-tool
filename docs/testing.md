# Testing

`jeryu-tool` is validated by its CI lanes; there is no product code to unit-test,
so the "tests" are the deterministic gates over the manifest and generator.

## Local gate

Run the full gate with one command:

```
just
```

Or run the lanes individually (same scripts CI runs — see `agent/proof-lanes.toml`):

- `just fast` — assert every family consumer's pin matches `tool-manifest.toml`
  (`ops/render-tool-manifest.sh --check`).
- `just check` — the manifest parses with a complete `[jankurai]` block, the
  generated pin is current, and every shell/python entrypoint is syntactically
  valid.
- `just score` — the pinned jankurai audit over this repo (writes `.jankurai/`).
- `just security` — gitleaks / actionlint / committed-`.env` checks.

`scripts/ci-local.sh` runs `fast` + `check`; `scripts/ci-doctor.sh` runs `score`;
`ops/git-hooks/pre-push` runs `fast` + `check` + `score` before any push.

## Drift test

The load-bearing test is the pin drift check. Editing `tool-manifest.toml` and
running `ops/render-tool-manifest.sh` must update every consumer; `--check` must
then be green. Reverting any one consumer by hand must make `--check` fail.
