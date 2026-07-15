# jeryu-tool Agent Instructions

This is the **tool control plane** for the Jeryu split family: the jankurai
audit toolchain **and** the reusable-code-tool registry.

Before editing, read `README.md`, `tool-manifest.toml`, and (for the registry)
`docs/tools-registry.md`.

`tool-manifest.toml` is the single source of truth for the jankurai toolchain.
Never hardcode a jankurai rev/tag/version anywhere in the family — change it here
and run `ops/render-tool-manifest.sh`. `generated/jankurai-pin.env` is rendered,
not authored.

`tools-registry.toml` + `tasks/` are the single source of truth for reusable
tools (shared crates / TS / React / shell libs) and their build queue. Discovery
of new candidates lives in `jeryu-tool-finder`, which files proposals here;
`ops/registry_summary.py --check` validates them in `just check`. The forge
golden box on `/repos` reads the registry via `GET /api/v1/tools/registry/summary`.

Keep this repo lightweight: manifests, generators, installer, default policy,
the registry, and docs only — no product code (Python is allowed under `ops/`).
The governed auditor source lives in the local-forge `jeryu/jankurai` repository;
the separate public hub release is not the Jeryu CI authority. Reusable tools live
in their canonical home repos; enforcement runtime lives in the consuming repos (forge in
jeryu-core/jeryu-deploy, guard in jeryu-release-ops, sandbox in jeryu-ci-runner).
