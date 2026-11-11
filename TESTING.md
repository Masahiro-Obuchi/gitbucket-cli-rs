# Testing Guide

Last updated: 2026-05-02

## Purpose

This document explains the current automated test layout for `gb`.
Use it when you want to know:

- which test file covers which behavior
- which command to run for fast feedback
- where a new test should be added

## Fast Commands

Run these during normal development:

```bash
cargo fmt --all
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Run these before opening a PR or creating a release tag:

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

`cargo test` and `cargo test --locked` start local mock HTTP listeners. If they fail in a restricted sandbox with `Operation not permitted`, rerun them in a normal local shell before treating the result as a real test failure.

Useful focused runs:

```bash
cargo test --test api_command
cargo test --test completion_command
cargo test --test config_command
cargo test --test config_resolution
cargo test --test label_command
cargo test --test milestone_command
cargo test --test api_auth_repo_flows
cargo test --test issue_create_state_comment_flows
cargo test --test issue_edit_flows
cargo test --test pr_create_flows
cargo test --test pr_edit_flows
cargo test --test pr_list_state_comment_flows
cargo test --test git_regressions
cargo test --test e2e_smoke -- --ignored --nocapture
cargo test <name>
```

## Current Test Layout

### Unit tests in `src/`

These cover pure logic and formatting behavior.

- `src/config/auth/`
  host canonicalization, default-host selection, profile resolution, protocol resolution, config removal behavior, file permissions
- `src/cli/common/`
  repo parsing, git URL parsing, list/edit state validation, and shared context resolution
- `src/api/`
  base URL normalization, endpoint path construction, pagination link handling, and web URL generation
- `src/output/mod.rs`
  UTF-8-safe truncation
- `src/output/table.rs`
  ANSI stripping and table helper behavior
- `src/cli/auth.rs`
  login error mapping
- `src/cli/pr/`
  PR creation head syntax validation and PR worktree helper behavior

### Integration tests in `tests/`

These execute the real CLI binary as a subprocess.

- `tests/api_command.rs`
  raw API command behavior, path normalization, JSON body handling, and empty success responses
- `tests/config_command.rs`
  local config command behavior, canonical saved-host lookup, profile config, and config-only error handling
- `tests/completion_command.rs`
  shell completion generation for supported shells and completion help output
- `tests/config_resolution.rs`
  invalid `--state` handling, host/token/repo/protocol precedence, profile selection behavior
- `tests/label_command.rs`
  mocked HTTP request paths, JSON output, color validation, and delete behavior for label flows
- `tests/milestone_command.rs`
  milestone list/view/create/edit/delete request shapes, due-date validation, and GitBucket web fallback behavior
- `tests/state_requests.rs`
  `issue list` and `pr list` state query parameters
- `tests/api_auth_repo_flows.rs`
  mocked HTTP request paths and payloads for auth and repo create/fork/delete flows
- `tests/issue_create_state_comment_flows.rs`
  mocked HTTP request paths and payloads for issue create/reopen/close/comment flows, including `--edit-last`
- `tests/issue_edit_flows.rs`
  issue metadata edit request shapes and web fallback constraints
- `tests/pr_create_flows.rs`
  PR create payloads, cross-repo head syntax, JSON output, and `--detect-existing`
- `tests/pr_edit_flows.rs`
  PR title/body/state/assignee edits and explicit web fallback behavior
- `tests/pr_list_state_comment_flows.rs`
  PR list state recovery, close/merge/comment flows, comment JSON, and comment editing
- `tests/pr_command.rs` and `tests/issue_command.rs`
  help text and structured JSON error behavior for representative commands
- `tests/view_flows.rs`
  view rendering, comments, JSON output, and representative 404 API error handling for repo, issue, and PR commands
- `tests/git_regressions.rs` with modules under `tests/git_regressions/`
  regression coverage for previously fixed CLI bugs, including git-heavy flows such as `repo clone`, `pr create`, `pr checkout`, and `pr diff`
- `tests/e2e_smoke.rs` with modules under `tests/e2e_smoke/`
  ignored Docker-backed smoke tests for auth, config, raw API access, labels, milestones, issue/PR comments, PR/git flows, and representative live GitBucket write operations

## How To Choose A Test Type

Add a unit test when:

- the behavior is a pure function or local transformation
- no subprocess, network, or git repository setup is needed

Add a CLI integration test when:

- clap parsing, environment variables, config files, stdout/stderr, or exit codes matter

Add a mocked API flow when:

- the main thing to verify is HTTP method, path, query string, headers, or JSON payload
- a real GitBucket instance is unnecessary

Add or extend a git regression test when:

- the behavior depends on local git state, remotes, branches, or fetch/checkout/diff behavior

## Practical Notes

- The mocked HTTP helpers use timeouts so failures should become test failures, not hangs. Shared helpers live under `tests/support/`.
- `tests/git_regressions.rs` intentionally uses temporary git repositories and is heavier than the other integration tests.
- If a new bug fix depends on both HTTP shape and git behavior, prefer adding one focused mocked API test and one focused git regression test instead of one oversized test.
- Browser launch behavior, interactive prompt ergonomics, terminal color/readability, and live server compatibility should still be checked manually when a change affects those surfaces.

## Suggested Workflow

1. Add or update the smallest relevant test first.
2. Run the narrowest target that covers your change.
3. Run `cargo test` before committing.
4. Run `cargo clippy --all-targets --all-features -- -D warnings` before merging significant changes.

## Live E2E Smoke

A Docker-backed live smoke test scaffold exists in `tests/e2e_smoke.rs`.
These tests are ignored by default and are intended to be driven by the bootstrap scripts in `scripts/e2e/`.
Fixture clones fetch only the default branch without tags. Parallel scenarios
force-push or delete their own topic branches, which must not enter unrelated
fixtures' clone requests. Tests fetch additional PR refs explicitly when needed.

Default local flow:

```bash
./scripts/e2e/bootstrap.sh
set -a
source .tmp/e2e/runtime.env
set +a
cargo test --test e2e_smoke -- --ignored --nocapture
./scripts/e2e/down.sh
```

Path-prefixed deployments can be exercised with the same flow by setting `GB_E2E_PATH_PREFIX` before bootstrap:

```bash
GB_E2E_PATH_PREFIX=/gitbucket ./scripts/e2e/bootstrap.sh
set -a
source .tmp/e2e/runtime.env
set +a
cargo test --test e2e_smoke -- --ignored --nocapture
./scripts/e2e/down.sh
```

Bootstrap writes these environment variables to `.tmp/e2e/runtime.env`:

- `GB_E2E_HOST`
- `GB_E2E_PATH_PREFIX`
- `GB_E2E_USER`
- `GB_E2E_PASSWORD`
- `GB_E2E_TOKEN`
- `GB_E2E_REPO`
- `GB_E2E_FORK_SOURCE`
- `GB_E2E_PROTOCOL`
- `GB_E2E_BASE_URL`

The dedicated GitHub Actions workflow in `.github/workflows/e2e.yml` uses the same bootstrap contract on `main` pushes, pull requests, and manual runs, and exercises both root-path and `/gitbucket` path-prefixed deployments.
The normal Rust workflow in `.github/workflows/rust.yml` runs `cargo check`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`.

