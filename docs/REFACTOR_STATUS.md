# Refactor status and remaining work

The planned PR-00–17 series is merged through [PR #56](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/56)
(merge `2dc992e`). These are planning labels, not GitHub PR numbers. This closes
the defined client implementation units; it does not complete unsupported
server features or validate an unknown deployment. No next release version is
selected by this document. The manifest still says `0.6.1`.

## Delivered units

| Unit | Merged PR | Delivered behavior and evidence |
| --- | --- | --- |
| PR-00 | [#38](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/38) | Shared environment-test guard and pinned REST/Web contract foundation; later fixture/position cases were added in PR-10/12/13. [Compatibility baseline](GITBUCKET_COMPATIBILITY.md). Deployment validation remains open. |
| PR-01 | [#39](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/39), [#40](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/40) | Scoped Git credentials, instance/repository identity and API/redirect boundaries. Legacy credential migration was completed in PR-14. [Authentication](AUTH_PROFILES.md), [target validation tests](../tests/target_validation.rs), [Git credentials](../tests/git_credentials.rs). |
| PR-02 | [#41](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/41) | Locked read/modify/write, atomic configuration replacement and filesystem protections. [Storage tests](../tests/config_storage.rs). |
| PR-03 | [#42](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/42) | Checkout protects worktrees, index, branches, submodules and tracking refs. [Checkout tests](../tests/pr_checkout.rs). |
| PR-04 | [#43](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/43) | Milestone replacement preserves unspecified values; known title constraints select verified Web operations. [Routes and limits](WEB_OPERATIONS.md). |
| PR-05 | [#44](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/44) | PR title/body/state REST edits, close/reopen and canonical target checks; assignees remain separate verified Web changes. [PR edit tests](../tests/pr_edit_flows.rs). |
| PR-06 | [#45](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/45) | Issue label deltas and identity-preserving label color edits. Unsupported description writes fail before mutation. [Remaining server needs](UPSTREAM_LABEL_API.md). |
| PR-07 | [#46](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/46) | REST/Cookie principal checks, verified Web writes and shared operation outcomes. [Web operations](WEB_OPERATIONS.md). |
| PR-08 | [#47](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/47) | HTTP/Git deadlines, cancellation, response contracts and write uncertainty. [Transport tests](../tests/transport_failures.rs), [Git deadlines](../tests/git_timeouts.rs). |
| PR-09 | [#48](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/48) | Complete pagination, canonical fork ownership and existing-PR lookup. [Collection contracts](PAGINATION.md). |
| PR-10 | [#49](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/49) | Saved-commit PR diff/checkout, retained refs and deleted-fork recovery. [Snapshot limits](PR_SNAPSHOTS.md). |
| PR-11 | [#50](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/50) | Immutable original API JSON, typed projections and explicit comment augmentation. [JSON contract](JSON_OUTPUT.md). |
| PR-12 | [#51](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/51) | Visible inline/commit review reads with typed identities and explicit coverage. [Read limits](REVIEW_COMMENTS.md). |
| PR-13 | [#52](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/52) | Inline create/edit/reply/delete, saved-position checks, principal checks and local retry receipts. [Write limits](REVIEW_WRITES.md). |
| PR-14 | [#53](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/53) | Profile/global scope, verified auth status, explicit credential-key migration and guarded restore. [Scope and migration](AUTH_PROFILES.md). |
| PR-15 | [#54](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/54) | Unified JSON errors, stable exit codes, closed-pipe handling and pager lifecycle. [Output contracts](CLI_ERRORS_OUTPUT.md). |
| PR-16 | [#55](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/55) | File/stdin bodies and token input, noninteractive behavior, published-head/default-base checks, Unicode/control-character display. [Input contracts](INPUT_DISPLAY.md). Optional editor input remains deferred. |
| PR-17 | [#56](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/56) | Reviewed-head merge checks, canonical readback and an explicit review/merge support matrix. [Guarantees and remaining limits](REVIEW_MERGE.md). |

The implementation introduces typed targets, domain/application modules and
operation-specific adapters incrementally. It retains command orchestration in
parts of `src/cli/`; merging the series is not a claim that every command has
been moved into the application layer. [SPEC's layout](../SPEC.md#91-project-layout)
describes the actual directories.

## Upgrade notes to carry into the next release

These changes are already on the branch; they must not be advertised as present
in an older published binary solely because its version matches the manifest.
Consult the selected release's notes and source revision.

- **Authentication and scopes:** `auth status` verifies `/user` by default;
  `--offline` is inventory with an unknown actor. Auth/config mutations honor the
  selected profile; `--global` explicitly selects global storage. Logout can
  leave environment tokens/global fallback active. Explicit credential-key
  migration previews collisions and creates a guarded backup. Git with
  `GB_TOKEN` alone also requires `GB_USER`. [Migration details](AUTH_PROFILES.md).
- **Automation and JSON:** absent API fields remain absent rather than becoming
  synthetic null/false/empty values. Issue comment details use `comment_details`;
  the PR's legacy comments array retains the original count under `api_comments`
  when present. Incomplete pagination fails instead of returning a partial
  collection. [JSON](JSON_OUTPUT.md), [pagination](PAGINATION.md).
- **Input and failures:** noninteractive creation requires a title and omits an
  unspecified optional body. Use `--body-file -` for explicit UTF-8 stdin and
  `auth login --with-token` for token stdin. JSON-error mode suppresses prompts;
  argument errors exit 2, runtime errors exit 1, and a failed pager exits 1.
  A closed stdout pipe alone exits normally. [Input](INPUT_DISPLAY.md),
  [errors](CLI_ERRORS_OUTPUT.md).
- **PR/Git behavior:** omitted base comes from `repo.default_branch`; an inferred
  head must match the live published branch. There is no automatic push.
  Checkout refuses conflicting branches/worktrees; diff uses saved commits.
  Keep local inline-write receipts when retrying uncertain submissions.
  [Snapshots](PR_SNAPSHOTS.md), [receipts](REVIEW_WRITES.md#retry-records).
- **Mutation semantics:** updates preserve omitted values, label deltas preserve
  unrelated associations, and Web writes verify actors and stored results.
  Label description writes remain unsupported. Multi-step edits are not atomic
  and unverified writes must be inspected before another attempt.
  [Routes](WEB_OPERATIONS.md), [label limits](UPSTREAM_LABEL_API.md).
- **Reviews and merge:** comments are not formal approvals. Merge head checks
  are client checks with a race window. `pr merge --json` is a CLI report with
  `atomic=false` and the preserved API result, not a bare PR resource.
  [Review and merge support](REVIEW_MERGE.md).

## Completed follow-up: C1 source-package policy

The manifest now permits explicit public root files, Rust source/tests,
JSON/HTML/Markdown fixtures and Markdown documentation, excluding hidden paths.
Local review notes and other root workspace trees are excluded regardless of
Git tracking/ignore state or `--allow-dirty`. Regression fixtures check both
Git and non-Git inputs, preserve required public files and build the extracted
packages. CI and the release helper also verify the actual source archive.
See [the package policy](PACKAGE_CONTENTS.md) and
[regression tests](../tests/package_contents.rs). This does not scan permitted
file contents or change binary-archive documentation (C4).

## Remaining client and release work

The following are candidates for separately scoped follow-up PRs. They are not
unimplemented parts silently counted as completed server functionality.

| ID | Work | Completion evidence |
| --- | --- | --- |
| C2 | Minimum Rust version | Choose and test an MSRV against the locked dependencies and intended targets before adding a `rust-version` promise. CI currently uses stable; the former README claim of Rust 1.70+ was not supported by an MSRV job. |
| C3 | Optional editor input | Separate PR if needed, as deferred in PR-16. Define editor selection, protected temporary files, failure/cancellation handling and unchanged file/stdin semantics. File/stdin input already works. |
| C4 | Release artifact documentation | Binary archives currently contain only `gb`, README and LICENSE; linked `docs/` files are not bundled. Choose bundling or an explicit online documentation strategy and verify navigation from an extracted archive. Crate source and binary archives are separate artifacts. |

The next implementation candidate is C2. C3 is optional. Choosing a release
version, publishing a crate, tagging or dispatching a release is a separate
maintainer action; the packaging-policy change performs none of them.
See [release gates](../RELEASE.md#preparation-gates).

## Remaining server and deployment requirements

| ID | Requirement | What closes it |
| --- | --- | --- |
| S1 | Formal approval/request-changes/dismissal and complete stored review history | A verified server API with decision lifecycle/commit scope, stable persisted associations, raw bodies and history enumeration. The 4.44.0 adapter exposes visible comments only. [Review API requirements](UPSTREAM_REVIEW_API.md). |
| S2 | Atomic expected-head merge / conditional updates | Server comparison and ref/state update under the same lock/transaction, with concurrency tests proving mismatch rejection without effects. More client GETs or an ignored `sha` field cannot provide this. [Merge requirements](REVIEW_MERGE.md#what-a-server-extension-must-guarantee). |
| S3 | Label description persistence and fully supported in-place REST color edits | Storage and create/read/update/clear contracts, stable label identity and concurrent-update semantics. Current known restrictions remain explicit client errors/Web operations. [Label requirements](UPSTREAM_LABEL_API.md). |
| D1 | Intended deployment compatibility | Record exact server build, base path/proxy, auth/plugin/SSO mode, Git transports/SSH port and certificate setup; validate the relevant read/write/Git flows in an authorized disposable repository. Store sanitized evidence and no credentials/cookies. No business-environment verification was supplied by this series. |

The verified server baseline is **GitBucket 4.44.0 core** at root and `/gitbucket`.
Linux/macOS/Windows CI verifies the configured test suites, not every filesystem,
auth plugin, terminal/font or GitBucket version. Password/Cookie Web sign-in does
not implement interactive SSO/OIDC. The unrelated user/webhook/collaborator
feature backlog remains in [SPEC](../SPEC.md#backlog--re-evaluate-later).

## Validation record and how to refresh it

At PR #56, local validation recorded **536 ordinary tests**, with **47 Docker
cases ignored** by the ordinary run, and the **five related merge/review/JSON
cases at each deployment layout**. Its CI completed the Linux/macOS/Windows jobs
and both full E2E jobs. These are historical observations of that revision, not
a certification of a future release or unknown deployment.

Use [TESTING.md](../TESTING.md) for current commands. Before release, record the
exact commit/toolchain/targets, package contents, ordinary tests, full root and
subpath E2E, and any target-deployment checks. Preserve failed/unverified results
as open work; do not replace them with a pass based only on documentation edits.

When a follow-up is merged, update its row, link the PR and verification evidence,
and update the affected contract documents. Keep the initial and per-PR test
counts as dated/revision-scoped history instead of rewriting them as current totals.
