# Releasing GitBucket CLI

Publish the Cargo crate with `cargo publish`. Publish prebuilt binaries through the GitHub Release workflow by pushing a version tag. Verify the published crate and GitHub assets separately.

## Version and release notes

Use a fresh `vMAJOR.MINOR.PATCH` tag whose version matches both `Cargo.toml` and the root package entry in `Cargo.lock`. Build and validate the exact commit that will be tagged. Match installed artifacts to the source tag, commit, and checksum when investigating an upgrade. A manifest version or `gb --version` output alone can appear in builds from different commits.

Cover changes users or scripts will observe relative to the previous published version. In particular, check [saved authentication settings](../usage/authentication.md#scheme-and-instance-matching), [JSON and error contracts](../reference/automation.md), and the [tested deployment scope](../reference/gitbucket-compatibility.md). Describe any required config or script changes in the release notes. During a staged upgrade, verify targets with `gb config list --json` and the effective REST actor with `gb auth status --json`. Preserve the relevant inline-write [retry records](../reference/review-writes.md#retry-records) when switching configuration directories.

## Preparation gates

1. Commit the chosen version and lockfile. Prepare release notes for the precise candidate and its server limits.
2. Inspect `cargo package --list --locked` from a clean checkout. Check that package contents match the [source package policy](packaging.md#source-package).
3. Run [local checks](testing.md#local-checks), the [MSRV checks](testing.md#minimum-supported-rust-version), and `cargo package --locked --offline`. Check the native platform and archive jobs on the candidate.
4. Run the [pinned GitBucket E2E suite](testing.md#live-gitbucket-tests) at the site root and under `/gitbucket`. For an intended deployment that differs from the fixture, record the server build, base URL layout, authentication, plugins, Git transports, and relevant test results.
5. Run `cargo publish --dry-run --locked` to verify the crate before publication. Inspect the candidate's [binary archives](packaging.md#binary-archives) and their extracted documentation separately.
6. Record the candidate commit, version, toolchain, target matrix, artifact inventory, and results. Resolve failed checks before publishing.

## Publication

1. Publish the verified crate with `cargo publish --locked`.
2. Run `scripts/release-tag.sh` with the new tag. It verifies the version, an unused local tag, a clean worktree, and the locked release checks before creating a local tag.
3. Push that new tag to origin. The Release workflow runs preflight, builds and verifies the archives, writes `SHA256SUMS`, and publishes a non-draft GitHub Release.
4. Confirm both publication channels and the matching checksum. For an incorrect version or a partial publication, inspect which artifacts exist before choosing a new version and tag. A consumed tag identifies its original revision.

## Rebuilding GitHub artifacts

Manual dispatch of the Release workflow uses an existing tag to build or update GitHub assets. The `draft` input controls the GitHub Release state. Use the tagged source and workflow, and verify that the chosen tag, crate record, and binary checksums describe the intended artifact. The [E2E workflow](testing.md#live-gitbucket-tests) is a separate validation step.