## Release Validation

The release helper runs the locked release checks before creating a local tag:

```bash
scripts/release-tag.sh v0.6.0
```

The tag version must match the root package version in `Cargo.toml` and `Cargo.lock`. After pushing a `v*` tag, the `Release` workflow repeats the release validation, builds platform archives, generates `SHA256SUMS`, and publishes the GitHub Release.

## Pinned Server Contract Tests

`tests/e2e_smoke/api_contract.rs` exercises the pinned GitBucket 4.44.0 REST/Web
contracts directly, alongside the CLI smoke tests. It verifies PR editing,
inline comment visibility, milestone replacement semantics, and label update
constraints. See [the compatibility baseline](docs/GITBUCKET_COMPATIBILITY.md)
for verified behavior and remaining fixture gaps.

The contracts read back persisted values after writes, verify rejected milestone
edits leave existing fields intact, and confirm deletion with a subsequent GET.
The comment-listing contract includes an ordinary comment as a positive control,
so an empty or malformed response cannot establish that inline comments are
excluded. Label PATCH coverage checks each missing required field and a
successful rename as well as the same-name failure.

After bootstrapping the disposable instance and exporting its runtime environment,
run only the contract tests with:

```bash
cargo test --locked --test e2e_smoke api_contract -- --ignored --nocapture
```

