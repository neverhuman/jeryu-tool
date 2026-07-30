# Governed release process

`jeryu-tool` ships no product binary of its own. It releases the immutable
control-plane identity used to build and install the internal Jankurai auditor.
The canonical repository is local-forge `jeryu/jeryu-tool`; product and public
hub releases are separate.

`VERSION` is the repository release identity and must equal the next unused
immutable local-forge tag. `CHANGELOG.md` carries the matching release notes.
`tool-manifest.toml` is the sole source for Jankurai source, build, and binary
identity.

## Release gate

Before the protected PR is approved or merged:

- run `just fast`, `just check`, `just score`, `just security`, and `just artifact-support`
- require the forge's `jankurai/proof` and exact-head `jeryu-tool/required`
  checks plus one independent approval
- qualify the exact manifest candidate only in a temporary root; its receipt
  must say `test_mode=true`, `source.verification=diagnostic-candidate`, and
  `governance.status=diagnostic-candidate`
- confirm source tree/archive, Cargo.lock, closed vendor/config inventories,
  builder image/linker/glibc, Rust/Cargo/target, build context, and binary
  SHA-256 evidence all match `tool-manifest.toml`
- verify renderer custody, offline-fetch refusal, receipt tamper, interrupted
  install, corrupt rollback object, restore, and wrong-identity tests
- confirm the immutable source and prior content-addressed binary provide the
  release backup, and that rollout monitoring is active before host install

### PR7 premerge root-seal bootstrap

PR7 changes the auditor whose proof is itself required before merge. The sole
cycle breaker is `ops/bootstrap-jankurai-root-seal.sh`; it does not install or
promote the candidate. A distinct reviewer must first approve the complete
source series and a fresh qualification must materialize the exact manifest
binary plus its content-addressed `diagnostic-candidate` receipt. Host custody
must be empty before the root operator starts.

The operator supplies one ordinary, physical, single-link JSON request with
schema `jeryu.jankurai-root-seal-bootstrap/v1`. Unknown fields are rejected.
It binds:

- a unique 64-hex attempt ID and the compiled PR7 topic ref
- the clean checkout's exact published head and tree
- the candidate's absolute physical path
- the qualification receipt's absolute path and content-addressed SHA-256
- integer creation and expiry epochs, with a maximum 900-second lifetime

The bootstrap cannot run from this checkout in production. First land, tag,
bind, and install the SplitOps correction that provides
`/usr/local/libexec/jain/splitctl` and its closed
`native-build-tools-installer.config.json`; there is no checkout fallback.
Using that installed broker, root privately materializes the independently
reviewed PR7 head from the fixed Jeryu forge, installs only its
`ops/bootstrap-jankurai-root-seal.sh` as
`/usr/local/libexec/jain/bootstrap-jankurai-root-seal` mode 0500 and its
`generated/jankurai-pin.env` beside it mode 0400, then writes the closed
root:root mode-0600 `jeryu-tool-root-seal.config.json`. That config binds the
fixed remote/topic, reviewed commit/tree, both installed paths and digests,
state root, predecessor digest, installed SplitOps config/broker, and installed
token path. The temporary provisioning materialization is removed before use.

The installed entrypoint descriptor-retains itself, the pin, both authority
configs, the SplitOps broker, and token. It authenticates the fixed published
PR7 ref into private custody and compares its own and the pin's held bytes with
the exact Git blobs before sourcing the pin. It then verifies every
qualification identity, the diagnostic/no-protected-main governance fields,
the exact local and forge ref/head/tree, protected predecessor SHA-256, and
both broker configs. Before any candidate probe, it copies the candidate into
root-owned mode-0500 single-link custody, independently reauthenticates the
copy, and runs the version probe only from those held bytes.

The held installed SplitOps broker verifies exact protected-main policy and
materializes only fixed `veox/jain-split-ops` protected main with the exact
immutable tag/commit named by its installed config. Main must still equal that
tagged commit and tree. The bootstrap executes only the resulting root-owned
non-writable runner and helper tree. Caller checkout paths, `origin`, `HEAD`,
refs, remotes, helpers, configs, or self-consistent substitutes never select
privileged bytes.

Immediately before publication, the wrapper reauthenticates every installed
authority descriptor, candidate, receipt, and held runner
pathname/inode/content identity plus both published authorities. Path
replacement and same-inode content drift are terminal. The
recovery journal also retains the qualification receipt and byte-exact
predecessor backups. The transaction consumes the head's sole attempt, changes
only the auditor binary and the two `jankurai_sha256` config fields, and
launches the fixed `jeryu jeryu-tool <head> <canonical-root>
jeryu-tool/required` command as the reserved host-CI parent identity. Callers
cannot supply a command, broker, runner, ref family, install root, state root,
predecessor, pin, or clock.

Normal completion, command failure, `HUP`, `INT`, and `TERM` restore the exact
predecessor binary and config bytes and remove the held runner materialization
before return. The child receives a
parent-death signal; if the supervisor is killed or the machine stops between
atomic publications, the durable active marker makes the next invocation
restore before it can consume another request. The attempt remains spent after
any failure. The content-addressed result records the check exit code and
`predecessor_restored=true`; it never turns a failed check green.

This authority is intentionally narrower than installation: the candidate
receipt remains diagnostic, the governed home path is untouched, production
installation remains forbidden until protected fast-forward merge, and the
ordinary release broker continues to reject caller receipt authority.

After protected fast-forward merge, cut the immutable tag named by `VERSION` at
the merged commit. Then run `ops/install-jankurai.sh` from a clean checkout of
that exact protected main. The installer reads back immutable-main protection,
materializes the exact local-forge source and closed vendor inventory, and
builds as a non-root user in the digest-pinned read-only OCI image with
`--network none`. It verifies all pinned digests, atomically installs the
binary, and emits the production receipt. Only after that receipt exists may
consumer PRs require the governed host binary.

The family-wide renderer check becomes green as the protected consumer PRs
land. It is the rollout monitor: any source, version, or digest drift fails.
Rate limiting and abuse controls are N/A because this repository exposes no
public runtime surface.

## Provenance and evidence

The installation receipt binds the local source remote, immutable Jankurai tag
and commit, Git tree/archive and Cargo.lock checksums, Rust and Cargo versions,
target triple, builder image and native tool identities, vendor/config/context
digests, exact environment/command/remaps, network and privilege isolation,
binary SHA-256 and version output, absolute installation path,
previous-binary digest, and exact protected `jeryu-tool` manifest
commit/tree/bytes. Test receipts are never release authority.

## Rollback

The installer stores the previous binary under its SHA-256 and re-hashes it
before trust. A failed post-rename transaction restores through a verified
staging file and verifies the final target digest. A governed rollback uses a
new protected manifest PR pinning the prior immutable Jankurai source/binary,
then repeats render, merge, installation, and sandbox build. Never move an
immutable tag; publish the next repair tag.
