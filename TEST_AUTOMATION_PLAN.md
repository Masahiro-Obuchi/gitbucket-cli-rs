# Test Automation Plan

Last updated: 2026-03-20

## Goal

This document turns [VALIDATION_CHECKLIST.md](./VALIDATION_CHECKLIST.md) into an implementation plan.
It answers a practical question:

> What should be automated first in this repository, and what should remain manual until later?

## Recommended Order

Build automation in this order:

1. Strengthen local quality gates.
2. Add unit tests for pure logic and edge cases.
3. Add integration tests for CLI behavior and config resolution.
4. Add mocked API tests for request correctness.
5. Add real GitBucket E2E tests only after the lower layers are stable.

This order is important because the repository already has some useful unit tests, but the highest remaining risk is in end-to-end behavior across CLI parsing, config resolution, HTTP requests, and git command execution.

## Priority Backlog

### Priority 0: Always-on local quality gates

These should pass on every branch before manual verification.

- `cargo fmt --all`
- `cargo check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Exit condition:

- all four commands pass locally and in CI

Why first:

- this is the cheapest way to prevent regressions
- it establishes a consistent baseline before adding heavier tests

## Priority 1: Unit tests for pure logic

Automate the logic that does not need a live GitBucket instance or process spawning.
These tests are cheap, stable, and should cover most edge cases.

### 1.1 Highest-value unit test targets

- hostname normalization and canonical matching in `src/config/auth.rs`
- credential and protocol selection rules in `src/config/auth.rs`
- repository parsing in `src/cli/common.rs`
- list-state validation in `src/cli/common.rs`
- UTF-8-safe truncation in `src/output/mod.rs`
- base URL normalization in `src/api/client.rs`
- browser URL generation in `src/api/client.rs`

### 1.2 Missing unit test cases worth adding

- `GB_TOKEN` + stored config protocol fallback for equivalent host forms
- host matching when one side includes `/api/v3`
- host matching with port numbers
- invalid state values preserving a clear error message
- `truncate()` behavior at widths `0`, `1`, `2`, and `3`
- table width calculation with ANSI-colored multibyte strings

Exit condition:

- pure logic branches have direct tests
- edge-case regressions no longer depend on manual checking

## Priority 2: Integration tests for CLI behavior

These tests should execute the built CLI as a subprocess and assert on stdout, stderr, exit code, config files, and environment-variable precedence.

Recommended tooling:

- `assert_cmd`
- `predicates`
- `tempfile`
- optional: `assert_fs`

### 2.1 Highest-value integration scenarios

#### Authentication/config

- `gb auth status` with no config
- `gb auth token` with selected host
- `GB_HOST` overriding stored default host
- `GB_REPO` overriding git auto-detection
- `GB_TOKEN` overriding stored token
- `GB_PROTOCOL` affecting plain-host requests
- config directory override with `GB_CONFIG_DIR`

#### Input validation

- invalid `--repo` format
- invalid `--state` on `issue list`
- invalid `--state` on `pr list`
- missing repo context outside a git repository

#### Output behavior

- `--json` output for repo/issue/pr list commands
- human-readable error messages for auth and repo resolution failures

### 2.2 Why these come before E2E

They verify real CLI entry behavior while remaining deterministic and fast.
They also catch mistakes that unit tests miss, such as clap parsing, environment precedence, and user-facing output.

Exit condition:

- the most important non-network CLI flows are covered by automated subprocess tests

## Priority 3: Mocked API tests

The next layer should verify that the CLI is making the right HTTP requests without needing a live GitBucket server.

Recommended approaches:

- start a lightweight mock HTTP server in tests
- point `GB_HOST` or `--hostname` to the mock server
- feed the CLI predefined JSON responses

Possible tooling:

- `wiremock`
- `httpmock`
- a small custom `tokio` test server if you want minimal dependencies

### 3.1 Highest-value mocked API scenarios

#### Auth

- `GET /user` success on login
- `GET /user` returns `401`
- `GET /user` returns `404`

#### Issue and PR listing

- `issue list --state open` sends `?state=open`
- `issue list --state closed` sends `?state=closed`
- `pr list --state all` sends `?state=all`

#### Repo and PR actions

- `repo view` path is correct
- `repo create` payload is correct
- `issue create` payload includes labels/assignees when provided
- `pr create` payload includes `head`, `base`, and `body`
- `pr merge` sends the correct merge endpoint

Exit condition:

- request paths, query parameters, and payloads are verified without relying on a live server

## Priority 4: Git-process integration tests

These tests validate flows that depend on local git behavior.
They are more expensive than unit tests but still cheaper than live GitBucket E2E.

Recommended setup:

- create temporary git repositories in tests
- set up remotes locally
- use fixture repositories to simulate same-repo and fork scenarios where possible

### 4.1 Highest-value git integration scenarios

- repo auto-detection from `origin`
- `repo clone` behavior for full URLs and owner/repo inputs
- `pr checkout` writing branch content from `FETCH_HEAD`
- `pr diff` comparing `origin/<base>` against fetched head content

Note:

- full automation of fork PR behavior may still be easier at the E2E layer, depending on how much git plumbing you want to simulate in-process

Exit condition:

- git-command behavior is validated in temporary repositories, not only by hand

## Priority 5: Real GitBucket E2E tests

Only add this after the lower layers exist.
Otherwise failures will be too expensive to debug.

Recommended environment:

- Docker or docker compose
- one GitBucket container
- a seeded test user/token
- one base repository
- one fork or second user for PR scenarios

### 5.1 Highest-value E2E scenarios

- auth login against a live instance
- repo list/view/create
- issue create/list/view/close/reopen/comment
- PR create/list/view/comment/merge
- `pr checkout` for same-repo PR
- `pr checkout` for fork PR
- `pr diff` for same-repo PR
- `pr diff` for fork PR

### 5.2 What should remain manual even after E2E exists

- browser-launch behavior on each OS
- interactive prompt ergonomics
- terminal color/readability in multiple terminal environments

Exit condition:

- a small but representative smoke suite passes against a disposable real GitBucket environment

## Suggested Test Layout

### Unit tests

Keep near the implementation with `#[cfg(test)]`.