The existing E2E command and CI workflow run these tests too. Both root and
`/gitbucket` deployments must pass. Some assertions intentionally document
undesirable server behavior; they are not the desired user-facing CLI behavior.
When fixing the CLI, add a separate regression test proving the CLI compensates
for that behavior. Do not change the server contract to match a CLI workaround.

Unit tests that read or modify process-wide `GB_*` configuration must acquire
`crate::test_support::TestEnv`. It uses one shared lock, clears inherited CLI
configuration, and restores the original values when dropped, including during
panic unwinding. Do not introduce a module-local environment mutex or manually
clear variables without restoring them. This lock coordinates participating unit
tests; it does not make concurrent environment access by arbitrary threads safe.
Integration tests should continue to set environment variables on child commands.
The guard's restoration regression test runs in an isolated child test process
and covers absent, empty and populated values, panic unwinding, and poisoned-lock
recovery (including non-Unicode values on Unix).

## Git Credential Security Regression Tests

`tests/git_credentials.rs` drives the real Git executable against local HTTP
challenge servers. It checks scoped authentication, URL rewrites to another
origin/deployment, rejection of authenticated redirects and non-HTTP transports,
private clone authentication, password-URL rejection, and non-persistence through
an existing credential-store helper. Redirect coverage includes URL-specific
settings after a rewrite adds a username and settings with a trailing slash.
Encoded deployment paths and clone URLs ending in a slash exercise Git's
normalized credential context.
The redirect target replies on unexpected connections so a regression fails
without hanging Git. Accepted mock sockets explicitly use blocking I/O with
timeouts on every platform, including Windows.
The helper executable path deliberately
contains a space and a single quote. Run it with:

```bash
cargo test --locked --test git_credentials
```

The Rust workflow runs this integration target on macOS and Windows in addition
to the full Linux suite. The Docker suite includes a private repository clone
that checks the saved remote URL and Git configuration for credentials. Internal
helper environment variables are subprocess-only implementation details, not
public configuration options.

## Repository Target and API Boundary Regressions

`tests/target_validation.rs` uses real temporary Git remotes and local HTTP
listeners to verify that origin mismatches, malformed repository names, API path
escapes, and redirects are rejected before sending requests to an unintended
endpoint. Positive cases cover root/subpath HTTP URLs, SSH URLs with independent
ports, and explicit/profile repository precedence.
Regression cases also cover trailing slashes in clone URLs, ASCII hostname case
in SSH origins, and raw/repository API requests under encoded deployment paths.
Equivalent encodings in root-relative pagination and origin-text paths must not
duplicate the deployment prefix. Encoded separators across boundary components
and decoded traversal remain rejected.

```bash
cargo test --locked --test target_validation --test api_command
```

The Docker suite also runs origin-derived Issue listing without `GB_REPO`, so
both root and `/gitbucket` deployments exercise repository inference against the
live server. The pure target/URL tests cover IPv6, encoded path traversal, and
base-URL validation separately.

## Configuration Storage Regressions

`tests/config_storage.rs` starts independent CLI processes to exercise concurrent
profile updates, login suspended during HTTP authentication, readers during
repeated atomic replacements, lock timeouts, and lock release after process
termination. Malformed TOML must remain unchanged on failure. Unit cases inject
partial writes and rename failures and cover no-op byte preservation, Unix
permissions, and symlink rejection. Windows unit cases hold readers that deny
delete sharing to verify retry/release and bounded failure with old-byte retention
and temporary-file cleanup. All credentials in these tests are dummy values.

```bash
cargo test --locked --test config_storage
cargo test --locked --bin gb config::
```

The platform workflow runs both commands on macOS and Windows; the full Linux
suite includes them. Platform-specific Unix checks are conditional. Local tests
exercise the local filesystem, not power-loss durability or all network filesystems.

## Safe PR Checkout Regressions

