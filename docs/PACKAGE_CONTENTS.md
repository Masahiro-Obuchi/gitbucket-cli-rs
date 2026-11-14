# Source-package contents

C1 replaces the manifest's list of excluded directories with an explicit
`package.include` policy in [Cargo.toml](../Cargo.toml). Local `review/` notes,
new workspace directories, runtime files and editor settings do not become
publishable simply by being untracked, force-added to Git or packaged with
`--allow-dirty`. The earlier worktree listing included five local review files;
the same files remain on disk and are absent from the new listing.

## Public inputs

The policy permits these repository-root paths:

- `Cargo.toml`, `Cargo.lock`, `LICENSE`, `README.md`, `SPEC.md`, `RELEASE.md`
  and `TESTING.md`.
- Rust source under `src/` and `tests/`.
- JSON, HTML and Markdown fixtures under `tests/fixtures/`.
- Markdown documentation under `docs/`.

Hidden files and hidden directories under those trees are explicitly removed
from the selection. Other extensions and other root paths are not included.
`RELEASE.md` and `TESTING.md` now ship with the source crate so its documentation
can resolve the existing links to them. This does not change the separately
assembled binary archives; their documentation strategy remains C4.

The patterns are rooted so a similarly named directory elsewhere is not a new
public source tree. Cargo handles its generated manifest, original-manifest
copy, lockfile and VCS metadata. As documented by
[Cargo](https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields),
explicit include rules take precedence over Git ignore rules. Therefore the
policy also excludes dotfiles explicitly; adding an ignore rule alone does not
change an explicit published-input decision.

This policy selects paths and file types; it does not scan allowed Rust,
Markdown or fixture contents for credentials. Keep private material outside
these public trees. Review any new included file type/tree and retain clean
checkout/package inspection as release gates. Public source required by a module
must not disappear merely because a local Git ignore rule matches it.

## Regression and archive verification

```sh
cargo test --locked --test package_contents
cargo package --list --locked
cargo package --locked --offline
```

The integration tests copy the actual manifest policy into dependency-free
temporary packages. They supply public source/docs/fixtures as positive controls
and synthetic local files as negative controls. One fixture has no Git
repository; another deliberately commits local files, then adds untracked and
ignored local files. The latter also ignores a required public Rust module.
Each test checks Cargo's file list, then packages and builds the extracted
fixture, detecting both leaked local files and missing required build inputs.
They never read or modify the worktree's local review notes.

The Linux/macOS/Windows Rust jobs run these regressions. The Linux Rust job,
Release preflight and `scripts/release-tag.sh` additionally run
`cargo package --locked --offline` after the ordinary checks have populated the
dependency cache. That command builds the real archive after Cargo extracts it;
package verification failure stops the job/helper before a tag is created.
The command generates a local archive, not a registry upload.

For local development verification, `--allow-dirty` can be added to the package
commands; it relaxes Cargo's worktree check, not the manifest selection policy.
Release preparation still uses a clean checkout and inspects the exact archive.
See [release gates](../RELEASE.md#preparation-gates). Version selection, an MSRV
declaration and publication are separate actions.
