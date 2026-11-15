# Rust version support

This source revision declares **Rust 1.88.0** as its minimum supported Rust
version (MSRV) in `package.rust-version`. Current stable is also tested. This
policy applies to this revision and its committed `Cargo.lock`; the unchanged
package version does not extend this promise to previously published artifacts.

## Why 1.88.0

The locked normal dependencies `cookie_store 0.22.1` and `time 0.3.47` require
Rust 1.88.0; `time-core 0.1.8` and `time-macros 0.2.27` have the same declared
minimum. The locked `process-wrap 10.0.1` requires 1.87.0. Cargo rejects the
existing dependency graph on Rust 1.86.0 before compiling this application.
The 1.88.0 baseline retains these dependencies and lockfile versions.

This is a tested support floor, not a claim that dependency downgrades or
`--ignore-rust-version` could never produce a build on an older compiler.
Cargo documents the manifest field and its support expectations in
[Rust Version](https://doc.rust-lang.org/cargo/reference/rust-version.html).

## Verification scope

The Rust workflow runs MSRV jobs on native runners for every current binary
release target:

| Target | MSRV checks |
| --- | --- |
| `x86_64-unknown-linux-gnu` | All targets/features compile; full ordinary tests; actual source-package verification. |
| `x86_64-apple-darwin` | All targets/features compile; package-selection tests and fixture builds. |
| `aarch64-apple-darwin` | All targets/features compile; package-selection tests and fixture builds. |
| `x86_64-pc-windows-msvc` | All targets/features compile; package-selection tests and fixture builds. |

All dependency-bearing checks use `--locked`. Stable jobs retain Clippy,
ordinary/platform tests and root/subpath GitBucket E2E. Live E2E runs on stable,
not on MSRV. Intermediate compiler versions are not separate CI jobs. This
matrix does not verify other target triples, historical OS releases or a
particular deployment.
Release binaries continue to use stable rather than pinning their compiler to
the source-build minimum. Local commands are in
[TESTING.md](../TESTING.md#minimum-supported-rust-version).

The ARM macOS jobs in both workflows use `macos-15`; GitHub
[retired `macos-14` on November 2, 2026](https://github.blog/changelog/2026-10-01-github-actions-macos-14-runner-image-retirement/).
Changing the runner retains the `aarch64-apple-darwin` artifact target.

## Dependency and compiler updates

- Use `--locked` when installing or building this revision. Resolving new
  versions without the lockfile is outside this tested dependency set.
- Dependency and source updates must pass both MSRV and stable CI. A failed
  MSRV job is a compatibility failure; do not silently switch it to stable or
  bypass `rust-version` checks.
- Raising MSRV requires a deliberate compatibility decision, a release-note
  entry and synchronized changes to `Cargo.toml`, the MSRV workflow toolchain,
  README/SPEC and this document. Record the new dependency/source requirement
  and verify the same release targets before claiming support.
- A stable toolchain update alone does not raise MSRV. Clippy and formatting
  policy follow stable; the MSRV job checks compilation, tests and packaging.

No fixed compiler-age or calendar-based MSRV increase is implied. No release
version or publication is selected by establishing this baseline.