`tests/pr_checkout.rs` uses real repositories and a scripted API to check initial
creation, repeat checkout, local commits, advanced/force-pushed PRs, unrecorded
branches, target collisions (repository/deployment/host), dirty staged/unstaged/
untracked files, ignored file conflicts, linked worktrees, concurrent lock
ownership, detached HEAD, operations in progress and fetch/checkout/config-write
failures. Refusal cases compare HEAD, branch tips, index diff, working diff and
files before and after. Fetch isolation checks tracking refs, tags, FETCH_HEAD
and legacy internal refs. A Unix hook case checks the explicit partial-completion
error when a post-checkout hook fails after Git has switched branches.
Submodule cases check that ignore settings cannot hide uncommitted work, and
that recursive Git settings cannot overwrite ignored files, move submodule HEADs
or update their tracking refs and `FETCH_HEAD` during PR checkout.
Long deployment URLs are exercised through both checkout and diff with
`core.longpaths=false`; internal ref paths stay bounded and branch provenance
retains the complete target.

```bash
cargo test --locked --test pr_checkout --test git_credentials --test git_regressions
```

The platform workflow also runs `pr_checkout` on macOS and Windows. Local Git
2.34.1 is covered; the worktree check uses NUL-delimited `for-each-ref` atoms,
without requiring the newer `worktree list -z` option.

## Milestone Replacement and Web Boundaries

`tests/milestone_command.rs` checks complete replacement payloads for single-field
edits, retention of closed state and exact existing timestamps, explicit JSON-null
clears, null retention, no-change/invalid-input rejection, REST error propagation
without password prompts, and known title-validation-only Web supplementation.
Web cases cover Unicode/space titles, preserved state, refreshed values, lossy
form rejection before authentication, sentinel dates, duplicates, missing
resources, validation pages and failed post-submit verification.
Trim regressions exercise Java-trimmed control characters before authentication
and exact preservation of fullwidth/nonbreaking spaces in Web submissions.

```bash
cargo test --locked --test milestone_command
```

The pinned Docker contract test exercises the server's full replacement behavior,
required title and null clears directly. Live CLI regressions compare every
editable field after each single-field edit, including closed_at retention and
non-midnight timestamps, with GB_PASSWORD removed. Unicode-title cases exercise
the Web-only actions and switching back to a REST-compatible title without a
password. Fullwidth spaces in titles and nonbreaking spaces in descriptions are
compared exactly after Web edits. All run under both root and /gitbucket
deployments in the E2E workflow.

## PR REST Editing and Target Validation

`tests/pr_edit_flows.rs` verifies that content/state use only PR PATCH, including
title/body pairing for the pinned server, empty-body clears, absent-body retention,
no-op state/assignee requests, closed/merged handling, HTTP failures, Web permission
and sign-in failures before REST mutation, and mixed edit results/partial failures.
`tests/pr_target_validation.rs` checks all numbered PR command paths against 404,
200/null, mismatched-number and forbidden responses. The null compatibility lookup
may read only the PR collection; it must not reach Issue writes, comments or prompts.

```bash
cargo test --locked --test pr_edit_flows --test pr_target_validation --test pr_list_state_comment_flows
```

Pinned Docker contract coverage directly proves that body-only PR PATCH is ignored
and title-only PATCH clears the body, so the CLI compensates with a content pair.
Live CLI tests run edits/close/reopen without GB_PASSWORD (also with --web on a REST
edit), compare stored fields after every step, reject merged reopening, and verify
that ordinary Issues/comments stay unchanged after attempted PR commands. The
existing mixed Web assignee E2E test verifies the resulting assignee set. Both root
and /gitbucket deployments run these tests.

## Label Delta Updates and In-Place Web Color Edits

`tests/label_command.rs` covers color-preserving rename, required complete PATCH
inputs, REST-first behavior, explicit Web permission, ID/form verification,
unsupported descriptions before mutation, no-op requests, failed verification
and unsafe names. `tests/issue_edit_flows.rs` checks dedicated add/remove requests,
no Issue PATCH or password for label-only edits, preservation of unrelated labels,
preflight of missing additions, metadata-first verification, and partial failures.
Issue create and label deltas also distinguish padded names from unpadded names,
including ASCII spaces, NBSP and fullwidth spaces.

```bash
cargo test --locked --test label_command --test issue_edit_flows
cargo test --locked --bin gb api::web::label
```

The parser unit tests use actual sanitized 4.44.0 row/form captures and reject
ambiguous rows, changed identity and form values. The live E2E follows a numeric
label ID and Issue association through REST rename and Web color update, then
checks description rejection and token-only Issue label deltas. Root and
/gitbucket deployments both run it, alongside the direct server contract checks.
The live delta flow preserves NBSP-padded labels and adds/removes ASCII-padded
labels by their exact names.

