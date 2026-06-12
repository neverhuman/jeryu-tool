# Changelog

## Unreleased

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
