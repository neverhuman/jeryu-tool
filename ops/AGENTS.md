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

The sole premerge cycle-breaker is `bootstrap-jankurai-root-seal.sh`. It may
run only as root for the compiled PR7 topic, exact published head/tree,
content-addressed diagnostic qualification receipt, 15-minute-or-shorter
request, and one attempt for that head. It must retain the request, candidate,
receipt, broker, configs, and runner by descriptor or root-held copy; launch
only the fixed required-check command; and restore the exact protected
predecessor on success, failure, signal, or recovery. It grants no production
installation authority and must not become a general candidate broker.

## Proof lane

Changes under `ops/` run `just fast`, `just check`, `just score`, and
`just security`. Renderer changes additionally run
`ops/test-render-tool-manifest.sh` and
`ops/test-governed-jankurai-path.sh`. Bootstrap changes additionally run
`ops/test-bootstrap-jankurai-root-seal.sh`.