## Web Authentication and Mutation Outcomes

`tests/web_outcomes.rs` verifies cookie-only identity probes, explicit actor
selection, preflight failures and confirmed/failed/unverified/unattempted phases.
Redirect regressions distinguish direct Unicode-URL rejections from failures
after redirects back to the same URL, allow route fragments, and block encoded
and repeatedly encoded traversal during password sign-in.
`tests/repo_mutation_flows.rs` covers fork destination/parent proof and deletion
readbacks. Public fork verification covers existing and newly created forks when
the deployment prefix matches the destination or upstream owner, with both
deployment-prefixed and unprefixed header links and no Web password.
The live E2E runs these operation adapters at root and `/gitbucket`.
Milestone regressions also reject a mismatched refreshed target before accepting
an already-satisfied state or attempting another mutation.

```bash
cargo test --locked --test web_outcomes --test repo_mutation_flows
```

## Transport Deadlines and Response Contracts

`tests/transport_failures.rs` exercises stalled headers, a stalled body, TLS
handshake stalls, a trickling body that exceeds total time, and abrupt truncation.
It checks causes/statuses, write uncertainty, request counts (no retries), bounded
credential-safe diagnostics, ordinary JSON strings/status-body objects, empty/raw
versus typed contracts, malformed merge envelopes, Web identity/body failures,
and invalid timeout configuration before mutation. Existing create/merge tests
retain positive compatibility cases; direct 4.44 E2E captures the actual PR-create
string wrapper and three-field merge ActionResult using an isolated base branch.

`tests/git_timeouts.rs` runs a real Git clone with a fake SSH command that launches
another process. It checks termination of both helpers and descendants; Unix also
sends Ctrl+C to the CLI and checks cleanup before return. A blocked read-only
Git configuration FIFO preserves its timeout without claiming a write. Windows uses native
process liveness checks for the timeout case. A unit regression covers a leader
that exits while its descendant still holds stdout/stderr pipes. No real SSH
server is contacted. Tests isolate the timeout environment variables.

`tests/cancellation.rs` uses a Unix pseudo-terminal to cancel hostname/token
prompts with SIGTERM and an actual Ctrl+C key, and checks a blocked
`gb api --input -` pipe. PR checkout tests cancel a running post-checkout hook
after HEAD has changed and verify that
the mutation's cancelled/unverified error and provenance survive failed readback.
The macOS/Windows CI jobs also run the Git deadline suite and applicable input
cancellation cases.
Missing snapshot regressions ensure advertised signed diff URLs are neither
fetched nor exposed in diagnostics.

```bash
cargo test --locked --test transport_failures --test git_timeouts
cargo test --locked --bin gb api::client::tests
cargo test --locked --bin gb git::process::tests
```

The Git supervisor uses process-wrap's Unix process groups / Windows Job Objects.
Its nix dependency requires libc >=0.2.186; the lockfile's libc update accompanies
that requirement. TLS continues to use rustls with native trusted roots.


## Complete Collections and Existing PR Detection

`tests/pagination_flows.rs` covers fixed Issue/PR pages at 0/1/25/26/50/51,
all-return comments through 101 rows, relative/absolute/multiple Link fields,
filter preservation, exact terminal pages, deduplication, cycles, ignored pages,
unsafe URLs and configuration/cap failures. Later-page failures never return a
partial array or allow PR creation. Null detail searches and failed-create
rechecks traverse all pages, preserve errors and never retry the POST.
Regressions cover unrelated canonical/self/schema Link relations, terminal pages
with no new IDs, and SIGTERM during a failed-create recheck. Both text and JSON
errors retain the uncertain POST result; failed identity checks never claim that
an earlier create was not applied.
View/list/edit-last share linked comment retrieval. PR list/create regressions
also verify canonical fork identities and failure before creation when detail
cannot establish identity; no Issue-listing recovery is assumed.

```bash
cargo test --locked --test pagination_flows --test pr_create_flows --test pr_list_state_comment_flows
```

