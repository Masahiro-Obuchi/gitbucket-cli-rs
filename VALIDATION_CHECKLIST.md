# Validation Checklist

Last updated: 2026-03-20

## Purpose

This checklist is for validating the current `gb` implementation.
It is organized so you can use it for:

- local development checks
- manual verification against a real GitBucket instance
- future automation work

## Recommended Validation Levels

### Level 1: Fast local checks

Run on every change:

```bash
cargo fmt --all
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

### Level 2: CLI and behavior checks

Validate command behavior, environment variable precedence, and error handling.
These are good candidates for integration tests.

### Level 3: Real-environment checks

Validate against a real GitBucket instance and an actual git repository.
These checks are necessary for API compatibility and git command behavior.

## Validation Matrix

| Area | What to validate | Why it matters | Recommended method |
| --- | --- | --- | --- |
| Build and lint | Format, compile, lint, tests pass | Baseline correctness | Automated |
| Auth config | Host selection, token loading, protocol resolution, file permissions | Prevents broken auth and unsafe config handling | Automated + manual |
| Host/repo resolution | `--hostname`, `GB_HOST`, `--repo`, `GB_REPO`, git remote parsing | Core command targeting | Automated |
| API behavior | Auth, repo, issue, and PR API requests | Confirms request paths and payloads | Automated + manual |
| Git integration | `repo clone`, `pr checkout`, `pr diff` | Depends on local git behavior and remote shape | Manual + E2E |
| Output behavior | Table layout, JSON output, UTF-8 truncation, colored states | User-facing correctness | Automated |
| Error paths | Auth failures, missing repo, invalid state, API errors | Prevents misleading UX | Automated + manual |

## Detailed Checklist

### 1. Build and static validation

- [ ] `cargo fmt --all` completes with no diffs.
- [ ] `cargo check` succeeds.
- [ ] `cargo test` succeeds.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` succeeds.
- [ ] No newly introduced warnings remain in normal builds.

### 2. Authentication and configuration

#### 2.1 Login and token verification

- [ ] `gb auth login` succeeds against a valid GitBucket instance.
- [ ] `gb auth login -H <host> -t <token>` succeeds non-interactively.
- [ ] `gb auth login` rejects an invalid token with a useful message.
- [ ] `gb auth login` reports a helpful hint when the base path is wrong and the API returns 404.
- [ ] `gb auth token` prints the stored token for the selected host.
- [ ] `gb auth status` shows the expected username and protocol.
- [ ] `gb auth logout` removes the selected host entry.

#### 2.2 Config file behavior

- [ ] `config.toml` is created in the expected config directory.
- [ ] On Unix-like systems, `config.toml` is written with `0600` permissions.
- [ ] Successful login updates `default_host`.
- [ ] Removing the default host selects the next fallback host deterministically.
- [ ] Existing configs without `default_host` still work.

#### 2.3 Credential and protocol precedence

- [ ] `GB_TOKEN` overrides the token stored in `config.toml`.
- [ ] When `GB_TOKEN` is used with `-H https://host/path`, the protocol resolves to `https`.
- [ ] When `GB_TOKEN` is used with `-H http://host/path`, the protocol resolves to `http`.
- [ ] When `GB_TOKEN` is used with a plain host and `GB_PROTOCOL=http`, the protocol resolves to `http`.
- [ ] When `GB_TOKEN` is used with a plain host matching stored config, the stored protocol is reused.
- [ ] Equivalent host forms resolve to the same stored config entry:
  - [ ] `https://host/path`
  - [ ] `host/path`
  - [ ] `https://host/path/api/v3`

### 3. Host and repository resolution

#### 3.1 Host resolution

- [ ] `--hostname` takes precedence over all stored config defaults.
- [ ] `GB_HOST` is used when `--hostname` is omitted.
- [ ] Stored `default_host` is used when both are omitted.
- [ ] Backward-compatible fallback works when only host entries exist and `default_host` is absent.

#### 3.2 Repository resolution

- [ ] `--repo OWNER/REPO` is parsed correctly.
- [ ] Invalid `--repo` values fail with a clear error.
- [ ] `GB_REPO` is used when `--repo` is omitted.
- [ ] Git remote auto-detection works for:
  - [ ] `https://host/owner/repo.git`
  - [ ] `https://host/git/owner/repo.git`
  - [ ] `https://host/subpath/owner/repo.git`
  - [ ] `https://host/subpath/git/owner/repo.git`
  - [ ] `git@host:owner/repo.git`
- [ ] Missing or unparsable git remotes produce `RepoNotFound` behavior.

### 4. Repository commands

#### 4.1 Listing and viewing

- [ ] `gb repo list` lists repositories for the authenticated user.
- [ ] `gb repo list <owner>` lists repositories for the specified owner.
- [ ] `gb repo list --json` emits valid JSON.
- [ ] `gb repo view <owner/repo>` shows core repository details.
- [ ] `gb repo view --web` opens the expected browser URL.

#### 4.2 Mutating repository actions

- [ ] `gb repo create <name>` creates a repository.
- [ ] `gb repo create --private` creates a private repository.
- [ ] `gb repo create --group <group>` creates a repository under the group.
- [ ] `gb repo delete` prompts for confirmation.
- [ ] `gb repo delete --yes` skips confirmation.
- [ ] `gb repo fork` creates a fork and prints the resulting repository.

