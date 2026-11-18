# Repository Guidelines

## Current Implementation Status

- Implemented top-level commands: `auth`, `repo`, `issue`, `pr`, `browse`, `config`, `api`, `label`, `milestone`, and `completion`.
- Implemented auth flows: `login`, `logout`, `status`, and `token`.
- Repository flows support list/view/create/clone/delete/fork.
- Issue flows support list/view/create/edit/close/reopen/comment.
- Pull request flows support list/view/create/edit/close/reopen/merge/checkout/diff/comment.
- Formal review approval/request-changes/dismissal, atomic expected-head merges,
  and complete stored review history are not implemented by the verified adapter.
  See `docs/reference/merge.md` for GitBucket 4.44.0 core support and limits; other
  versions, plugins and SSO integrations require separate verification.

## Project Structure & Module Organization

- `src/main.rs`: CLI entrypoint and command dispatch.
- `src/cli/`: command implementations (`auth`, `api`, `completion`, `config`, `label`, `milestone`, `repo`, `issue`, `pr`) and shared resolution logic in `common/`.
- `src/api/`: GitBucket API client and endpoint wrappers.
- `src/models/`: API request/response structs.
- `src/config/`: local auth/config file handling.
- `src/output/`: table and display helpers.
- `src/application/`, `src/domain/`, `src/git/`: operation verification, domain identities, and Git adapters.
- `README.md`: project overview, prerequisites, installation and Quick Start.
- `docs/README.md`: documentation index for usage, reference and maintenance.

Keep CLI help and the relevant pages under `docs/` aligned with command changes. Keep the README brief. Use one line per prose paragraph and use Markdown line breaks for structure or intentional display. Describe features by name in public documentation.

## Build, Test, and Development Commands

- `cargo build`: compile the project in debug mode.
- `cargo run -- --help`: run locally and print CLI help.
- `cargo test`: run unit and integration tests.
- `cargo check`: fast compile checks during development.
- `cargo fmt --all`: format Rust code.
- `cargo clippy --all-targets --all-features -- -D warnings`: lint and fail on warnings.
- `cargo package --locked --offline`: build and verify the source archive after dependencies are cached.
- `cargo +1.88.0 check --locked --all-targets --all-features` and
  `cargo +1.88.0 test --locked --all-features`: verify the declared MSRV.
- `python3 -B -m unittest discover -s scripts/tests -v`: test binary-archive
  selection, link rewriting and extracted navigation without publishing.

The MSRV is Rust 1.88.0 with the committed lockfile. Keep `package.rust-version`,
the MSRV CI toolchain and `docs/maintainers/testing.md` aligned. Dependency updates must
pass MSRV CI; raising the minimum requires an explicit compatibility change.

Source-package paths are explicitly permitted by `package.include` in `Cargo.toml`.
When adding public file types or directories, update that policy deliberately and
run `cargo test --test package_contents`; keep local material outside the public
source/docs/fixture trees. See `docs/maintainers/packaging.md`.

Binary archives use `scripts/package-release.py` and the reusable
`.github/workflows/archive.yml` from both PR CI and Release. Keep bundled
documentation/link checks aligned; see `docs/maintainers/packaging.md`.

## Coding Style & Naming Conventions

- Follow Rust defaults: 4-space indentation, `rustfmt` formatting, no trailing whitespace.
- Use `snake_case` for functions/modules/files, `PascalCase` for structs/enums/traits, `SCREAMING_SNAKE_CASE` for constants.
- Prefer explicit error handling via `Result<T, GbError>` and `?`.
- Keep command UX consistent with GitHub CLI naming (`list`, `view`, `create`, `--json`, `--web`).
- Prefer storing GitBucket targets as host-or-base-URL strings; path-prefixed deployments such as `https://gitbucket.example.com/gitbucket` are supported.
- Preserve the current `reqwest` TLS setup using `rustls-tls-native-roots` so self-hosted instances can use system-trusted certificates.

## Current Functional Notes

- `--hostname/-H` and `GB_HOST` accept either a bare host or a full base URL.
- Auth config is stored in `~/.config/gb/config.toml` (or `GB_CONFIG_DIR`) under `[hosts."<normalized-http(s)-base-url>"]`.
- Repository auto-resolution supports HTTPS, SSH, and GitBucket `/git/` clone URLs.
- `issue list` and `pr list` support `--state open|closed|all` and pass that filter through to the API.
- Issue/PR bodies support mutually exclusive `--body`, `--body-file` and explicit
  `--editor/-e`. Editor input requires terminal stdin/stderr, is disabled by
  `--json-errors`, and preloads existing bodies for edits. See `docs/usage/input-and-display.md`.

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
