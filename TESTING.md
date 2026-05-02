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
