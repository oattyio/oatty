# Release Playbook

This document describes the standard release process for Oatty.

## Scope

- Build and publish multi-platform release assets via GitHub Actions.
- Publish four platform packages, then the npm launcher, via Trusted Publishing.
- Verify published artifacts and install path.

## Prerequisites

- npm publish ownership for `oatty` and all four public packages in the `@oattyio` scope:
  `@oattyio/cli-darwin-x64`, `@oattyio/cli-darwin-arm64`, `@oattyio/cli-linux-x64`, and `@oattyio/cli-win32-x64`.
- A separate npm Trusted Publisher for **each of the five packages**, with GitHub organization `oattyio`, repository `oatty`, workflow filename `release.yml` (not its full path), and environment `oatty`. Permit direct `npm publish`; staging permission alone does not authorize this workflow. The workflow retains `id-token: write` and publishes with provenance.
- GitHub environment `oatty` configured (reviewers/restrictions as desired).
- Clean working tree on `main`.

New trusted publishers default to staging permission. A newly configured publisher must complete its first successful publish within two days; recreate an expired configuration before retrying. These settings belong to npm package maintainers and are not configured by this repository. See [npm's trusted publisher setup](https://docs.npmjs.com/trusted-publishers/).

For scoped names that do not exist yet, a maintainer with publish access to `@oattyio` must establish the package before its package-level publisher can be configured. Use an authenticated maintainer session with 2FA for an initial publication, or stage a reviewed platform tarball with `npm stage publish <tarball> --access public`. Staging a new name creates a public `0.0.0-stage` placeholder; the intended version remains unavailable until a maintainer approves it with 2FA. Configure the package's publisher, approve the intended version, then rerun the release job. Do not publish the launcher while any pinned platform version is only staged. See [npm's staged publishing process](https://docs.npmjs.com/staged-publishing/).

The automated helper uses direct publication. A stage-only publisher or a package requiring staged publication needs a maintainer decision and an explicit staged-release process: approve all four platform versions before the launcher. The workflow does not silently switch publication modes or provision a long-lived npm token.

## Versioning rules

- Cargo's workspace package version, the npm launcher version, and all four exact optional-dependency pins must agree:
  - package version: `X.Y.Z`
  - git tag: `vX.Y.Z`
- Keep the root `package-lock.json` version, root metadata, and four platform records synchronized too. The lock includes complete platform metadata so `npm ci --omit=optional --ignore-scripts` works before those versions exist on npm. Avoid registry-dependent lock regeneration for an unpublished version.
- `release.yml` checks Cargo/npm/tag consistency before building; packaging and legal validation reject mismatched platform pins and lock metadata.

## One-time sanity checks

```bash
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo install cargo-about --version 0.9.2 --features cli --locked
cargo fetch --locked
npm ci --omit=optional --ignore-scripts
npm run licenses:prepare
npm run test:licenses
python3 -m unittest discover -s scripts/tests -p test_npm_distribution.py -v
npm pack --dry-run
```

## Release steps

1. Update the Cargo workspace version, npm version, all four optional-dependency pins, and corresponding root lock metadata together. Keep the platform versions exact (no `^` or `~`). Prepare legal materials again and run the checks above.

```bash
git add Cargo.toml Cargo.lock package.json package-lock.json
git commit -m "chore(release): bump version to X.Y.Z"
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
  - Preserves signed executable bytes in platform npm tarballs and verifies global/local installs with scripts disabled.
  - Produces `SHA256SUMS`.
  - Produces GitHub build attestations.
  - Signs assets with keyless cosign.
  - Publishes release assets.
  - Publishes all four platform npm packages with provenance, confirms their registry versions, then publishes the launcher.

## Required release assets

For tag `vX.Y.Z`, release must include:

- `SHA256SUMS`
- `oatty-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
- `oatty-vX.Y.Z-x86_64-apple-darwin.zip`
- `oatty-vX.Y.Z-aarch64-apple-darwin.zip`
- `oatty-vX.Y.Z-x86_64-pc-windows-msvc.zip`
- `oattyio-cli-darwin-x64-X.Y.Z.tgz`
- `oattyio-cli-darwin-arm64-X.Y.Z.tgz`
- `oattyio-cli-linux-x64-X.Y.Z.tgz`
- `oattyio-cli-win32-x64-X.Y.Z.tgz`

Each platform tarball joins the existing archives in checksum, cosign, and build-provenance coverage. The launcher tarball is packed from the tagged source and verified legal bundle in the npm publication job.

## Post-release verification

1. Verify npm dist tags:

```bash
npm view oatty dist-tags
npm view @oattyio/cli-darwin-x64@X.Y.Z version
npm view @oattyio/cli-darwin-arm64@X.Y.Z version
npm view @oattyio/cli-linux-x64@X.Y.Z version
npm view @oattyio/cli-win32-x64@X.Y.Z version
```

2. Verify fresh install:

```bash
npm i -g oatty@X.Y.Z --ignore-scripts --include=optional
oatty --version
oatty --help
oatty update --check
```

The updater must identify the installation as npm-managed and direct updates through npm. Also install locally in a clean temporary project with `npm i oatty@X.Y.Z --ignore-scripts --include=optional` and run `npx oatty --version`. Check each supported OS/architecture; Linux packages require glibc. If optional dependencies were omitted, reinstall with `--include=optional`; `npm rebuild` is not needed because there is no downloader hook.

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
- The helper checks every package independently: an existing platform version is skipped only when its registry integrity matches the prepared tarball. Authentication and network failures abort rather than being treated as absent versions. Identical retries complete missing platform packages before publishing the launcher; different bytes require a new version and release tag.

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
npm ci --omit=optional --ignore-scripts
npm run licenses:prepare
npm run licenses:verify
npm run test:licenses
npm pack --dry-run
```

`--omit=optional` avoids fetching unpublished platform packages during source preparation; `--ignore-scripts` keeps dependency lifecycle scripts disabled. The source-checkout `prepack` guard requires Python and rejects missing, changed, or stale materials on both `npm pack` and `npm publish`. Do not bypass it when packing the source checkout. `scripts/package-npm.py` verifies the same inputs, then stages a launcher manifest without development scripts. Published packages require Node, with no Python, Rust, or lifecycle scripts needed during installation or launch.

Use a registry release for ordinary npm installation. Direct npm Git URL installs are unsupported because their fresh checkout lacks the generated licensing materials. To create release artifacts, prepare the tagged source checkout as described above, keep its prepack guard enabled, and use `scripts/package-npm.py` to verify and stage the distribution. Published manifests contain no scripts, so npm can repack them without Python; the source verifier and packager remain the supported way to construct verified release artifacts.

`about.toml` declares the MIT-first license choices and the union of all four release targets. Generation uses `crates/cli/Cargo.toml` with default features and includes build and transitive dependencies, excluding dev-only dependencies. `cargo-about --locked --offline --fail` must resolve every selected crate and license text; the supplemental pass preserves upstream LICENSE, NOTICE, COPYRIGHT, and COPYING files recursively, including native AWS-LC/ring terms and encoding_rs attribution. The launcher has no third-party JavaScript dependencies. Its four first-party optional packages are validated against an exact platform/version allowlist and ship the same Rust legal materials; other npm dependencies still require their upstream license files. The website has no runtime npm dependencies in its separate package.

Every binary archive and npm tarball ships `LICENSE`, `THIRD-PARTY-NOTICES.txt`, `third-party-licenses/`, and `third-party-sources/`. The last directory contains the unchanged complete `nucleo-matcher` MPL-2.0 source as its original `.crate` archive, verified against `Cargo.lock`, with extraction and license instructions. Oatty's MIT license does not replace MPL or other dependency terms. If that dependency changes or another MPL dependency is added, review the source-distribution obligation and update the explicit policy; generation fails until addressed. Keep native required-notice assertions current when upgrading their crates. Generic SPDX templates are insufficient attribution: generation requires actual upstream notices or narrowly reviewed evidence in `scripts/license-supplements/reviewed.json`. Those committed supplements pin the exact crate version/checksum, immutable upstream revision, and notice/header checksum, including original declarations where upstream supplied no copyright notice. Unknown notice-free/template-only packages fail; do not invent an owner or year. Re-audit source headers and evidence when upgrading. Supplemental material is vendored, and license harvesting runs offline after fetching locked crate sources.

The generated materials and `.license-manifest.json` are ignored build outputs. Regenerate after changing dependency locks, workspace manifests, package metadata, licensing policy, or the generator. Review new license choices and upstream notices rather than adding an exception for unknown licensing. CI tests failure paths, real npm packing, all release archive layouts, exact executable/source preservation, and repeated deterministic generation. Release jobs consume the same verified material bundle; the archive packager verifies it again before creating the existing asset names/formats. macOS continues to use `ditto` ZIP packaging before notarization.

To package a local binary after preparation:

```bash
python3 scripts/package-release.py --binary target/release/oatty \
  --target x86_64-unknown-linux-gnu --version vX.Y.Z
python3 scripts/package-npm.py --binary target/release/oatty \
  --target x86_64-unknown-linux-gnu --output dist
python3 scripts/package-npm.py --umbrella --output dist
```

Platform packages preserve the signed binary and carry `bin/.oatty-installed-version` beside it, so the existing updater treats npm installations as managed. macOS npm payloads are staged only after the unchanged release ZIP passes notarization. Release macOS x64 builds run on an Intel runner so installation smoke tests execute the matching native payload.

Publish the prepared set with `python3 scripts/publish-npm.py --directory dist` only from the authorized release environment after the npm prerequisites above are complete. This command performs live publication; packing and tests alone do not publish anything.


## Native update archive retention

Native `oatty update` saves the entire downloaded ZIP or tar.gz before extraction and executable replacement, preserving the release's license notices and corresponding dependency source. The original asset filename is kept inside a new `release-*` directory for every attempt, including repeated or concurrent downloads of the same version. Oatty copies, flushes, and synchronizes the archive before publishing it without overwriting an existing file. A retention failure rejects installation. If a later extraction or installation step fails, the saved archive remains and its path is included in the error.

The root is the platform's local application data directory plus `oatty/releases`: Linux uses `$XDG_DATA_HOME` or `~/.local/share`; macOS uses `~/Library/Application Support`; Windows uses `%LOCALAPPDATA%`. Resolution and directory creation occur only when an archive has actually downloaded. There is no working-directory fallback when the platform data directory is unavailable. Checking for updates, finding no newer version, and npm-managed installations create no retention artifacts. A successful native update prints the retained path unless `--quiet` is set; confirmation behavior remains unchanged.

These archives are not automatically pruned and consume disk space until the user deletes them. They may be removed manually when no longer needed. Keep the updater's release-name validation aligned with the four supported archive targets if the release matrix changes. The `compression-zip-deflate` feature is required because the macOS and Windows release ZIPs use DEFLATE.

Offline updater tests use a custom release source, loopback HTTP, temporary installation paths, and compressed fixture archives. They verify replacement and retention without GitHub requests or replacing the running executable. Release validation also extracts archives produced by the actual packager through the pinned updater library, checking executable and licensing/source bytes.
