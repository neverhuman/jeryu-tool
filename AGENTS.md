# jeryu-tool Agent Instructions

This is the **audit control plane** for the Jeryu split family.

Before editing, read `README.md` and `tool-manifest.toml`.

`tool-manifest.toml` is the single source of truth for the jankurai toolchain.
Never hardcode a jankurai rev/tag/version anywhere in the family — change it here
and run `ops/render-tool-manifest.sh`. `generated/jankurai-pin.env` is rendered,
not authored. Keep this repo lightweight: manifest, generator, installer, default
policy, and docs only. The auditor binary itself lives in `neverhuman/jankurai`;
enforcement runtime lives in the consuming repos (forge in jeryu-core/jeryu-deploy,
guard in jeryu-release-ops, sandbox in jeryu-ci-runner).
