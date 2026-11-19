# Building and testing

## Build from source

Install Git and Rust 1.88.0 or later. Use the committed lockfile for builds and dependency-bearing checks.

```sh
git clone https://github.com/Masahiro-Obuchi/gitbucket-cli-rs.git
cd gitbucket-cli-rs
cargo build --release --locked
```

The executable is `target/release/gb` or `target/release/gb.exe` on Windows. Put it on your `PATH` or run it directly. For development, `cargo run --locked -- --help` builds and runs the debug binary.

## Local checks

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Unit tests live near the implementation under `src/`. Integration tests in `tests/` invoke the real CLI against temporary repositories, configuration directories, and scripted HTTP servers. Use mock HTTP tests for CLI request, result, and failure behavior. Test differences that depend on real GitBucket persistence, routing, or authentication against the Docker fixture.

| Area | Test examples |
| --- | --- |
| Credentials and routing | [Profiles](../../tests/auth_profiles.rs), [Git credentials](../../tests/git_credentials.rs), [target validation](../../tests/target_validation.rs) |
| Configuration and process safety | [Configuration storage](../../tests/config_storage.rs), [cancellation](../../tests/cancellation.rs), [Git timeouts](../../tests/git_timeouts.rs) |
| Output and input | [JSON](../../tests/json_contract.rs), [CLI errors](../../tests/cli_errors_output.rs), [terminal editor](../../tests/editor_input.rs) |
| API and Web operations | [Transport](../../tests/transport_failures.rs), [pagination](../../tests/pagination_flows.rs), [Web outcomes](../../tests/web_outcomes.rs) |
| Pull requests | [Checkout](../../tests/pr_checkout.rs), [snapshots](../../tests/pr_snapshots.rs), [review reads](../../tests/pr_review_comments.rs), [review writes](../../tests/pr_review_write.rs), [merge](../../tests/pr_merge.rs) |

For focused tests, use `cargo test --locked --test pr_checkout` or an applicable target from the table. [Packaging](packaging.md) lists source and binary distribution checks.

## Live GitBucket tests

The ignored [E2E tests](../../tests/e2e_smoke.rs) require Git, Docker with Compose, Bash, curl, and Python 3. The scripts start GitBucket 4.44.0 behind a local proxy and provision disposable accounts, tokens, and repositories. Run the tests against the fixture. The generated `runtime.env` contains disposable credentials and belongs with the local fixture.

From the repository root, run the site root and `/gitbucket` variants separately. Each command block uses its own environment, port, and Compose project. The exit trap tears down the fixture and removes its data.

```bash
(
  export GB_E2E_ROOT="$PWD/.tmp/e2e-root" GB_E2E_COMPOSE_PROJECT=gb-e2e-root GB_E2E_HTTP_PORT=18080 GB_E2E_PATH_PREFIX=
  trap './scripts/e2e/down.sh' EXIT
  ./scripts/e2e/bootstrap.sh || exit
  set -a
  source "$GB_E2E_ROOT/runtime.env" || exit
  set +a
  cargo test --locked --test e2e_smoke -- --ignored --nocapture
)
```

```bash
(
  export GB_E2E_ROOT="$PWD/.tmp/e2e-prefix" GB_E2E_COMPOSE_PROJECT=gb-e2e-prefix GB_E2E_HTTP_PORT=18081 GB_E2E_PATH_PREFIX=/gitbucket
  trap './scripts/e2e/down.sh' EXIT
  ./scripts/e2e/bootstrap.sh || exit
  set -a
  source "$GB_E2E_ROOT/runtime.env" || exit
  set +a
  cargo test --locked --test e2e_smoke -- --ignored --nocapture
)
```

Use an additional filter, such as `cargo test --locked --test e2e_smoke api_contract -- --ignored --nocapture`, to focus on the direct server contracts and corresponding CLI behavior. Write contract tests that verify persisted values with a readback. Keep captured HTML focused and sanitized. See [GitBucket compatibility](../reference/gitbucket-compatibility.md) and [fixture documentation](../../tests/fixtures/README.md).

GitHub Actions runs both deployment layouts on stable Rust. The fixture covers the pinned core server; validate additional versions, plugins, authentication, and transport combinations on the deployment that will use them.

## Minimum supported Rust version

The manifest declares Rust **1.88.0** for the committed lockfile. The locked `cookie_store` and `time` dependencies require that version. Stable Rust is also tested and is used for release binaries.

```sh
cargo +1.88.0 check --locked --all-targets --all-features
cargo +1.88.0 test --locked --all-features
cargo +1.88.0 package --locked --offline
```

The MSRV matrix compiles all targets/features on native Linux x86-64, macOS x86-64 and arm64, and Windows x86-64 runners. On Linux it runs the ordinary suite and verifies the source package. The macOS and Windows MSRV jobs also run package-selection tests. Docker E2E runs in its own stable-Rust workflow.

Verify dependency updates on stable and the declared minimum. To raise the minimum, update `package.rust-version`, the workflow toolchain, the README prerequisites, and this section together. Check the release targets and document the version change in the release notes.
