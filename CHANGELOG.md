# Changelog

## jeryu-tool-v5.1.0-split.1 — 2026-07-15

### Added
- New family repo `jeryu-tool`: the audit control plane.
- `tool-manifest.toml` — single source of truth for the jankurai pin, per-profile
  score floors, and per-tool default modes.
- `ops/render-tool-manifest.sh` (+ `render_tool_manifest.py`) — propagates the pin
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
