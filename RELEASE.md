# Release Playbook

This document describes the standard release process for Oatty.

## Scope

- Build and publish multi-platform release assets via GitHub Actions.
- Publish npm package via Trusted Publishing.
- Verify published artifacts and install path.

## Prerequisites

- npm Trusted Publisher configured for:
  - Package: `oatty`
  - Repository: `oattyio/oatty`
  - Workflow: `.github/workflows/release.yml`
- GitHub environment `oatty` configured (reviewers/restrictions as desired).
- Clean working tree on `main`.

## Versioning rules

- `package.json` version must match release tag exactly:
  - package version: `X.Y.Z`
  - git tag: `vX.Y.Z`
- `release.yml` enforces this and fails on mismatch.

## One-time sanity checks

```bash
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo install cargo-about --version 0.9.2 --features cli --locked
cargo fetch --locked
npm ci --ignore-scripts
npm run licenses:prepare
npm run test:licenses
npm pack --dry-run
```

## Release steps

1. Update npm package version.

```bash
npm version X.Y.Z --no-git-tag-version
git add package.json package-lock.json
git commit -m "chore(release): bump npm package to X.Y.Z"
git push
```

2. Create and push release tag.

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

3. Wait for workflow:

- `.github/workflows/release.yml`
  - Builds Linux/macOS/Windows artifacts.
  - Produces `SHA256SUMS`.
  - Produces GitHub build attestations.
  - Signs assets with keyless cosign.
  - Publishes release assets.
  - Publishes npm package with provenance.

## Required release assets

For tag `vX.Y.Z`, release must include:

- `SHA256SUMS`
- `oatty-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
- `oatty-vX.Y.Z-x86_64-apple-darwin.zip`
- `oatty-vX.Y.Z-aarch64-apple-darwin.zip`
- `oatty-vX.Y.Z-x86_64-pc-windows-msvc.zip`

## Post-release verification

1. Verify npm dist tags:

```bash
npm view oatty dist-tags
```

2. Verify fresh install:

```bash
npm i -g oatty@X.Y.Z
oatty --help
```

3. Verify artifact trust signals:

- Download `SHA256SUMS` and confirm each release asset checksum matches.
- Verify `.sig` and `.cert` files with `cosign verify-blob` for at least one platform artifact.

### Linux artifact verification example

Use this flow after downloading release assets for tag `vX.Y.Z`:

```bash
export VERSION="vX.Y.Z"
export REPO="oattyio/oatty"
export ASSET="oatty-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"

curl -LO "https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"
curl -LO "https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}.sig"
curl -LO "https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}.cert"
curl -LO "https://github.com/${REPO}/releases/download/${VERSION}/SHA256SUMS"

shasum -a 256 -c SHA256SUMS 2>/dev/null | grep "${ASSET}: OK"

cosign verify-blob \
  --signature "${ASSET}.sig" \
  --certificate "${ASSET}.cert" \
  --certificate-identity-regexp "^https://github.com/${REPO}/.github/workflows/release.yml@refs/(heads|tags)/.+$" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  "${ASSET}"
