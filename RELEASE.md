# Release Process

The `gitbucket-cli` crate is published to crates.io by an explicit maintainer
command. Pushing a version tag runs the GitHub `Release` workflow, which builds
and publishes binary artifacts; it does **not** publish the crate. These are
separate publishing actions.

The refactor series through PR #56 is merged, but this document does not choose
or publish its next version. Read [the delivered work, upgrade notes and remaining
requirements](docs/REFACTOR_STATUS.md) before preparing a release. A manifest
version of `0.6.1` does not imply that an existing `v0.6.1` binary contains later
refactor commits.

## Versioning

Use a new SemVer-style tag with a leading `v`. Its version must match both
`Cargo.toml` and the root package entry in `Cargo.lock`. For illustration, package
version `1.2.3` requires tag `v1.2.3`; this is not a selected release version.
The refactor changes authentication scope/status, JSON absence/augmentation,
noninteractive input, PR defaults and merge verification. Review those
[upgrade-sensitive behaviors](docs/REFACTOR_STATUS.md#upgrade-notes-to-carry-into-the-next-release)
when selecting a version and writing release notes.

Use the stable toolchain tested by CI. An MSRV has not been established; do not
describe Rust 1.70+ or another minimum as supported without testing it. Defining
an MSRV and a matching CI job is follow-up C2 in the status register.

## Preparation gates

Prepare from a clean checkout of the intended release commit. The manifest's
explicit [source-package policy](docs/PACKAGE_CONTENTS.md) includes public
source, tests/fixtures and named documentation, and excludes local workspace
material such as `review/`, `.tmp/` and dotfiles even with `--allow-dirty`.
This closes C1's observed review-file leak; content inside the allowed public
paths still needs review. Do not use `--allow-dirty` as a substitute for that
review or the clean-checkout gate.

Before any publication:

1. Select the version and release commit, update the manifest/lockfile and
   prepare release notes that distinguish delivered features from server and
   deployment limits. Keep the worktree clean after committing those changes.
2. Inspect the **source crate** contents from that checkout:

   ```sh
   cargo package --list --locked
   ```

   Confirm that no `review/`, `.tmp/`, `.codex`, token files or session artifacts
   are included. This command lists files; it does not upload a package.
3. Run the locked ordinary validation:

   ```sh
   cargo fmt --all -- --check
   cargo check --locked
   cargo test --locked
   cargo clippy --locked --all-targets --all-features -- -D warnings
   cargo package --locked --offline
   ```

4. Confirm the **full** pinned GitBucket 4.44.0 E2E suite at root and `/gitbucket`
   on the release revision, plus relevant private/Git authentication and
   Linux/macOS/Windows checks. Follow [TESTING.md](TESTING.md). Ordinary tests
   ignore Docker cases; neither `release-tag.sh` nor the Release preflight job
   substitutes for this server validation. Record any intended-deployment
   checks separately; the pinned fixture does not prove arbitrary plugin/SSO
   or business-server compatibility.
5. Validate the exact source package without publishing:

   ```sh
   cargo publish --dry-run --locked
   ```

6. Record commit, version, toolchain/targets, package inventory and test results.
   Preview binary-archive contents separately from the source crate. Resolve
   failures or retain them as open release work; an unverified check is not a
   passed gate.

The historical PR-17 record is 536 ordinary tests, 47 Docker cases ignored by
ordinary tests, and five related live cases per layout locally; PR CI ran
full E2E suites. Re-run and record the totals for the chosen release commit
rather than copying that historical evidence as a new release validation.

## Publication

After the gates pass and the maintainer chooses to publish:

1. Publish the chosen crate with `cargo publish --locked`.
2. Run `scripts/release-tag.sh` with the chosen **new** tag. The helper checks
   tag syntax/version, a clean worktree, existing local tags and ordinary release
   validation including source-package verification, then creates the tag
   locally. It does not push or publish by itself.
3. Push only that new release tag to origin. The tag-push `Release` workflow
   publishes a **non-draft** GitHub Release after its preflight/build jobs succeed.

Do not move a published/consumed release tag. If an attempted release used the
wrong version or failed after part of publication, record which artifacts already
exist and choose a new version/tag for the correction. Reconcile crate and binary
publication states before continuing; one successful publication does not prove
that the other completed.

## Re-running a Release

The `Release` workflow's manual dispatch uses an existing tag to build or update
GitHub binary artifacts. Its `draft` input controls the GitHub Release only;
tag pushes publish non-draft releases. It does not publish a crate or re-run the
dedicated E2E workflow automatically. Inspect existing artifacts and the chosen
tag before dispatching a repeat build.

## Artifacts

The current workflow builds:

- `gb-<tag>-x86_64-unknown-linux-gnu.tar.gz`
- `gb-<tag>-x86_64-apple-darwin.tar.gz`
- `gb-<tag>-aarch64-apple-darwin.tar.gz`
- `gb-<tag>-x86_64-pc-windows-msvc.zip`
- `SHA256SUMS` over those archives

Each binary archive currently contains the `gb` binary, `README.md` and `LICENSE`.
The linked `docs/` pages are not bundled. Read documentation at the matching tag
in the source repository; bundling/navigation from extracted archives remains
follow-up C4. A Cargo source package has its own manifest-driven file inventory
and is not validated merely by inspecting the binary archives.
