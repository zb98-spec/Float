# Releasing

Releases are built in `ijd5004/totalfloat` by `.github/workflows/release.yml` when a version tag is pushed. Forks don't run the workflow.

## Publishing a release

1. Merge everything the release needs, including the `version` bump in the root `Cargo.toml` and its `CHANGELOG.md` entry, and confirm `just check` passes on `main`.
2. Tag `vX.Y.Z` on `main` and push the tag. Wait for the release workflow.
3. Replace the generated release notes with the version's `CHANGELOG.md` section (`gh release edit vX.Y.Z --notes-file`).

Pushing a tag such as `v0.1.0` runs `.github/workflows/release.yml`. It builds the web app and a Linux and a Windows binary with it compiled in, unpacks each archive and runs it (version, `inspect`, `schedule`, `new`, an MCP handshake, and `serve` showing the app), writes a CycloneDX SBOM and `SHA256SUMS`, and publishes a GitHub release. On Windows the test also fails if the exe needs the Visual C++ Redistributable. Releases are Linux (x86-64) and Windows (x86-64, a `.zip` with the files at its root, so Explorer's **Extract All** gives one folder); a macOS build passed the same test, and the workflow says how to add it back. A tag with a hyphen, such as `v0.1.0-rc.1`, publishes a pre-release.

Each archive is published twice: under its versioned name, such as `totalfloat-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`, and as a byte-identical copy without the version, `totalfloat-x86_64-unknown-linux-gnu.tar.gz` and `totalfloat-x86_64-pc-windows-msvc.zip`. `SHA256SUMS` lists both, and both are attested. The docs site's download buttons link to the version-free copies through `releases/latest/download/<name>`, which always serves the latest release. The install scripts download the versioned files. The `Install scripts` workflow (`.github/workflows/install.yml`) checks weekly, and on pull requests that change the release workflow, that the latest release has both version-free copies.

Before tagging, set `version` in the root `Cargo.toml`, because the binary reports that version, not the tag.

## Attestations

The workflow attests every artifact and the SBOM with GitHub artifact attestations. GitHub offers these only on public repositories, or private ones on Enterprise Cloud, so while this repository is private the steps are skipped and the run says so. Once public, verify a download with:

```sh
gh attestation verify totalfloat-v0.1.0-x86_64-unknown-linux-gnu.tar.gz --repo ijd5004/totalfloat
```

## Checking a download

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

On Windows, see [the install guide](guide/install.md#windows).

## Practice run

To exercise the workflow, push a pre-release tag such as `v0.0.1-rc.1`, check the artifacts, then remove it:

```sh
gh release delete v0.0.1-rc.1 --yes --cleanup-tag
```
