# `jeryu-toolctl`

`jeryu-toolctl` is the locked, offline Rust control binary behind the thin
`ops/*.sh` entrypoints. Every command reads the manifests under one checkout,
named by the required global flag:

```
jeryu-toolctl --tool-root PATH <COMMAND> [FLAGS]
```

The command line is parsed by clap, so `jeryu-toolctl --help`,
`jeryu-toolctl --tool-root . <COMMAND> --help`, and `--version` always describe
the accepted schema. A rejected command line still prints the agent-facing
repair block (`purpose`, `reason`, `common_fixes`, `docs_url`, `repair_hint`)
after the parse error, so a refusal is machine-readable wherever it comes from.

Prefer the wrappers over the binary: they pin `--locked --offline` and pass the
canonical `--tool-root`.

## `registry-summary`

Validates `tools-registry.toml` and every `tasks/NNNN-*.toml` against the closed
schema, then reports.

| flag | meaning |
|---|---|
| `--check` | Print the one-line verdict instead of the JSON summary. |

Without `--check` the whole summary is printed as pretty JSON — tool rows,
per-status counts, open tasks, and anticipated/realized LOC saved. That is the
document the forge serves as `GET /api/v1/tools/registry/summary`. With
`--check` only `registry ok: ...` is printed; either way an invalid registry or
task file fails the command. Wrapper: `ops/registry-summary.sh` (run with
`--check` in `just check`). Schema and lifecycle: `docs/tools-registry.md`.

## `render-tool-manifest`

Renders the governed jankurai identity from `tool-manifest.toml` into every
consumer that claims it.

| flag | meaning |
|---|---|
| `--check` | Validate the rendered bytes without writing them. |
| `--candidate` | Qualify a candidate head instead of protected main. Valid only with `--check`, and refused outright in release CI. |
| `--repo NAME` | Canonical repository to render into; repeat to select several. Unscoped (no `--repo`) is forced check-only. |
| `--repo-root NAME=/absolute/path` | The physical checkout for one repository. In write mode it must be the canonical checkout under the family root. |
| `--expected-head NAME=SHA` | The 40-hex head that repository must already be at; a write refuses any other head. |
| `--family-root PATH` | Directory holding the family checkouts. Defaults to `JERYU_FAMILY_ROOT`, else the tool root's parent. Write mode accepts only the canonical family root. |

Wrapper: `ops/render-tool-manifest.sh`; hostile cases are proven by
`ops/test-render-tool-manifest.sh`.

## `emit-ensure-script`

Prints, to stdout, the `require-jankurai` verifier with the current pin baked
in: the static `ops/render-assets/require-jankurai.sh` template plus a generated
pin block bound to `generated/jankurai-pin.env`. It takes no arguments and
writes no files — consumers install what it prints and never hand-edit the
result. Rendering it into a consumer is `render-tool-manifest`'s job; this
command exists so a caller can inspect or install the verifier on its own.

## Askpass mode (`JERYU_TOOL_GIT_ASKPASS=1`)

The binary has one mode that is not a subcommand. When the environment carries
`JERYU_TOOL_GIT_ASKPASS=1`, the process answers a git credential prompt instead
of parsing a command line:

- it takes exactly **one** argument, the prompt git itself emits, and rejects
  any other argument count;
- the prompt must be exactly
  `Password for 'https://git@git.neverhuman.org/git/jeryu/<canonical-repo>.git': `
  — any other host, owner, or repository name is refused, so the token cannot be
  handed to a foreign remote;
- the answer is read from the file named by `JERYU_FORGE_TOKEN_FILE`, which must
  be an absolute, already-canonical path to an unlinked-once regular file owned
  by the caller with mode exactly `0600`, non-empty and at most 16 KiB, whose
  identity is re-checked after the open and after the read;
- on success the token is printed on stdout and nothing is written to stderr; on
  refusal it exits non-zero and prints nothing at all, so a prompt failure never
  leaks the reason into git's output.

`render-tool-manifest` sets this mode on itself — it points git's `GIT_ASKPASS`
at its own held executable while fetching a family remote. Nothing else should
invoke it: it is a credential helper, not a user-facing command, which is why it
is an environment mode rather than a subcommand that could be typed by mistake.
Custody is proven by `hosted_askpass_is_path_only_and_prompt_bound` in
`crates/jeryu-tool-control/tests/control_cli.rs`.
