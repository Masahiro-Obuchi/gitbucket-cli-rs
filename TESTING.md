# Testing Guide

Last updated: 2026-03-25

## Purpose

This document explains the current automated test layout for `gb`.
Use it when you want to know:

- which test file covers which behavior
- which command to run for fast feedback
- where a new test should be added

For broader validation scope, see [VALIDATION_CHECKLIST.md](./VALIDATION_CHECKLIST.md).
For long-term automation priorities, see [TEST_AUTOMATION_PLAN.md](./TEST_AUTOMATION_PLAN.md).

## Fast Commands

Run these during normal development:

```bash
cargo fmt --all
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Useful focused runs:

```bash
cargo test --test config_resolution
cargo test --test mocked_api_flows
cargo test --test regression_pre_fix
cargo test <name>
```

## Current Test Layout

### Unit tests in `src/`

These cover pure logic and formatting behavior.

- `src/config/auth.rs`
  host canonicalization, default-host selection, protocol resolution, config removal behavior, file permissions
- `src/cli/common.rs`
  repo parsing, git URL parsing, list-state validation
- `src/api/client.rs`
  base URL normalization and web URL generation
- `src/output/mod.rs`
  UTF-8-safe truncation
- `src/output/table.rs`
  ANSI stripping and table helper behavior
- `src/cli/auth.rs`
  login error mapping
- `src/cli/pr.rs`
  PR remote identity matching for fetch source selection

### Integration tests in `tests/`

These execute the real CLI binary as a subprocess.

- `tests/config_resolution.rs`
  invalid `--state` handling, host/token/repo/protocol precedence, config selection behavior
- `tests/state_requests.rs`
  `issue list` and `pr list` state query parameters
- `tests/mocked_api_flows.rs`
  mocked HTTP request paths and payloads for auth, repo create/fork, issue create/close, PR create/close/merge
- `tests/regression_pre_fix.rs`
  regression coverage for previously fixed CLI bugs, including git-heavy flows such as `repo clone`, `pr checkout`, and `pr diff`

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

- The mocked HTTP helpers use timeouts so failures should become test failures, not hangs.
- `tests/regression_pre_fix.rs` intentionally uses temporary git repositories and is heavier than the other integration tests.
- If a new bug fix depends on both HTTP shape and git behavior, prefer adding one focused mocked API test and one focused git regression test instead of one oversized test.

## Suggested Workflow

1. Add or update the smallest relevant test first.
2. Run the narrowest target that covers your change.
3. Run `cargo test` before committing.
4. Run `cargo clippy --all-targets --all-features -- -D warnings` before merging significant changes.
