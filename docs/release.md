# Release

`neverhuman/jeryu-deploy` publishes all signed release artifacts for the family;
`jeryu-tool` ships no binary of its own — it governs the jankurai toolchain pin.

Version source is `VERSION` plus the split tag recorded in `repos.manifest.toml`.
Release notes are recorded in `CHANGELOG.md`.

## Release gate

Before a release or split tag is promoted:

- run `just fast`, `just check`, `just score`, `just security`, and `just artifact-support`
- confirm `ops/render-tool-manifest.sh --check` is green (every family consumer
  matches `tool-manifest.toml`)
- confirm checksum, provenance, SBOM, and cosign evidence for any artifacts the
  pinned jankurai version is bumped to
- confirm backups or reproducible source inputs exist for rollback
- confirm monitoring is active for the pinned-version rollout (the drift lane is
  the live monitor: it fails when any consumer diverges from the manifest)
- confirm rate limit or abuse controls are configured for public surfaces (this
  control-plane repo exposes no public runtime surface, so this is N/A by design)

## Provenance & evidence

The only artifact this repo governs is the jankurai binary pin in
`tool-manifest.toml` (`repo`, `rev`, `tag`, `version`, `semver`). A pin bump is
evidence-bearing: the rev is an immutable commit SHA, and `ops/install-jankurai.sh`
re-verifies `jankurai --version` against the pinned `version` string before
placing the binary at `~/.jeryu/bin/jankurai`.

## Rollback

Rollback restores the previous `[jankurai]` block in `tool-manifest.toml`, then
re-runs `ops/render-tool-manifest.sh` (re-propagates the old pin) and
`ops/install-jankurai.sh` (re-installs the old host binary); rebuild the agent
sandbox image to restore its baked copy. Do not overwrite split tags — publish a
new repair tag.
