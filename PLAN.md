# Implementation Plan

This document tracks the near-term implementation plan for `gitbucket-cli-rs`.
It is intentionally focused on upcoming work, not completed feature history.

## Current Focus

The core command surface is implemented. The next work should prioritize reliability:

- Keep Docker-backed E2E coverage representative of real GitBucket deployments.
- Validate path-prefixed deployments such as `/gitbucket`.
- Add focused live checks for high-risk write and git flows.
- Avoid broad feature expansion unless a real workflow gap is found.

## In Progress

### Path-Prefixed Docker E2E

Branch: `feature/e2e-path-prefix`

Goal:

- Run the Docker-backed E2E suite against both root-path and `/gitbucket` path-prefixed deployments.
- Ensure web fallback flows keep working through a path prefix.
- Ensure git clone flows work when GitBucket returns container-internal clone URLs.

Planned/implemented changes in that branch:

- Add an nginx proxy to the E2E Docker fixture.
- Add `GB_E2E_PATH_PREFIX` support to `scripts/e2e/*`.
- Matrix the E2E GitHub Actions workflow over root and `/gitbucket`.
- Normalize internal Docker clone URLs to the public E2E base URL without rewriting intentionally external clone origins.

Verification target:

- `cargo test -- --nocapture`
- `cargo clippy --all-targets --all-features -- -D warnings`
- root-path Docker E2E
- `/gitbucket` Docker E2E

## Next Phases

### Phase 1: Issue Edit Constraint E2E

Goal:

- Lock down the live GitBucket behavior of `gb issue edit`.
- Document and test which fields are supported by REST vs web fallback.

Candidate tests:

- `issue edit --title --body --state` succeeds against live GitBucket.
- `issue edit --milestone` and `--remove-milestone` succeed against live GitBucket.
- label/assignee edit attempts fail clearly when REST support is unavailable and only web fallback remains.

### Phase 2: Raw API Live Write E2E

Goal:

- Validate `gb api` as a reliable escape hatch for unsupported endpoints.

Candidate tests:

- `gb api <endpoint> -X <METHOD> --input file.json` sends a live write request successfully.
- `gb api` handles empty success bodies.
- `gb api` preserves valid top-level JSON values.

### Phase 3: Representative Failure-Path E2E

Goal:

- Improve confidence in user-facing errors for common real-world failures.

Candidate tests:

- PR merge failure produces a non-zero exit code and useful message.
- `pr checkout` / `pr diff` reports fetch source and branch when fetch fails.
- web fallback failures for delete/fork/edit report missing credentials clearly.

## Backlog

These are intentionally lower priority than reliability work:

- `repo collaborator`
- `webhook`
- admin/user-oriented commands
- broader shell/install automation beyond `gb completion`

## Working Rules

- Use a new `git worktree` and feature branch for every implementation step.
- Keep commits scoped to one change.
- Keep `README.md`, `SPEC.md`, `TESTING.md`, and CLI help aligned when behavior changes.
- Prefer mocked/integration tests for request shape and validation.
- Prefer Docker-backed E2E only for GitBucket-specific behavior, web fallback, and git integration.
