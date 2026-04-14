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
4. Run the release validation locally:

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

5. Commit the version and documentation changes.
6. Create and push the release tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The `Release` workflow validates the tag, builds release binaries for Linux, macOS Intel, macOS Apple Silicon, and Windows, generates `SHA256SUMS`, and publishes a GitHub Release.

## Re-running A Release

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
