# Release Process

This project publishes prebuilt `gb` binaries to GitHub Releases from version tags.

## Versioning

Use SemVer-style tags with a leading `v`, such as `v0.5.1`.

The tag version must match the package version in `Cargo.toml`. For example:

```toml
version = "0.5.1"
```

matches:

```text
v0.5.1
```

## Release Checklist

1. Choose the next version.
2. Update `Cargo.toml` and refresh `Cargo.lock` if the package version changed.
3. Update user-facing release notes or README content when behavior changed.
4. Commit all version and documentation changes:

```bash
git add Cargo.toml Cargo.lock README.md  # include any other updated files
git commit -m "chore: release v0.5.1"
```

5. Create the release tag with the helper script:

```bash
scripts/release-tag.sh v0.5.1
```

This command validates the tag format, checks the Cargo version, verifies a clean working tree, runs the release checks, and creates the local tag.

6. Push the release tag:

```bash
git push origin v0.5.1
```

The `Release` workflow validates the tag, builds release binaries for Linux, macOS Intel, macOS Apple Silicon, and Windows, generates `SHA256SUMS`, and publishes a GitHub Release.

Do not move a published release tag for a normal release. If a tag was pushed before the Cargo package version was updated, update `Cargo.toml` and `Cargo.lock`, commit the fix, and then either create the next patch tag or deliberately force-update the failed tag before any release artifacts are consumed.

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