The ignored live contract `contract_fixed_pages_all_comments_and_canonical_fork_identity`
creates an isolated repository with 51 Issues, 51 PRs and 51 comments. It checks
all boundary counts directly over REST and through the CLI, open/closed/all,
the oldest PR beyond page two, latest-comment editing and a real root-owned fork.
It reproduces the 4.44.0 collection's wrong fork ownership and verifies canonical
JSON hydration and existing-PR lookup. Run it with the other E2E tests at root
and `/gitbucket` using the dedicated Docker fixture. Fixture records remain in
the test volume for inspection. See [pagination design](docs/PAGINATION.md).


## Saved PR Commits, Diff and Checkout

`tests/pr_snapshots.rs` uses real repositories to verify exact saved SHA behavior
for open/closed/merged PRs after branch reuse/base advancement, retained PR refs
after deletion/force-push, shallow clones, missing/invalid objects, null or fully
read 200/empty detail recovery, transport/decode failures without recovery,
replacement-object suppression, offline cached objects, empty trees,
rename/binary/large patches, subdirectory invocation and a real TTY pager.
Pager cancellation covers both waiting for exit and a blocked large-patch write,
requiring a `cancelled` error and a reaped child.
Existing checkout and Git credential regressions still verify non-destructive
branch behavior, cancellation outcomes, authentication scopes and redirect rules.

```bash
cargo test --locked --test pr_snapshots --test pr_checkout --test git_credentials --test git_regressions --test transport_failures
```

The three new Docker cases in `pr_snapshot.rs` and `api_contract.rs` check closed
PR reuse/deletion, merged PR branch advancement and fork deletion. The latter
explicitly observes 200/zero-byte canonical detail, saved collection SHAs and the
advertised refs/pull/N/head before checking CLI recovery. Run the whole E2E suite
at root and /gitbucket. [Snapshot contracts](docs/PR_SNAPSHOTS.md) describe
API/Git boundaries and unrecoverable object loss.


## Original API JSON and Comment Shapes

`tests/json_contract.rs` compares complete response objects: unknown/nested data,
Unicode, missing/null/false, unchanged collection arrays, canonical PR identity
with unknown summary metadata, Issue count plus comment_details, PR comments plus
api_comments, pagination failure without partial JSON, reserved-key conflicts,
create wrappers and comment POST responses. Pinned augmented shapes are under
`tests/fixtures/json/`. Existing mutation tests continue checking exact request
DTOs, and PR metadata text tests distinguish known draft from missing draft.
Raw HTTP-body cases verify precise unknown integers/decimals in view and
canonical list composition, while invalid known IDs still fail validation.

```bash
cargo test --locked --test json_contract
```

The ignored live case
`contract_cli_json_preserves_api_fields_and_separates_issue_comment_details`
compares CLI JSON with REST for a real draft PR and Issue, including ordinary
comments. It verifies preserved id/URL metadata, nullable mergeability and the
4.44 Issue's absent comment count at root and /gitbucket. See
[JSON design](docs/JSON_OUTPUT.md). The review/inline reader and its expanded
coverage are described below.

## Review Conversation Reader

`tests/pr_review_comments.rs` and Web parser unit cases exercise whitespace-bearing
paths, partially changed review groups, qualified identities across
ordinary/review stores, scope/unknown fields, shared view/list
ordering, kind filters, missing detail recovery, public password-free reads,
Cookie-only private reads, unknown/mismatched markup, duplicates, auth expiry,
HTTP errors, head changes and no partial output. The focused sanitized fixture is
`tests/fixtures/gitbucket-4.44-review-conversation.html`; user-body markup cannot
add trusted records. Missing/both line values remain displayed data, without
inventing Git validation or a reply-parent relationship.

```bash
cargo test --locked --test pr_review_comments
cargo test --locked --bin gb api::web::review
```

The live contracts `contract_review_comment_markup_and_visibility` and
`contract_private_review_reads_require_cookie_session_after_token_denial` run at
root and /gitbucket. They use real rename/deleted files, multiple authors,
old/new lines, same-position additions, whole-commit reviews, updated/outdated
positions and force-push; view/list/filter output is compared with visible markup.
The discussion edit-last path remains isolated. Public reads explicitly remove
password/user overrides; the private fixture checks token-only 401 before Cookie
reading. [Scope and server limits](docs/REVIEW_COMMENTS.md) distinguish these
successful reads from complete stored-history or original-Markdown guarantees.

## Inline Review Writes

