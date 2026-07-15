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
| `tool-manifest.toml` | **Audit source of truth**: local-forge commit/tag, source tree/archive and lock digests, exact version, reproducible build environment, binary digest, per-profile score floors, and per-tool default modes. |
| `tools-registry.toml` | **Registry source of truth**: one `[[tool]]` per reusable tool — kind, status, adopting/candidate repos, realized + anticipated LOC saved. |
| `tasks/NNNN-*.toml` | Reusable-tool **build queue**: build-this-tool / migrate-these-repos work items. |
| `ops/registry_summary.py` | Validates the registry + tasks and computes the golden-box summary (`--check` runs in `just check`). |
| `ops/render-tool-manifest.sh` | Propagates the jankurai pin into every family consumer (CI scripts, workflow envs, sandbox Dockerfiles, per-repo `required_tool_version`). `--check` is the drift lane. |
| `ops/install-jankurai.sh` | Verifies the immutable local-forge source, builds from the lockfile offline, atomically installs `/home/ubuntu/.jeryu/bin/jankurai`, preserves rollback content, and writes a content-addressed receipt. |
| `ops/qualify-jankurai-candidate.sh` | Builds the exact premerge candidate into a temporary root and persists a content-addressed diagnostic receipt; it can never target the governed host root. |
| `ops/test-install-jankurai.sh` | Proves identity-bound idempotency and safe refusal for receipt tamper, external sources/redirects, wrong digests, wrong versions, offline cache misses, interrupted installs, and rollback faults. |
| `ops/test-render-tool-manifest.sh` | Proves unscoped rendering is check-only and write mode rejects missing custody, dirty roots, wrong origins, and heads not based on current protected main. |
| `policy/default-audit-policy.toml` | The jeryu-managed fallback policy used to force-score repos that carry no policy of their own. |
| `generated/jankurai-pin.env` | Generated source/build/binary identity, including commit/tag/tree, archive/lock/binary digests, toolchain, target, and exact version. Do not edit by hand. |
| `docs/tools.md` | The jankurai tool-compounding catalog + adoption guidance (live adoption data comes from the forge). |
| `docs/tools-registry.md` | The reusable-tool registry schema, lifecycle, and LOC-saved definition. |

## Upgrading jankurai (the whole family at once)

1. Edit `[jankurai]` in `tool-manifest.toml`.
2. Commit the manifest update, then run `ops/render-tool-manifest.sh --repo <name> --repo-root <name>=<clean-path> --expected-head <name>=<40-hex-sha>` for every explicitly claimed root. Unscoped invocation is check-only; writes require the exact handed-off clean canonical local-forge checkout based on current protected `main`.
3. Land every consumer and this manifest through exact-head protected PRs, then require `ops/render-tool-manifest.sh --check` to be drift-free.
4. Host: `ops/install-jankurai.sh` rebuilds offline and atomically installs only when source, build, binary, path, and receipt all match.
5. Sandbox: rebuild the agent-sandbox image from the same identity and verify its baked binary digest before any network-isolated lane runs.

`ops/render-tool-manifest.sh --check` fails CI if any consumer drifted from the
manifest, so a half-done bump can never ship.

The local protected PR gate may qualify an exact premerge candidate only in a
temporary root with `test_mode=true` and a content-addressed receipt. That
diagnostic candidate never grants installation or merge authority. Once an
independently reviewed candidate is installed, the same gate automatically
requires `/home/ubuntu/.jeryu/bin/jankurai` plus its production receipt (or can
be forced fail-closed with `JERYU_TOOL_REQUIRE_GOVERNED_HOST=1`). The GitHub
workflow is a static, non-authoritative mirror because it cannot reach the
100%-local forge.

## Relationship to standalone jankurai

This repo is **additive**. People auditing their own code keep using jankurai's
own installer and per-repo `agent/audit-policy.toml` exactly as before — nothing
here changes the auditor's CLI or on-disk formats. `jeryu-tool` only governs how
the *jeryu forge* pins, installs, forces, and reports on jankurai.
