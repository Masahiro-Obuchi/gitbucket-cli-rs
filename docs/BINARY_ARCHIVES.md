# Binary archive documentation

Binary archives retain the existing names, formats and top-level executable:
`gb-<tag>-<target>.tar.gz` on Linux/macOS and `.zip` on Windows. Unpack the whole
archive and open **README.md** in a Markdown viewer. The bundled documentation
describes the packaged source revision, including its remaining server and
deployment limits; the manifest version alone does not identify its contents.

## Included files and navigation

- `gb` (Unix) or `gb.exe` (Windows), and `LICENSE`.
- `README.md`, `SPEC.md`, `RELEASE.md` and `TESTING.md` at the archive root.
- Public Markdown pages below `docs/`, with their relative directory layout.

Links between these documents, including heading anchors, remain local. For
example, README's authentication, input, JSON and review links work without
fetching another document. Links to source evidence such as `Cargo.toml` and
`tests/*.rs`, which are not bundled, become GitHub links at the **release tag**.
Existing absolute links (upstream contracts, PRs, CI badges) stay unchanged and
require network access. Command examples and paths written as code are not
links and do not imply that a source checkout, test suite or build tool is
included in the binary archive.

Only the named root files and public `docs/**/*.md` are selected. Hidden paths,
directories whose names end in `.md`, symlinks, local notes and other workspace
trees cannot widen this selection. Allowed document contents still need review;
the selector is not a content/secret scanner. The source checkout is never
rewritten when preparing archive-specific links.

This policy is separate from [Cargo source-package selection](PACKAGE_CONTENTS.md).
Rust source files and build helpers are not bundled. Rebuilding an older tag
predating this packager requires that tag's workflow and packaging policy;
changing the current workflow does not retrofit older release files.

## Build and verify without publishing

From a source checkout, build a native binary and use the shared packager
(`python` below means Python 3.10 or later). Substitute the chosen version label
and the appropriate target/executable for another platform:

```sh
cargo build --release --locked --target x86_64-unknown-linux-gnu
python scripts/package-release.py --binary target/x86_64-unknown-linux-gnu/release/gb --tag v0.6.1 --target x86_64-unknown-linux-gnu --smoke-test
```

This produces a local verified archive under `target/dist/`; it does not create
a Git tag, publish a crate or upload a GitHub Release. The packager accepts a
version label for previews; Release preflight separately checks that a real
tag matches the package version. Source-evidence URLs in a preview will only
resolve online if that label actually exists with the selected source files.

Before returning the archive, the packager reads it back into a fresh temporary
directory, compares its exact file set and bytes, checks local Markdown file
and heading links against that extracted tree, and checks Unix executable mode.
`--smoke-test` additionally runs the extracted native binary's `--help` there.
A failure stops before replacing an existing output archive.

The link checker supports this project's inline Markdown links, image links
and optional double-quoted titles; it skips fenced/inline code. If introducing
other link or heading syntax, extend its parser and regression fixtures in the
same change. It does not fetch external URLs or render Markdown to HTML.

## CI and release integration

The Rust PR/main workflow and Release workflow call the same reusable
`archive.yml` workflow. On native Linux x86_64 GNU, macOS x86_64/aarch64 and
Windows x86_64 MSVC runners it:

1. Runs the disposable packaging/navigation regressions.
2. Builds the real release-mode binary with stable Rust and `--locked`.
3. Packages, extracts, verifies documentation and runs the extracted binary.
4. Uploads only the verified archive as a workflow artifact.

PR/main artifacts use `v0.0.0-archive-test` as a preview label and create no tag
or release. Their checkout is pinned to the triggering event's commit SHA, so
queued archive jobs still verify that run's revision after a branch/PR update.
The Release caller retains version validation, checksum generation
and its existing explicit tag/dispatch publishing behavior. No publication is
triggered merely by adding or running the archive checks.

See [release gates](../RELEASE.md#preparation-gates) and
[archive tests](../TESTING.md#binary-archive-documentation-and-navigation).