`tests/pr_review_write.rs` uses local saved Git objects and HTTP scripts to check
Cookie-only payloads, stored diff coordinates/snippets, exact original Markdown,
same-ID namespace isolation, author/permission/outdated/position rejection,
head changes, body/resource mismatch, definite rejection and truncated responses.
An uncertain response is reconciled with reads only; failed verification retains
the original transport cause/status and never reports success. A retry of a
reserved create intent reuses a confirmed result without another POST. Root
paths and pathspec metacharacters are checked from a nested working directory.
Git preference regressions cover `diff.suppressBlankEmpty=true` with a blank
context row and `GIT_DIFF_OPTS=--unified=0` with a context-line target; both assert
the posted coordinates and stored snippet. An inherited `--unified=100` cannot
expand the supported hunk, and `GIT_ICASE_PATHSPECS=1` cannot mix case-distinct
files into the selected patch (Linux filesystem regression).
Receipt tests cover immutable baselines, changed/corrupt intent rejection and
symlink directory refusal. Additional regressions cover Unicode repository names,
PR closure/merge immediately before submission, read-only matching edits and
binary/non-UTF-8/symlink/submodule rejection before Web authentication.

```sh
cargo test --locked --test pr_review_write
cargo test --locked --bin gb review_
```

The live contracts `contract_cli_inline_create_edit_reply_delete_and_retry` and
`contract_cli_inline_old_new_rename_deleted_and_context_positions` run in root and
/gitbucket deployments. They create/list/edit/reply/delete real inline comments,
verify matching Web groups, preserve Markdown and test retry reuse. They also
compare modified old/new, renamed new/context and deleted old-line positions and
verify that a new explicit request key permits an intentional second identical
submission. See
[write evidence and limits](docs/REVIEW_WRITES.md). No business server is mutated.

PR-13 local validation on 2026-11-10: 479 ordinary tests passed, with 42 Docker
E2E cases ignored by default. The complete live suite passed all 42 cases in
both root and `/gitbucket` deployments. The first concurrent subpath run had
API response timeouts (26 passed, 16 failed); proxy logs showed requests closing
before a response. A rerun with `--test-threads=4` passed all 42. `cargo check`,
Clippy with `-D warnings`, formatting and diff whitespace checks passed too.

## Authentication Scopes and Credential-Key Migration

`tests/auth_profiles.rs` checks profile/global edit isolation, explicit scope
conflicts, scoped logout and retained routing/fallback, environment-only tokens,
online actor verification versus saved hints, rejected/unverified status, and
offline operation without a request. Saved credentials are tested across
schemes, ports, deployments, aliases and legacy default references.
Regressions also cover explicit default selection through global legacy keys,
`host --default` and promoted global defaults with both schemes saved, and
profile logout preserving its original HTTP instance rather than selecting an
HTTPS fallback. Migration refuses ambiguous bare defaults in global/profile
scopes during preview and apply; an explicit URL resolves that ambiguity.

Migration tests compare preview/apply/no-op/restore with exact original bytes,
unknown TOML fields, IPv6 and subpaths, identical aliases, conflicting tokens,
unknown default schemes, later-edit restore refusal and Unix backup permissions.
Unix symlink cases verify that neither config nor backup links are followed.
Corrupt-backup errors are checked for secret omission and unchanged config bytes.
The existing process-lock and atomic-replacement tests exercise the shared lock
and persistence code used by migration. macOS/Windows CI now runs auth_profiles
alongside configuration storage tests.

```sh
cargo test --locked --test auth_profiles
cargo test --locked --test e2e_smoke auth_config -- --ignored --test-threads=4
```

The live profile case verifies the actual `/user` login despite a stale saved
hint, profile and global fallback after logout, isolation of another profile,
normalization of a real legacy credential and exact rollback without losing
authentication. It runs in root and `/gitbucket` fixtures.

PR-14 validation on 2026-11-11: 496 ordinary tests passed; all 43 live E2E cases
passed in each deployment. `cargo check`, Clippy with `-D warnings`, formatting
and diff checks passed. The first three added regressions failed before the
scope/scheme fixes. [Behavior, JSON changes and migration limits](docs/AUTH_PROFILES.md)
are documented separately. No business server credentials are migrated by these
tests; all config mutations use temporary directories.
