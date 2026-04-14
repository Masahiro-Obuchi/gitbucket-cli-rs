# Release Process

This project publishes prebuilt `gb` binaries to GitHub Releases from version tags.

## Versioning

Use SemVer-style tags with a leading `v`, such as `v0.1.0`.

The tag version must match the package version in `Cargo.toml`. For example:

```toml
version = "0.1.0"
```

matches:

```text
v0.1.0
```

## Release Checklist

1. Choose the next version.
2. Update `Cargo.toml` and refresh `Cargo.lock` if the package version changed.
3. Update user-facing release notes or README content when behavior changed.
4. Create the release tag with the helper script:

```bash
scripts/release-tag.sh v0.1.0
```

This command validates the tag format, checks the Cargo version, verifies a clean working tree, runs the release checks, and creates the local tag.

5. Push the release tag:

```bash
git push origin v0.1.0
```

The `Release` workflow validates the tag, builds release binaries for Linux, macOS Intel, macOS Apple Silicon, and Windows, generates `SHA256SUMS`, and publishes a GitHub Release.

## Re-running a Release

Use the `Release` workflow's manual dispatch with an existing tag when the release job needs to be re-run without creating a new tag.

Manual dispatch can create or update the GitHub Release as a draft. Tag pushes publish non-draft releases.

## Artifacts

Each release includes:

- `gb-<tag>-x86_64-unknown-linux-gnu.tar.gz`
- `gb-<tag>-x86_64-apple-darwin.tar.gz`
- `gb-<tag>-aarch64-apple-darwin.tar.gz`
- `gb-<tag>-x86_64-pc-windows-msvc.zip`
- `SHA256SUMS`

Each archive contains the `gb` binary plus `README.md` and `LICENSE`.
