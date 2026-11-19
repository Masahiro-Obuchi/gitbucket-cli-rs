# GitBucket CLI documentation

Start with [installation and Quick Start](../README.md).

## Usage

- [Authentication and configuration](usage/authentication.md) covers login, profiles, saved instances, and credential selection.
- [Command examples](usage/commands.md) covers repository selection, repositories, issues, labels, milestones, browser access, the raw REST API, and shell completion.
- [Pull requests](usage/pull-requests.md) covers creation, editing, discussion, reviews, checkouts, diffs, and merges.
- [Input and terminal display](usage/input-and-display.md) covers file and stdin input, editors, prompts, and terminal output.

## Reference

- [Automation](reference/automation.md) covers environment variables, deadlines, JSON output, error codes, pagers, and pagination.
- [GitBucket compatibility](reference/gitbucket-compatibility.md) covers the tested server, REST and Web behavior, and deployment limits.
- [Saved PR commits](reference/pull-request-snapshots.md) explains checkout and diff behavior when branches or forks change.
- [Reading review comments](reference/review-comments.md) explains the records and coverage returned by the visible conversation.
- [Writing review comments](reference/review-writes.md) covers inline operations, verification, and retries.
- [Merge checks and review decisions](reference/merge.md) explains the guarantees and prerequisites for merging.

## Maintenance

- [Building and testing](maintainers/testing.md) covers source builds, test selection, the Rust version policy, and Docker E2E.
- [Releasing](maintainers/release.md) covers versioning, validation, and publication.
- [Packaging](maintainers/packaging.md) covers Cargo source packages, binary archives, and offline documentation.
