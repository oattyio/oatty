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
npm pack --dry-run
```

## Release steps

1. Update npm package version.

```bash
npm version X.Y.Z --no-git-tag-version
git add package.json
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
