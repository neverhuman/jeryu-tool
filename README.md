# jeryu-tool

The **audit control plane** for the jeryu family. One repo owns the jankurai
toolchain so every other repo — and every agent sandbox — gets the same auditor,
the same pin, the same floors, and the same forced scoring, without managing any
of it itself.

## What lives here

| Path | Purpose |
|---|---|
| `tool-manifest.toml` | **The single source of truth**: jankurai `{repo, rev, tag, version, semver}`, per-profile score floors, per-tool default modes. |
| `ops/render-tool-manifest.sh` | Propagates the pin from the manifest into every family consumer (CI scripts, workflow envs, sandbox Dockerfiles, per-repo `required_tool_version`). `--check` is the drift lane. |
| `ops/install-jankurai.sh` | Installs the jeryu-owned, pinned binary to `~/.jeryu/bin/jankurai` (the host global). |
| `policy/default-audit-policy.toml` | The jeryu-managed fallback policy used to force-score repos that carry no policy of their own. |
| `generated/jankurai-pin.env` | Generated sourced env (`JANKURAI_REPO/TAG/REV/VERSION/SEMVER`). Do not edit by hand. |
| `docs/tools.md` | The tool-compounding catalog + adoption guidance (live adoption data comes from the forge). |

## Upgrading jankurai (the whole family at once)

1. Edit `[jankurai]` in `tool-manifest.toml`.
2. `ops/render-tool-manifest.sh` — propagates into all ~40 consumer sites.
3. Host: `ops/install-jankurai.sh` re-installs `~/.jeryu/bin/jankurai`.
4. Sandbox: rebuild the agent-sandbox image (it bakes the auditor; it is `--network none`).

`ops/render-tool-manifest.sh --check` fails CI if any consumer drifted from the
manifest, so a half-done bump can never ship.

## Relationship to standalone jankurai

This repo is **additive**. People auditing their own code keep using jankurai's
own installer and per-repo `agent/audit-policy.toml` exactly as before — nothing
here changes the auditor's CLI or on-disk formats. `jeryu-tool` only governs how
the *jeryu forge* pins, installs, forces, and reports on jankurai.