```

## Rollback and recovery

If npm publish fails:

- Fix configuration or workflow issue.
- Re-run failed GitHub workflow job.
- If package version already exists, increment version and release a new tag.

If release assets are incomplete:

- Fix release workflow/config.
- Publish a new patch release tag (`vX.Y.(Z+1)`).

Avoid deleting/reusing tags once consumed externally.

## Notes

- Official Windows target is validated in CI (`windows-latest`).
- Local macOS cross-checking to `x86_64-pc-windows-msvc` is not required for release success.

## Dependency licenses

Oatty-owned code is MIT licensed. Dependencies retain their upstream terms; MIT is selected when upstream offers a choice. Source-only sample provenance is documented in [schemas/README.md](schemas/README.md).

Before packing or publishing (including a manual `npm publish`), install Python 3.11+, Rust, Node, and the pinned license generator, then run:

```bash
cargo install cargo-about --version 0.9.2 --features cli --locked
cargo fetch --locked
npm ci --ignore-scripts
npm run licenses:prepare
npm run licenses:verify
npm run test:licenses
npm pack --dry-run
```

`--ignore-scripts` prevents the npm installer from downloading an existing release during preparation. The source-checkout `prepack` guard requires Python and rejects missing, changed, or stale materials on both `npm pack` and `npm publish`. Do not bypass it with `--ignore-scripts` when publishing. Installation of the published package does not require Python or Rust.

Use a registry release for ordinary npm installation. Direct npm Git URL installs are unsupported because their fresh checkout lacks the generated licensing materials. Use a registry release, or prepare a local source checkout as described above and run `npm pack`. The published tarball is designed for installation; repacking it requires the original source checkout and licensing tooling, which are not shipped in the tarball. Keep the prepack guard enabled.

`about.toml` declares the MIT-first license choices and the union of all four release targets. Generation uses `crates/cli/Cargo.toml` with default features and includes build and transitive dependencies, excluding dev-only dependencies. `cargo-about --locked --offline --fail` must resolve every selected crate and license text; the supplemental pass preserves upstream LICENSE, NOTICE, COPYRIGHT, and COPYING files recursively, including native AWS-LC/ring terms and encoding_rs attribution. npm dependencies are installed from the committed root lock and their own license files are preserved too. The website has no runtime npm dependencies in its separate package.

Every binary archive and npm tarball ships `LICENSE`, `THIRD-PARTY-NOTICES.txt`, `third-party-licenses/`, and `third-party-sources/`. The last directory contains the unchanged complete `nucleo-matcher` MPL-2.0 source as its original `.crate` archive, verified against `Cargo.lock`, with extraction and license instructions. Oatty's MIT license does not replace MPL or other dependency terms. If that dependency changes or another MPL dependency is added, review the source-distribution obligation and update the explicit policy; generation fails until addressed. Keep native required-notice assertions current when upgrading their crates. Generic SPDX templates are insufficient attribution: generation requires actual upstream notices or narrowly reviewed evidence in `scripts/license-supplements/reviewed.json`. Those committed supplements pin the exact crate version/checksum, immutable upstream revision, and notice/header checksum, including original declarations where upstream supplied no copyright notice. Unknown notice-free/template-only packages fail; do not invent an owner or year. Re-audit source headers and evidence when upgrading. Supplemental material is vendored, and license harvesting runs offline after fetching locked crate sources.

The generated materials and `.license-manifest.json` are ignored build outputs. Regenerate after changing dependency locks, workspace manifests, package metadata, licensing policy, or the generator. Review new license choices and upstream notices rather than adding an exception for unknown licensing. CI tests failure paths, real npm packing, all release archive layouts, exact executable/source preservation, and repeated deterministic generation. Release jobs consume the same verified material bundle; the archive packager verifies it again before creating the existing asset names/formats. macOS continues to use `ditto` ZIP packaging before notarization.

To package a local binary after preparation:

```bash
python3 scripts/package-release.py --binary target/release/oatty \
  --target x86_64-unknown-linux-gnu --version vX.Y.Z
```

When bumping the npm version, commit both `package.json` and `package-lock.json`.


## Native update archive retention

Native `oatty update` saves the entire downloaded ZIP or tar.gz before extraction and executable replacement, preserving the release's license notices and corresponding dependency source. The original asset filename is kept inside a new `release-*` directory for every attempt, including repeated or concurrent downloads of the same version. Oatty copies, flushes, and synchronizes the archive before publishing it without overwriting an existing file. A retention failure rejects installation. If a later extraction or installation step fails, the saved archive remains and its path is included in the error.

The root is the platform's local application data directory plus `oatty/releases`: Linux uses `$XDG_DATA_HOME` or `~/.local/share`; macOS uses `~/Library/Application Support`; Windows uses `%LOCALAPPDATA%`. Resolution and directory creation occur only when an archive has actually downloaded. There is no working-directory fallback when the platform data directory is unavailable. Checking for updates, finding no newer version, and npm-managed installations create no retention artifacts. A successful native update prints the retained path unless `--quiet` is set; confirmation behavior remains unchanged.

These archives are not automatically pruned and consume disk space until the user deletes them. They may be removed manually when no longer needed. Keep the updater's release-name validation aligned with the four supported archive targets if the release matrix changes. The `compression-zip-deflate` feature is required because the macOS and Windows release ZIPs use DEFLATE.

Offline updater tests use a custom release source, loopback HTTP, temporary installation paths, and compressed fixture archives. They verify replacement and retention without GitHub requests or replacing the running executable. Release validation also extracts archives produced by the actual packager through the pinned updater library, checking executable and licensing/source bytes.
