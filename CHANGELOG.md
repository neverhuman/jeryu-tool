# Changelog

## jeryu-tool-v5.1.0-split.8 — 2026-10-02

### Changed
- Pinned Jankurai `v1.6.11-deadlang-precision-split.5` (commit `aaed4a0`): forge-gated
  CI is read from `.jeryu/ci.toml` (schema "2", `provider = "jeryu"`) and credited only
  from the lane text the declaration resolves to, including `just` recipe
  dependencies; code-first contracts under generated zones satisfy HLT-007; HLT-047
  checks that agent instruction files point at `AGENTS.md`. Same `Cargo.lock` and
  vendor closure; updated source tree, archive digest, build context, and the
  reproduced OCI binary digest `d94d3e21…7f3ab`.

## jeryu-tool-v5.1.0-split.7 — 2026-10-01

### Changed
- Pinned Jankurai `v1.6.11-deadlang-precision-split.4` (commit `2b83122`), a
  rule-calibration release: dimensions that do not apply (no database) no longer
  score as failures, Build speed and Security reach the floor with generic
  signals, and HLT-001/006/008/030/038/042 are narrowed to real defects. Same
  `Cargo.lock` and vendor closure; updated source tree, archive digest, build
  context, and the twice-reproduced OCI binary digest `b05c03bc…f89103`.

## jeryu-tool-v5.1.0-split.6 — 2026-09-17

### Changed
- Moved the governed Jankurai install authority from the retired loopback
  forge to `git.neverhuman.org`: the pinned source is
  `https://git.neverhuman.org/git/jeryu/jankurai.git` (same tag, commit, tree,
  and archive digest), the installer binds receipts to the hosted
  `jeryu/jeryu-tool` manifest, and immutable-main protection is read back over
  HTTPS from the hosted forge.
- Receipt gates accept either the hosted or the legacy loopback manifest
  repository during the transition, so hosts keep passing until each is
  re-installed; the installer writes only the hosted authority.

## Unreleased

- Parse the `jeryu-toolctl` command line with clap, so `--help`, per-command
  `--help`, and `--version` describe the accepted schema; a rejected command
  line still prints the agent-facing repair block.
- Document the control binary in `docs/toolctl.md`: every subcommand and flag,
  including `emit-ensure-script` and the `JERYU_TOOL_GIT_ASKPASS` credential
  helper mode.
- Add bounded locked package-only compile, test, and coverage feedback.
- Add create-once exact-source repair receipts with replay, path, link,
  evidence, source-identity, schema, canonical-content, and replacement-race
  hostiles.
- Produce direct governed proof, security/SBOM, coverage, contract, witness,
  duplication, and ratchet evidence before the final score audit.
- Document the real no-database and no-paid-runtime boundary without inventing
  product migrations or spend state.

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
- Bound consumer `jankurai()` wrappers to the exact executable selected by the
  receipt verifier, and reject ambiguous or alternate execution shapes instead
  of validating one binary and running another.
- Resolve executable files independently of shell functions, so a generated
  `jankurai()` wrapper cannot shadow the authenticated binary while real PATH
  shadowing remains fail-closed.
- Require receipt-bound auditor executables to remain single-link files, so an
  alternate hard-link name cannot create ambiguous custody after installation.
- Render one canonical verifier-backed `jankurai()` wrapper into every consumer
  library, so an inherited shell function cannot pass executable verification
  and then intercept the audit command.

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