#### 4.3 Clone behavior

- [ ] `gb repo clone owner/repo` resolves clone URL and clones successfully.
- [ ] `gb repo clone <full-url>` clones without API lookup.
- [ ] `gb repo clone owner/repo <directory>` clones into the selected directory.
- [ ] Git clone failures are surfaced clearly.

### 5. Issue commands

#### 5.1 Listing and viewing

- [ ] `gb issue list` defaults to `open` issues.
- [ ] `gb issue list --state closed` returns closed issues.
- [ ] `gb issue list --state all` returns all issues.
- [ ] `gb issue list --state <invalid>` fails before making the API request.
- [ ] `gb issue list --json` emits valid JSON.
- [ ] `gb issue view <number>` shows issue details.
- [ ] `gb issue view --comments` includes comments.
- [ ] `gb issue view --web` opens the expected browser URL.

#### 5.2 Mutating issue actions

- [ ] `gb issue create -t <title>` creates an issue.
- [ ] `gb issue create` prompts for a title when omitted.
- [ ] `gb issue create -l <label>` sends labels correctly.
- [ ] `gb issue create -a <user>` sends assignees correctly.
- [ ] `gb issue close <number>` closes an issue.
- [ ] `gb issue reopen <number>` reopens an issue.
- [ ] `gb issue comment <number> -b <body>` creates a comment.

### 6. Pull request commands

#### 6.1 Listing and viewing

- [ ] `gb pr list` defaults to `open` pull requests.
- [ ] `gb pr list --state closed` returns closed pull requests.
- [ ] `gb pr list --state all` returns all pull requests.
- [ ] `gb pr list --state <invalid>` fails before making the API request.
- [ ] `gb pr list --json` emits valid JSON.
- [ ] `gb pr view <number>` shows PR details.
- [ ] `gb pr view --comments` includes comments.
- [ ] `gb pr view --web` opens the expected browser URL.
- [ ] Merged pull requests are shown as `MERGED` in human-readable output.

#### 6.2 Mutating PR actions

- [ ] `gb pr create` uses the current git branch as default head when available.
- [ ] `gb pr create` prompts for missing title/base values.
- [ ] `gb pr close <number>` closes the PR via the issues endpoint.
- [ ] `gb pr merge <number>` merges a mergeable PR.
- [ ] `gb pr merge <number>` surfaces API merge failures clearly.
- [ ] `gb pr comment <number> -b <body>` creates a comment.

#### 6.3 Checkout and diff behavior

- [ ] `gb pr checkout <number>` works for a PR from the same repository.
- [ ] `gb pr checkout <number>` works for a fork-based PR.
- [ ] `gb pr checkout <number>` creates or resets the local branch from `FETCH_HEAD` as expected.
- [ ] `gb pr diff <number>` works for a PR from the same repository.
- [ ] `gb pr diff <number>` works for a fork-based PR.
- [ ] `gb pr diff <number>` compares `origin/<base>` against the fetched PR head.
- [ ] Missing PR head metadata fails with a clear error.

### 7. Browse command

- [ ] `gb browse` resolves the repository correctly.
- [ ] `gb browse` opens the expected browser URL.
- [ ] Browser launch failures surface a useful error message.

### 8. Output and UX behavior

- [ ] Table output aligns correctly for normal ASCII content.
- [ ] Table output remains readable with ANSI-colored cells.
- [ ] Long repository descriptions truncate safely.
- [ ] Long issue titles truncate safely.
- [ ] Long PR titles truncate safely.
- [ ] UTF-8 text does not panic during truncation.
- [ ] `--json` output is valid machine-readable JSON.
- [ ] `NO_COLOR` behavior is acceptable in the current terminal environment.

### 9. Error handling

- [ ] Missing authentication produces a clear `gb auth login` hint.
- [ ] Missing repository context produces a clear repository resolution error.
- [ ] Network failures surface actionable messages.
- [ ] API 401 and 404 during login are mapped to useful auth guidance.
- [ ] Non-success API responses preserve enough detail for debugging.
- [ ] Git command failures return a clear user-facing error.

## Suggested Automation Plan

### Good candidates for unit tests

- hostname canonicalization
- protocol resolution
- state validation
- git remote URL parsing
- UTF-8 truncation

### Good candidates for integration tests

- CLI argument parsing
- environment-variable precedence
- JSON output shape
- config-file load and save behavior
- error message mapping

### Good candidates for E2E tests

- auth login against a disposable Docker GitBucket instance
- repo view against a seeded repository
- issue list `--json` against a seeded repository
- pr list `--json` against a seeded repository
- later expansion to repo/issue/pr write flows after smoke stability

## Minimal Manual Smoke Test

Use this when you want a fast real-environment check after a larger change.

1. Run local quality checks.
2. Log in to a real GitBucket instance.
3. Run `gb repo list`.
4. Run `gb issue list --state all` in a repository.
5. View one issue and one PR.
6. Run `gb pr diff <number>` for a same-repo PR.
7. Run `gb pr diff <number>` for a fork-based PR.
8. Confirm config file contents and permissions.

## Notes

- The highest confidence comes from combining automated checks with a real GitBucket test instance.
- For this project, git-command behavior and GitBucket API compatibility are the two areas most likely to require real-environment verification.
