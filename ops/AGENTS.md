# Jeryu Tool operations guidance

## Owns

`ops/` owns the thin shell entrypoints around the locked Rust control binary,
the governed Jankurai installer/verifier, and the locally reproducible CI
lanes. `tool-manifest.toml` remains the pin authority; the renderer may update
only its closed canonical consumer allow-list.

## Forbidden

Do not select an ambient auditor, accept caller release authority, render an
unclaimed or dirty write root, add Python execution, weaken secret scanning,
or treat the diagnostic GitHub mirror as a release path.

## Proof lane

Changes under `ops/` run `just fast`, `just check`, `just score`, and
`just security`. Renderer changes additionally run
`ops/test-render-tool-manifest.sh` and
`ops/test-governed-jankurai-path.sh`.
