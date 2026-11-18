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
| PR-14 | [#53](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/53) | Profile/global scope, verified auth status, explicit credential-key migration and guarded restore. [Authentication](AUTH_PROFILES.md). |
| PR-15 | [#54](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/54) | Unified JSON errors, stable exit codes, closed-pipe handling and pager lifecycle. [Output contracts](CLI_ERRORS_OUTPUT.md). |
| PR-16 | [#55](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/55) | File/stdin bodies and token input, noninteractive behavior, published-head/default-base checks, Unicode/control-character display. [Input contracts](INPUT_DISPLAY.md). Editor input followed separately in C3. |
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
  leave environment tokens/global fallback active. Saved hosts and defaults use
  normalized HTTP(S) base URLs. Git with `GB_TOKEN` also requires `GB_USER`.
  [Authentication](AUTH_PROFILES.md).
- **Automation and JSON:** absent API fields remain absent rather than becoming
  synthetic null/false/empty values. Issue and PR views add discussion under
  `comment_details`. PR comment lists report both source arrays and coverage.
  Incomplete pagination fails instead of returning a partial
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

Merged in [PR #58](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/58)
(merge `f2ccbc9`), including directory-glob exclusions and Git-environment
isolation for package fixtures.

The manifest now permits explicit public root files, Rust source/tests,
JSON/HTML/Markdown fixtures and Markdown documentation, excluding hidden paths.
Local review notes and other root workspace trees are excluded regardless of
Git tracking/ignore state or `--allow-dirty`. Regression fixtures check both
Git and non-Git inputs, preserve required public files and build the extracted
packages. CI and the release helper also verify the actual source archive.
See [the package policy](PACKAGE_CONTENTS.md) and
[regression tests](../tests/package_contents.rs). This does not scan permitted
file contents. Binary-archive documentation is handled separately by C4.

## Completed follow-up: C2 minimum Rust version

Merged in [PR #59](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/59)
(merge `6e8faa4`), including the available macOS ARM runner and portable PTY
test pointer needed by the new all-target compilation checks.

The manifest declares Rust **1.88.0** with the committed lockfile. Locked normal
dependencies already require that version. The Rust workflow checks all
targets/features on each of the four binary release targets at MSRV; Linux
also runs the full ordinary suite and verifies the actual source package, and
the other three run package-selection tests. Current stable tests/lints and
root/subpath E2E remain in place. See [the Rust version policy](RUST_VERSION.md)
for dependency-update rules and the limits of the verification scope.

## Completed follow-up: C4 binary-archive documentation

Merged in [PR #60](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/60)
(merge `ed4b908`), including event-commit pinning for previews and rejection of
non-regular ZIP entries during archive verification.

Binary archives now bundle README/SPEC/RELEASE/TESTING and public Markdown docs.
Local document/anchor links stay local, while links to omitted source/test
evidence target the release tag on GitHub. A shared packaging workflow builds
all four native release targets in PR/main CI and Release, verifies the actual
archive and extracted navigation, and runs the extracted binary's help. Preview
checks create no tag or release. See [the archive policy](BINARY_ARCHIVES.md).

## Completed follow-up: C3 optional editor input

Merged in [PR #61](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/61)
(merge `b7a2ce9`), including Unix terminal restoration, consistent edited-file
errors and the macOS pending-input adjustment in terminal-mode regressions.

Issue/PR create/edit/comment and inline create/edit/reply support explicit
`--editor/-e`, exclusive with body/file input. Existing edits preload the current
body; selection, private temporary data, subprocess exit/cancellation and
unchanged-text behavior are defined in [input contracts](INPUT_DISPLAY.md#explicit-editor-input).
File/stdin defaults remain intact; noninteractive/JSON-error mode never starts
an editor. No new server-side atomicity guarantee is claimed.

## Remaining release work

The C1–C4 client/release-policy follow-ups are implemented. Choosing a release
version, publishing a crate, tagging or dispatching a release is a separate
maintainer action; the completed client/release-policy changes perform none of them.
See [release gates](../RELEASE.md#preparation-gates).
The [unreleased notes and upgrade guide](NEXT_RELEASE.md) now provide the
release-note draft, configuration setup and candidate/publication handoff.
They select no version and do not treat their own validation as a future
release candidate's evidence.

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
