# Source packages and binary archives

## Source package

The Cargo manifest's explicit [include list](../../Cargo.toml) permits `Cargo.toml`, `Cargo.lock`, `README.md`, `LICENSE`, Rust sources under `src/` and `tests/`, Markdown under `docs/`, and public JSON, HTML, and Markdown fixtures under `tests/fixtures/`. Put credentials and workspace files outside those public paths.

The patterns exclude hidden files and descendants of directories with file extensions such as `.md` or `.rs`. Cargo's include rules take precedence over Git ignore rules. Review what is published by inspecting the files as well as the paths.

```sh
cargo test --locked --test package_contents
cargo package --list --locked
cargo package --locked --offline
```

The package tests exercise the manifest policy with both Git and Git-free temporary directories. They validate the file list and build the extracted fixture. The final command verifies the actual source crate after dependencies are cached. Run it from a clean checkout of the release candidate. See [release preparation](release.md#preparation-gates).

## Binary archives

Release archives contain the native executable (`gb` or `gb.exe`), `LICENSE`, `README.md`, and public Markdown under `docs/`, preserving their directory layout. Open the extracted README to navigate the [documentation index](../README.md). Links between bundled pages and their headings work offline. Links to source and test evidence point to the corresponding GitHub tag.

| Target | Archive |
| --- | --- |
| `x86_64-unknown-linux-gnu` | `gb-<tag>-x86_64-unknown-linux-gnu.tar.gz` |
| `x86_64-apple-darwin` | `gb-<tag>-x86_64-apple-darwin.tar.gz` |
| `aarch64-apple-darwin` | `gb-<tag>-aarch64-apple-darwin.tar.gz` |
| `x86_64-pc-windows-msvc` | `gb-<tag>-x86_64-pc-windows-msvc.zip` |

The GitHub Release also includes `SHA256SUMS` for the archives. The reusable archive workflow runs on pull requests to `main` and for releases. It builds the event revision with stable Rust and the committed lockfile, verifies the extracted archive, and runs the native binary's help.

### Verify an archive locally

Run these commands with Python 3.10 or later. The example uses a Linux build and a preview tag. Choose the target and executable for your platform. Source links in a preview resolve online only once the referenced tag exists.

```sh
python3 -B -m unittest discover -s scripts/tests -v
cargo build --release --locked --target x86_64-unknown-linux-gnu
python3 scripts/package-release.py --binary target/x86_64-unknown-linux-gnu/release/gb --tag v0.0.0-archive-test --target x86_64-unknown-linux-gnu --smoke-test
```

The packager verifies regular files, exact archive members and bytes, Unix executable mode, and local Markdown links and headings after extraction. `--smoke-test` also runs the extracted binary. A failure leaves any previously created archive in place. This local check produces an archive under `target/dist/`.
