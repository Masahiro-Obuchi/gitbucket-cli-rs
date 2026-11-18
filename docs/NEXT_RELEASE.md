# Next release: notes and upgrade guide

**Unreleased. No next version, tag or publication has been selected.** These
notes describe the merged PR-00–17 refactor and C1–C4 follow-ups through
[PR #61](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/61)
(merge `b7a2ce9`). They are a draft for the next release, not release notes for
an existing artifact.

The published [GitHub release v0.6.1](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/releases/tag/v0.6.1)
points to source tag `v0.6.1` (`fc45ae2`), before this series. The current manifest
also says `0.6.1`; `gb --version` alone cannot distinguish these source sets.
Keep the selected artifact's tag, source commit and checksum with an upgrade
record. Check crate-registry publication separately; a GitHub Release does not
establish which crate versions are available.

## Draft release notes

The release changes authentication scope and status verification, automation
output/input contracts, and local Git behavior. Review the migration steps below
before replacing a binary used by scripts.

- Authentication matches the selected scheme, host, port and deployment path.
  Git uses scoped credentials without embedding tokens in remote URLs. Profile
  management addresses its selected scope, and instance-key migration is
  explicit with a protected backup and guarded restore.
- HTTP/Git operations have deadlines and cancellation. Collection reads finish
  pagination before returning results. Writes report confirmed, failed and
  unverified outcomes without automatically resending ambiguous mutations.
- Issue/PR/milestone edits preserve unspecified fields and associations. Verified
  Web operations check their actor and stored result; PR title/body/state use
  REST, while PR assignee Web writes require explicit `--web`.
- PR diff and checkout use saved commit identities. Checkout protects existing
  branches/worktrees; inferred PR heads must match the published branch, and an
  omitted base uses the repository default branch. There is no automatic push.
- API-backed JSON retains unknown fields and distinguishes missing, null, false
  and empty values. Discussion and visible inline reviews have separate output
  contracts. Inline create/edit/reply/delete use validated saved positions,
  actor checks, verification and local retry receipts.
- Explicit UTF-8 file/stdin bodies and terminal-only `--editor/-e` are supported.
  Editor edits preload current text; unchanged text, failures and cancellation
  stop before content writes. Unix terminal modes are restored after editor exit.
  Human output escapes unsafe controls; JSON and raw patches retain data.
- Cargo builds require Rust 1.88.0 or later with the locked dependencies. System
  certificate trust remains supported. Binary archives include the guides and
  public Markdown documentation, with extracted navigation checks on all four
  release targets. Cargo source-package paths are explicitly selected.

These notes do not add formal approvals/request-changes/dismissal, complete
stored review history, atomic expected-head merge or persisted label descriptions.
The verified baseline remains **GitBucket 4.44.0 core at root and `/gitbucket`**;
other deployments/plugins/SSO require separate verification. See
[the support matrix](REVIEW_MERGE.md), [label limits](UPSTREAM_LABEL_API.md) and
[remaining server/deployment work](REFACTOR_STATUS.md#remaining-server-and-deployment-requirements).

## Upgrade procedure

### 1. Record the current installation and configuration scope

Keep the previous executable/artifact available while evaluating the new one.
Record its version along with its original artifact identity:

```sh
gb --version
```

For **all remaining examples**, `gb` means the separately staged **candidate
executable**, using its full path when necessary. Do not overwrite the installed
binary just to run these checks. Older binaries may not understand newer options
such as `auth status --offline`. Locate configuration through the candidate
rather than assuming the default directory:

```sh
gb config path
gb config list --json
gb auth status --offline --json
```

The inventory commands omit token values, but their host/profile/user metadata
can still be private. Do not attach actual configuration, migration backups,
cookies or tokens to a release report. `--offline` does not verify a token or
prove the actor's identity. Use a separate candidate executable path when
comparing old and new behavior; the identical manifest version is insufficient.

Inspect the intended `--profile`, `GB_PROFILE`, saved default profile and global
fallback before changing credentials. An environment token wins at runtime, and
profile logout can leave environment/global fallback usable. `--global` selects
global management and conflicts with explicit profile selection. Scope details
and command examples are in [AUTH_PROFILES.md](AUTH_PROFILES.md).

### 2. Preview instance-key migration, then apply only if needed

Use the **candidate executable** for these commands. Whole-file migration must
run without `--profile`, `--global` or `GB_PROFILE`; temporarily clear an exported
`GB_PROFILE` in the current shell if necessary. Saved default profiles remain
part of the configuration being migrated.

```sh
gb config migrate
```

This previews the whole file without rewriting configuration or creating a
backup. Review normalized schemes/base paths and any collisions. Resolve an
ambiguous HTTP/HTTPS target explicitly rather than dropping one credential or
guessing a scheme. Migration is optional when no normalization is needed.

After reviewing the preview, the user can explicitly apply it:

```sh
gb config migrate --apply
```

Apply recalculates under the configuration lock and reports a unique backup
path. Keep that backup private: it contains original credential bytes. Migrated
TOML can lose comments/formatting; the backup preserves the original bytes.
The guarded recovery command is:

```sh
gb config migrate --restore /path/reported/by/apply.json
```

Restore refuses to overwrite subsequent configuration edits. Reconcile such
edits explicitly instead of forcing an old backup over newer credentials.
Do not use an older binary to rewrite migrated configuration as a rollback
shortcut. [Migration and restore details](AUTH_PROFILES.md#explicit-migration-and-guarded-restore).

### 3. Verify the selected target and actor

An online status probe now verifies the effective token against the selected
instance. Use the real deployment URL, including any path prefix; substitute an
existing profile only when that is the intended scope:

```sh
gb auth status -H https://bucket.example/gitbucket --json
gb auth status --profile work -H https://bucket.example/gitbucket --json
```

Check the exit status as well as `effective_actor.verification` and `user`.
Inventory usernames and `stored_user` remain unverified hints. A rejected or
unverified probe exits 1 even when a JSON status record was printed. For Git over
HTTP with an environment `GB_TOKEN`, supply the matching `GB_USER`; a saved
username is not inherited for an unrelated environment token. Web operations
also verify the Cookie actor; an explicit `GB_WEB_USER` permits a different actor.

### 4. Update automation before enabling writes

| Area | What to change or check |
| --- | --- |
| Status | Use `auth status --offline` for inventory; default status now makes a live request. Do not equate successful JSON parsing with exit 0 or verified identity. |
| API JSON | Check field presence explicitly; missing keys no longer become synthesized null/false/empty values. Keep existing API fields instead of assuming a fixed projection. |
| Comments | Issue and PR view use `comment_details` for discussion. PR comment list always returns an object with `comments`, `review_comments` and `comment_coverage`. |
| Errors | Handle exits 0/1/2 and `error.code`/`cause`; inspect `outcome=unverified` before any retry. `--json-errors` disables prompts and editor launch. |
| Creation/input | Supply `--title` in automation. An omitted body is optional; stdin is consumed only with `--body-file -`. Do not combine body sources. Use `auth login --with-token` for token stdin. |
| PR selection | Omitted `--base` uses `default_branch`. An inferred head must match the published commit; explicit `--head` deliberately selects remote content. Push remains a separate user action. |
| Checkout/diff | `pr diff` compares saved commits and excludes working-tree edits; it does not require a clean worktree. `pr checkout` requires a clean worktree/index, including untracked files, and refuses conflicting branches/worktrees. Rename or reconcile a legacy conflicting branch explicitly; no automatic branch reset is performed. |
| Merge JSON | Consume the CLI report, with `atomic=false` and the API result under `result`. `--match-head-commit` pins a full reviewed SHA for client checks, not a server transaction. |
| Inline retries | Retain `GB_CONFIG_DIR/review-writes` and stable request IDs when reconciling uncertain submissions. Do not delete receipts and blindly resend a comment. |

For body input and `--editor` behavior, see [INPUT_DISPLAY.md](INPUT_DISPLAY.md).
For JSON shapes, use [JSON_OUTPUT.md](JSON_OUTPUT.md) and its linked fixtures.
[CLI_ERRORS_OUTPUT.md](CLI_ERRORS_OUTPUT.md) defines errors and pipe/pager behavior;
[PR_SNAPSHOTS.md](PR_SNAPSHOTS.md) and [REVIEW_WRITES.md](REVIEW_WRITES.md) define
saved-commit and retry limits. Test affected jobs against an authorized disposable
repository before allowing production mutations.

### 5. Check compatibility and recovery expectations

Build with the selected commit's lockfile and [Rust version policy](RUST_VERSION.md).
Extract the entire binary archive so local guide links remain available. Source
evidence links require network access to the matching release tag. Existing
archives are not retroactively updated; see [BINARY_ARCHIVES.md](BINARY_ARCHIVES.md).

Switching back to a prior executable does not undo completed server mutations,
restore removed credentials, erase retry uncertainty or make an unverified write
safe to repeat. Keep the relevant operation result and inspect actual state.
Do not reset user branches or remove retry receipts as an automatic rollback.
Configuration restore has the guarded procedure above; deployment-specific
backup/recovery remains the operator's responsibility.

## Maintainer handoff: still pending before publication

| Item | State at this preparation PR | Completion evidence |
| --- | --- | --- |
| Version and release notes | **Not selected / unreleased** | Choose a new version, update manifest and lockfile, and finalize these notes with the chosen tag. |
| Candidate source identity | **Not frozen** | Record the exact committed revision containing the version change; a validation of an earlier docs PR is not evidence for that future candidate. |
| Ordinary tests, MSRV and platform/archive checks | **Must run on the candidate** | Successful CI for that exact revision plus recorded toolchains/targets and package inventories. |
| Full root/subpath E2E | **Must run on the candidate** | Complete pinned 4.44.0 suites at both layouts; ordinary tests ignore these cases. |
| Intended deployment (D1) | **Unverified** | Exact server build/base path/auth/plugin/SSO/Git/TLS details and relevant checks in an authorized repository. Keep unverified deployments explicit in the release scope. |
| Source crate publication readiness | **Registry state not established here** | Inspect the exact source archive and run the dry-run gate; independently check the chosen crate version/ownership/publication state. |
| Crate, tag and GitHub Release | **Not performed by this PR** | Maintainer-authorized publication after the gates, with each publication result recorded separately. |

Follow [RELEASE.md](../RELEASE.md#preparation-gates) for commands and publishing
order. Its tag helper creates a local tag and its tag-push workflow publishes a
GitHub Release; neither belongs in a documentation-only validation run.
No gate above becomes passed merely because this guide or its links were checked.

Version selection should account for automation compatibility and the Rust
requirement, rather than assuming a patch update. Cargo's
[compatibility guidance](https://doc.rust-lang.org/cargo/reference/semver.html)
also describes pre-1.0 version conventions; the final version is a maintainer
decision. A published crate version cannot be overwritten, and crate publication
is separate from GitHub artifacts; see
[Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html).