Good fits:

- `src/config/auth.rs`
- `src/cli/common.rs`
- `src/api/client.rs`
- `src/output/mod.rs`
- `src/output/table.rs`

### Integration tests

Create under `tests/`.

Suggested files:

- `tests/auth_cli.rs`
- `tests/config_resolution.rs`
- `tests/repo_resolution.rs`
- `tests/issue_cli.rs`
- `tests/pr_cli.rs`
- `tests/json_output.rs`

### E2E tests

Keep separate from normal fast tests.

Suggested structure:

- `tests/e2e/`
- gated by an env var such as `GB_E2E=1`
- optionally run in CI only on demand or nightly

## First Concrete Tasks

If you want the fastest path forward, do these first:

1. Add `clippy -D warnings` to your regular verification flow.
2. Add integration tests for invalid `--state` values.
3. Add integration tests for `GB_HOST` / `GB_REPO` / `GB_TOKEN` / `GB_PROTOCOL` precedence.
4. Add mocked API tests for `issue list --state` and `pr list --state` query parameters.
5. Add one temporary-repo test for git remote auto-detection.
6. Add one live GitBucket smoke test later, after the above are stable.

## Recommended Definition Of Done

For a feature in this repository, aim for this minimum bar:

- unit tests cover pure logic and edge cases
- integration tests cover CLI entry behavior and config precedence
- mocked API tests verify request shape when HTTP is involved
- manual real-environment verification is done at least once for git- and API-heavy features

## Practical Advice

Do not start with full E2E.
This repository has enough pure logic and CLI behavior that you can get most regression protection more cheaply.

The most effective early investment is:

- config/auth precedence tests
- list-state request tests
- repo auto-detection tests
- git checkout/diff behavior tests

Those areas carry the highest risk of regressions relative to their implementation complexity.
