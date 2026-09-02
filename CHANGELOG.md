# Changelog

## Unreleased

### Changed

- Made the exact `git.neverhuman.org` HTTPS repository identity authoritative
  for renderer protected-main and write-root custody, while retaining the
  governed Jankurai source spelling and generated consumer identity unchanged.
- Replaced Bearer-token environment injection with a same-inode askpass path:
  authenticated Git receives PAT bytes only over its credential pipe, and
  hostile origin, prompt, credential-file, and URL variants fail closed.
- Made the credential-bearing protected-main read independent of ambient proxy,
  TLS/CA, trace-output, and dynamic-loader overrides, with direct hosted routing,
  redirects disabled, and platform-CA certificate verification required.
- Made generated shell pin replacement in-place and byte-idempotent, preserving
  authored command order while rejecting ambiguous markers and strict-shell
  setup instead of silently rewriting malformed consumers.

## jeryu-tool-v5.1.0-split.5 — 2026-08-02

### Changed
- Rotated the governed auditor authority to the immutable Jankurai split.3
  repair, which scopes skipped-directory detection to repository-relative
  paths and prevents an ancestor directory named `target` from hiding source.
- Bound the exact source tree/archive, unchanged Cargo.lock and closed vendor
  closure, updated hermetic build context, and twice-reproduced OCI binary
  digest for the protected split.3 commit.

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
- Candidate probing and the required-check runner now execute only from
  root-owned, single-link, non-writable held bytes. Final digest and authority
  reauthentication rejects both pathname replacement and same-inode content
  drift before candidate publication.

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
