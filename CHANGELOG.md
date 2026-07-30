# Changelog

## jeryu-tool-v5.1.0-split.4 — 2026-07-29

### Changed
- Replaced the ambient-host Jankurai build with one digest-pinned, non-root,
  network-disabled OCI build over the immutable source and a closed,
  checksum-inventoried Cargo vendor closure.
- Rotated the governed Jankurai binary digest without moving its immutable
  source tag. Host and sandbox consumers must render this same authority; a
  consumer-local binary identity remains invalid.
- Installation receipts now bind the builder image, linker and glibc identity,
  vendor and Cargo configuration inventories, exact build environment and
  command, canonical path remaps, and complete build-context digest.
- Added the PR7-only, root-held premerge seal transaction. It consumes one
  short-lived exact-head request, binds the independently qualified diagnostic
  receipt, runs only the fixed required-check command, and restores the exact
  protected predecessor broker/config bytes on completion or recovery without
  granting production installation authority.

## jeryu-tool-v5.1.0-split.1 — 2026-07-15

### Added
- New family repo `jeryu-tool`: the audit control plane.
- `tool-manifest.toml` — single source of truth for the jankurai pin, per-profile
  score floors, and per-tool default modes.
- `ops/render-tool-manifest.sh` (+ the locked Rust `jeryu-toolctl`) — propagates the pin
  into all ~40 family consumers; `--check` is the drift lane.
- `ops/install-jankurai.sh` — installs the jeryu-owned binary to `~/.jeryu/bin/jankurai`.
- `policy/default-audit-policy.toml` — fallback policy for forced scoring of
  unconfigured repos.
- `docs/tools.md` — tool-compounding catalog and adoption guidance.

### Changed
- Governed internal Jankurai is pinned to the protected 1.6.11 correction tag,
  exact source/build identity, and reproducible binary digest.
- Installation now requires local-forge source, a locked offline build, exact
  protected manifest governance, atomic replacement, verified rollback bytes,
  and a content-addressed receipt.
- Family rendering is explicit-worktree and custody checked; generated
  consumers bind the complete immutable source and binary identity.
