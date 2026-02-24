# Repository Guidelines

## Project Structure & Module Organization

- `src/main.rs`: CLI entrypoint and command dispatch.
- `src/cli/`: command implementations (`auth`, `repo`, `issue`, `pr`) and shared resolution logic in `common.rs`.
- `src/api/`: GitBucket API client and endpoint wrappers.
- `src/models/`: API request/response structs.
- `src/config/`: local auth/config file handling.
- `src/output/`: table and display helpers.
- `README.md`: user-facing usage guide.
- `SPEC.md`: detailed functional specification.

## Build, Test, and Development Commands

- `cargo build`: compile the project in debug mode.
- `cargo run -- --help`: run locally and print CLI help.
- `cargo test`: run unit and integration tests.
- `cargo check`: fast compile checks during development.
- `cargo fmt --all`: format Rust code.
- `cargo clippy --all-targets --all-features -- -D warnings`: lint and fail on warnings.

## Coding Style & Naming Conventions

- Follow Rust defaults: 4-space indentation, `rustfmt` formatting, no trailing whitespace.
- Use `snake_case` for functions/modules/files, `PascalCase` for structs/enums/traits, `SCREAMING_SNAKE_CASE` for constants.
- Prefer explicit error handling via `Result<T, GbError>` and `?`.
- Keep command UX consistent with GitHub CLI naming (`list`, `view`, `create`, `--json`, `--web`).

## Testing Guidelines

- Put unit tests near implementation in `#[cfg(test)] mod tests`.
- Put integration behavior in `tests/*.rs` when adding cross-module features.
- Cover success and failure paths for URL parsing, repo resolution, auth, and API error mapping.
- Run at least `cargo test` and `cargo check` before opening a PR.

## Commit & Pull Request Guidelines

- Commit messages: imperative, concise (`Add login subcommand`, `Handle 401 responses`).
- Keep commits scoped to one change.
- PRs should include: purpose, behavior changes, test evidence (`cargo test` output), and linked issue(s) if applicable.
- Include CLI output examples when changing command UX or error messages.
