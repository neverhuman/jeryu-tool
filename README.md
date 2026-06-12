# jeryu-tool

The **tool control plane** for the jeryu family. One repo, two jobs, no product
code:

1. **Audit toolchain** — owns the jankurai pin, floors, and forced scoring so
   every other repo and every agent sandbox audits identically (`tool-manifest.toml`).
2. **Reusable-code-tool registry** — the catalogue of shared crates / TS libs /
   React components / Vite plugins / shell libraries that replace copy-pasted
   code across repos, with adoption and LOC-saved bookkeeping
   (`tools-registry.toml` + `tasks/`). The forge "golden box" on `/repos` reads
   this. See `docs/tools-registry.md`.

Discovery of *new* tool candidates (cross-repo repeated-code clusters) lives in
the sibling `jeryu-tool-finder` repo, which files proposals back into this
registry.

## What lives here

| Path | Purpose |
|---|---|
| `tool-manifest.toml` | **Audit source of truth**: jankurai `{repo, rev, tag, version, semver}`, per-profile score floors, per-tool default modes. |
| `tools-registry.toml` | **Registry source of truth**: one `[[tool]]` per reusable tool — kind, status, adopting/candidate repos, realized + anticipated LOC saved. |
| `tasks/NNNN-*.toml` | Reusable-tool **build queue**: build-this-tool / migrate-these-repos work items. |
| `ops/registry_summary.py` | Validates the registry + tasks and computes the golden-box summary (`--check` runs in `just check`). |
| `ops/render-tool-manifest.sh` | Propagates the jankurai pin into every family consumer (CI scripts, workflow envs, sandbox Dockerfiles, per-repo `required_tool_version`). `--check` is the drift lane. |
| `ops/install-jankurai.sh` | Installs the jeryu-owned, pinned binary to `~/.jeryu/bin/jankurai` (the host global). |
| `policy/default-audit-policy.toml` | The jeryu-managed fallback policy used to force-score repos that carry no policy of their own. |
| `generated/jankurai-pin.env` | Generated sourced env (`JANKURAI_REPO/TAG/REV/VERSION/SEMVER`). Do not edit by hand. |
| `docs/tools.md` | The jankurai tool-compounding catalog + adoption guidance (live adoption data comes from the forge). |
| `docs/tools-registry.md` | The reusable-tool registry schema, lifecycle, and LOC-saved definition. |

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
