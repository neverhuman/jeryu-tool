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
- confirm source tree/archive, Cargo.lock, Rust/Cargo/target, and binary SHA-256
  evidence all match `tool-manifest.toml`
- verify renderer custody, offline-fetch refusal, receipt tamper, interrupted
  install, corrupt rollback object, restore, and wrong-identity tests
- confirm the immutable source and prior content-addressed binary provide the
  release backup, and that rollout monitoring is active before host install

After protected fast-forward merge, cut the immutable tag named by `VERSION` at
the merged commit. Then run `ops/install-jankurai.sh` from a clean checkout of
that exact protected main. The installer reads back immutable-main protection,
rebuilds from the local forge with Cargo offline, verifies all pinned digests,
atomically installs the binary, and emits the production receipt. Only after
that receipt exists may consumer PRs require the governed host binary.

The family-wide renderer check becomes green as the protected consumer PRs
land. It is the rollout monitor: any source, version, or digest drift fails.
Rate limiting and abuse controls are N/A because this repository exposes no
public runtime surface.

## Provenance and evidence

The installation receipt binds the local source remote, immutable Jankurai tag
and commit, Git tree/archive and Cargo.lock checksums, Rust and Cargo versions,
target triple, offline build mode, binary SHA-256 and version output, absolute
installation path, previous-binary digest, and exact protected `jeryu-tool`
manifest commit/tree/bytes. Test receipts are never release authority.

## Rollback

The installer stores the previous binary under its SHA-256 and re-hashes it
before trust. A failed post-rename transaction restores through a verified
staging file and verifies the final target digest. A governed rollback uses a
new protected manifest PR pinning the prior immutable Jankurai source/binary,
then repeats render, merge, installation, and sandbox build. Never move an
immutable tag; publish the next repair tag.
