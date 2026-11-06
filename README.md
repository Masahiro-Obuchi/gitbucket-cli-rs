# GitBucket CLI

[![Rust](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/actions/workflows/rust.yml)
[![E2E](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/actions/workflows/e2e.yml/badge.svg?branch=main)](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/actions/workflows/e2e.yml)

`gb` is a command-line interface for GitBucket.
It is inspired by [GitHub CLI (`gh`)](https://cli.github.com/) and implemented in Rust.
This is an unofficial community project and is not affiliated with the GitBucket project.

```bash
$ gb issue list
#    STATE   TITLE                          AUTHOR   LABELS
#1   OPEN    Fix login page bug             alice    bug
#3   OPEN    Add dark mode support          bob      enhancement

$ gb pr create -t "Add feature X" --head feature/x -B main
✓ Created pull request #5: Add feature X
Head: alice/my-app:feature/x
Base: alice/my-app:main
URL: https://gitbucket.example.com/alice/my-app/pull/5
```

## Installation

### Install with Cargo

```bash
cargo install gitbucket-cli --locked
```

To install a specific version tag:

```bash
cargo install --git https://github.com/Masahiro-Obuchi/gitbucket-cli-rs --tag v0.6.0 --locked
```

### Build from source

```bash
git clone https://github.com/Masahiro-Obuchi/gitbucket-cli-rs.git
cd gitbucket-cli-rs
cargo build --release
cp target/release/gb ~/.local/bin/
```

### Prebuilt binaries

Tagged releases publish prebuilt binaries to GitHub Releases for supported platforms.
See [RELEASE.md](RELEASE.md) for the release process.

**Requirements:** Rust 1.70+ and `git` when installing with Cargo or building from source

## Quick start

### 1) Authenticate

```bash
gb auth login
# GitBucket host or URL: gitbucket.example.com
# Personal access token: ********
# ✓ Logged in to gitbucket.example.com as alice
```

Create a token in **GitBucket → Account Settings → Personal access tokens**.

For subpath deployments, pass the base URL (or host plus path):

```bash
gb auth login -H https://gitbucket.example.com/gitbucket
# or
gb auth login -H gitbucket.example.com/gitbucket --protocol https
```

### 2) Repository operations

```bash
gb repo list
gb repo view alice/my-app
gb repo create my-new-repo
gb repo create my-team-repo --group dev-team
gb repo clone alice/my-app
gb repo fork alice/my-app
```

### 3) Label operations

```bash
gb label list
gb label create bug --color fc2929
gb label view bug
gb label edit bug --name defect
gb label edit defect --color cc0000 --web
gb label delete bug --yes
```

### 4) Milestone operations

```bash
gb milestone list --state all
gb milestone create v1.0 --description "First release" --due-on 2026-04-01
gb milestone view 1
gb milestone edit 1 --state closed
gb milestone delete 1 --yes
```

### 5) Issue operations

```bash
gb issue list --state all
gb issue create -t "Bug report"
gb issue view 1
gb issue view 1 --comments
gb issue view 1 --comments --no-pager
gb issue edit 1 --title "Updated bug report" --add-label urgent
gb issue close 1
gb issue comment 1 -b "Fixed!"
gb issue comment 1 --edit-last -b "Actually fixed!"
```

### 6) Pull request operations

```bash
gb pr list --state closed
gb pr list --state open --json --no-pager
gb pr create
gb pr create -t "From fork" --head feature/x --head-owner alice -B main
gb pr create --head feature/x -B main --detect-existing
gb pr view 5
gb pr view 5 --json --no-pager
gb pr edit 5 --title "Updated title"
gb pr edit 5 --add-assignee bob --web
gb pr close 5
gb pr reopen 5
gb pr merge 5
gb pr checkout 5
gb pr diff 5 --no-pager
gb pr comment 5 --edit-last -b "Updated note" --json
gb pr comment list 5 --json --no-pager
```

## Command reference

| Command                        | Description                          |
| ------------------------------ | ------------------------------------ |
| `gb auth login`                | Authenticate to a GitBucket instance |
| `gb auth logout`               | Remove auth for a host               |
| `gb auth status [--json]`      | Show current auth status             |
| `gb auth token`                | Print access token                   |
| `gb config`                    | Manage local CLI configuration       |
| `gb completion <SHELL>`        | Generate shell completion scripts    |
| `gb api <ENDPOINT>`            | Call the GitBucket REST API directly |
| `gb repo list [OWNER]`         | List repositories                    |
| `gb repo view [OWNER/REPO]`    | Show repository details              |
| `gb repo create [NAME]`        | Create a repository                  |
| `gb repo clone <REPO>`         | Clone a repository                   |
| `gb repo delete [OWNER/REPO]`  | Delete a repository                  |
| `gb repo fork [OWNER/REPO]`    | Fork a repository                    |
| `gb label list`                | List labels                          |
| `gb label view <NAME>`         | Show label details                   |
| `gb label create [NAME]`       | Create a label                       |
| `gb label edit <NAME>`         | Edit a label                         |
| `gb label delete <NAME>`       | Delete a label                       |
| `gb milestone list`            | List milestones                      |
| `gb milestone view <NUMBER>`   | Show milestone details               |
| `gb milestone create [TITLE]`  | Create a milestone                   |
| `gb milestone edit <NUMBER>`   | Edit a milestone                     |
| `gb milestone delete <NUMBER>` | Delete a milestone                   |
| `gb issue list`                | List issues                          |
| `gb issue view <NUMBER>`       | Show issue details                   |
| `gb issue create`              | Create an issue                      |
| `gb issue edit <NUMBER>`       | Edit an issue                        |
| `gb issue close <NUMBER>`      | Close an issue                       |
| `gb issue reopen <NUMBER>`     | Reopen an issue                      |
| `gb issue comment <NUMBER>`    | Add or edit an issue comment         |
| `gb pr list`                   | List pull requests                   |
| `gb pr view <NUMBER>`          | Show PR details                      |
| `gb pr create`                 | Create a pull request                |
| `gb pr edit <NUMBER>`          | Edit a pull request                  |
| `gb pr close <NUMBER>`         | Close a pull request                 |
| `gb pr reopen <NUMBER>`        | Reopen a closed, unmerged PR          |
| `gb pr merge <NUMBER>`         | Merge a pull request                 |
| `gb pr checkout <NUMBER>`      | Checkout a PR branch                 |
| `gb pr diff <NUMBER>`          | Show PR diff                         |
| `gb pr comment <NUMBER>`       | Add or edit a PR comment             |
| `gb pr comment list <NUMBER>`  | List PR comments                     |
| `gb browse`                    | Open repository in browser           |

## Global options

```text
-H, --hostname <HOST>    GitBucket host or base URL
-R, --repo <OWNER/REPO>  Target repository
    --profile <NAME>     Configuration profile
    --json-errors        Print failures as structured JSON on stderr
-h, --help               Show help
-V, --version            Show version
```

## Repository auto-resolution

Repository selection uses `-R/--repo`, then `GB_REPO`, then the active profile's
`default_repo`. If none is set, `gb` reads `git remote get-url origin` and checks
it against the selected GitBucket instance before making any API request.
HTTP(S) origin URLs must match its scheme, host, port, and deployment path.
SSH URLs must match its hostname (ignoring ASCII case) and deployment path; the
SSH port is independent of the HTTP port. Matching clone URLs may end in `/`.
A root-level SSH URL cannot identify a path-prefixed instance
unambiguously, so specify `-R` in that case. SSH aliases are not expanded.

On a mismatch, select the intended target explicitly:

```bash
gb -H https://gitbucket.example.com/gitbucket -R alice/my-app issue list
```

Explicit/environment/profile repositories do not need to match the local origin.
`OWNER/REPO` must contain exactly two nonempty names, without URL delimiters,
percent escapes, whitespace, or `.`/`..` components. Letters, digits, `_`, `-`,
`.`, `+`, and non-whitespace Unicode names are supported.

For `gb repo view`, `gb repo delete`, and `gb repo fork`, you can also pass
`OWNER/REPO` positionally after the subcommand. Deletion still requires an explicit
repository.

```bash
cd ~/projects/my-app
gb issue list
```

Supported remote URL formats include:

- `https://gitbucket.example.com/alice/my-app.git`
- `https://gitbucket.example.com/git/alice/my-app.git`
- `https://gitbucket.example.com/gitbucket/alice/my-app.git`
- `https://gitbucket.example.com/gitbucket/git/alice/my-app.git`
- `git@gitbucket.example.com:alice/my-app.git`
- `ssh://git@gitbucket.example.com:29418/alice/my-app.git`

## Output formats

```bash
gb issue list
gb issue list --json --no-pager
gb issue view 1 --web
```

## Skill Sample

This repository includes a sample Skill for AI agents that need to operate GitBucket with `gb`:

```text
skills/gitbucket-cli/
```

The skill keeps agent-facing workflow guidance separate from the full user documentation in this README and `SPEC.md`.

## State Filters

`gb issue list`, `gb pr list`, and `gb milestone list` support `--state open`, `--state closed`, and `--state all`.
Invalid values are rejected before the API call is made.

## Complete Issue, PR and Comment Lists

Issue and PR lists fetch all pages and sort by number descending. Ordinary
comment reads (view, list and edit-last) share complete retrieval, sorted by
comment ID ascending. `issue view --comments` currently includes comments only
in text output; JSON comment inclusion remains a follow-up. GitBucket 4.44.0 uses 25-row Issue/PR pages without Link
headers and returns all discussion comments in one response; these routes have
separate policies. Link-based servers are also supported with API-boundary and
cycle checks. An incomplete traversal fails without printing a partial list.

`GB_PAGINATION_MAX_PAGES` bounds each traversal (default 1000, range 1–100000).
Reaching the bound with another page pending is an error, not successful
truncation. `pr list --json` also verifies each PR with the detail API; table
output uses summary fields. This can increase request counts for large lists.
See [pagination and existing-PR detection](docs/PAGINATION.md) for contracts,
null-detail compatibility, and limitations under concurrent changes.

## GitBucket Web Fallbacks

Some GitBucket actions are only exposed through the web UI, not the REST API.
When `gb repo delete`, `gb repo fork`, `gb issue close`, `gb issue reopen`, or issue metadata updates hit that case, `gb` falls back to a short web sign-in flow and may prompt for your password.
`gb pr edit`, `gb pr close`, and `gb pr reopen` use the PR REST endpoint for
title/body/state, without Web credentials. Assignee changes use the Web UI only
when `--web` permits them; the flag does not force REST fields through Web.
REST failures, including `404`, are returned directly instead of triggering a
Web retry.
When a web fallback is used, `gb` writes a `Notice:` line to stderr so scripts can distinguish REST-only success from fallback success.
For `gb issue edit`, Web handles metadata when needed; labels always use their dedicated REST add/remove endpoints. Label-only changes need no Web password and do not require Issue PATCH support.
Use `GB_USER` and `GB_PASSWORD` to preseed those prompts when needed. Before a Web
write, `gb` verifies the cookie-authenticated user with `/api/v3/user` and compares
it with the REST token's user. If they differ, set `GB_WEB_USER` explicitly to
authorize the Web actor; the selected actors are displayed on stderr. Interactive
SSO/OIDC sign-in is unsupported by this password flow.

Web HTTP 200 is only a submitted request: stored values must confirm the change.
Failed or unverified mutations exit 1 and include operation outcomes on stderr;
`--json-errors` adds `error.operations` with `succeeded`, `failed`, `unverified`,
and `not_attempted` statuses, the route, and the verified Web actor when available.
Direct rejections are distinguished from failures after redirects, including
redirects back to the same URL. Encoded paths cannot bypass the deployment boundary.
Inspect the target before retrying an unverified write. Mixed edits are not atomic.
See [Web operations and verification](docs/WEB_OPERATIONS.md) for route constraints.

Repository deletion requires an initially readable target and a confirming 404
afterward. Forks verify the selected destination and immediate parent, including
4.44's Web header when REST omits fork metadata. Fork creation polls reads for up
to five seconds of delay; a timeout is unverified and does not resubmit the fork.
`--group` is also sent as `organization` when the REST fork route is available;
the default destination comes from the REST token, rather than `GB_USER`.

Issue label and assignee options can be repeated or comma-separated:

```bash
gb issue create -t "Bug report" --label bug,urgent --assignee alice
gb issue edit 1 --add-label needs-review --remove-assignee bob
gb pr edit 5 --add-assignee alice,bob --remove-assignee carol --web
```

`gb pr create --head` accepts a branch name for same-repository PRs. For cross-repository PRs, pass `OWNER:BRANCH` or use `--head-owner OWNER --head BRANCH`; after creation, the CLI prints the resolved `Head:` and `Base:` repositories and refs. Use `gb pr create --detect-existing` to return an existing open PR for the same head/base instead of creating a duplicate, including candidates on later pages. Candidates require canonical PR details because the GitBucket 4.44.0 PR collection can misidentify fork ownership; an incomplete search fails before creating a PR. `gb pr create --json` and `gb pr view --json` print the API payload for automation; `gb pr view --comments --json` also includes a top-level `comments` array. `gb pr list --json` fetches each listed PR detail for all state filters before printing JSON so fork head/base repository information matches `gb pr view --json` more closely.
`gb pr comment --json` prints the created or edited comment object; normal text output includes the comment ID and URL when GitBucket returns one. Use `gb pr comment list <NUMBER> --json` to inspect existing PR comments before retrying a comment mutation. `gb issue list`, `gb issue view`, `gb pr list`, `gb pr view`, and `gb pr comment list` accept `--no-pager` to force direct stdout output.

## Pull Request Edits and Target Validation

PR title/body/state edits use `PATCH /repos/OWNER/REPO/pulls/NUMBER`. GitBucket
4.44 updates the body only when a title is supplied and can clear it on a
title-only PATCH, so `gb` supplies the existing counterpart for content edits.
State-only edits send just state. Use `--body ''` to clear a body; a blank new
title is rejected. Repeated close/reopen requests already satisfied by the
current state do not send another update. Merged PRs cannot be reopened.

Numbered PR commands resolve the target through the PR API first, including
comment listing/posting/editing. Ordinary Issue numbers and mismatched API
numbers are refused before prompting or using Issue-backed subresources.
General PR conversation comments still use the Issue comments API after this
check; this does not add inline review-comment support.

Assignees are a separate operation. Actual changes require `--web`; no-op
assignee requests need no Web authentication. For mixed edits, Web permission
and sign-in are attempted before the REST mutation; REST fields are then
updated, and only assignee deltas use Web. Stored assignees are checked before
success. If a later step fails, the error identifies completed REST fields and
asks you to inspect the PR before retrying. A combined edit is not atomic;
concurrent writers can also race with the content snapshot. Shared Web sign-in
identity verification and general operation-result handling remain separate
follow-up work.

## Pull Request Checkout And Diff

`gb pr checkout` and `gb pr diff` work with same-repository PRs and fork-based PRs.
When a closed PR's diff cannot be reconstructed from fetched refs, `gb pr diff` tries the saved PR diff URL before failing. If no saved diff can be fetched, it exits non-zero and reports whether the live diff was unavailable because a ref was missing, a base/head fetch failed, or the fetched refs had no diff.
When the PR head repository is available from the API response, `gb` resolves that
repository's clone URL or a matching remote. Internal fetch refs under
`refs/gb/pr-targets/` use a fixed-length SHA-256 identifier of the configured
GitBucket base URL, repository, and PR number. This keeps internal ref paths short
for long deployment URLs on Windows. Remotes are matched by server and
repository path, not just `OWNER/REPO`. Fetches leave remote-tracking refs, tags,
and `FETCH_HEAD` alone.
Fetches do not recurse into submodules.

`gb pr checkout` requires a clean index and working tree, including untracked
files, with no merge/rebase/cherry-pick/revert/bisect in progress. Ignored files
that would be overwritten also cause checkout to stop. The first checkout creates
`pr-<NUMBER>` and records its target in local Git configuration. An existing
branch is reused only when that record matches this target and its commit equals
the fetched PR head. Local commits, an advanced or force-pushed PR head, another
PR's branch, and a branch checked out in another worktree cause an error.
There is no automatic reset, fast-forward, stash, rename, or deletion.
Submodule changes are checked even when Git configuration normally hides them.
Checkout leaves submodule working trees and HEADs untouched, including when
`submodule.recurse` is enabled. If the PR changes a submodule commit, inspect and
update that submodule separately after checkout.

Branches created by older versions lack this target record and are conservatively
refused, even if their commits match. To keep one, rename it yourself (for example,
`git branch -m pr-5 saved-pr-5`) before retrying. A shared OS lock serializes `gb`
checkouts across linked worktrees; the empty `gb-pr-checkout.lock` file in the Git
common directory remains and must not be deleted. Ordinary Git commands do not
participate in this lock. Repository hooks still run: a failed post-checkout hook
can occur after the switch; the error explicitly reports this partial completion
and asks you to inspect the working tree.

For automation, pass global `--json-errors` or set `GB_JSON_ERRORS=true` to get a single JSON error object on stderr, including fields such as `error.code`, `error.cause`, `error.status`, `error.exit_code`, and `error.operations` when mutation progress is available.

### Git authentication

`gb repo clone OWNER/REPO`, `gb pr checkout`, and `gb pr diff` reuse the selected
GitBucket user/token for HTTP(S) Git URLs on the same scheme, host, port, and
configured base path. With `GB_TOKEN` alone, also set `GB_USER` for private Git
access. SSH and other clone origins use Git's own authentication.

Credentials are supplied by a process-local Git credential helper, not placed in
Git command arguments, clone URLs, or Git configuration files. The helper checks
the exact URL Git requests, including after `url.*.insteadOf` rewriting, and does
not save credentials through existing helpers. Token-bearing commands allow only
HTTP(S) transports, even if a local URL rewrite enables another transport.
Token-bearing HTTP transfers do not follow redirects, including after URL rewriting;
configure the direct clone URL if a proxy redirects it.
The token is passed to the helper in the Git subprocess environment, so this does
not protect against a local process with permission to inspect that environment.

Full-URL `gb repo clone` uses Git's own credentials. HTTP URLs containing passwords
are rejected; use a credential helper or `OWNER/REPO` with `gb auth login` instead.
Existing HTTP remote userinfo is removed from the command URL used by PR fetches.

## Network and Git deadlines

REST and Web requests use a 10-second connection deadline, a 30-second read
inactivity deadline, and a 120-second total request deadline including its body.
Each Git subprocess has a 300-second deadline, including output collection.
Configure them with `GB_HTTP_CONNECT_TIMEOUT`, `GB_HTTP_READ_TIMEOUT`,
`GB_HTTP_TIMEOUT`, and `GB_GIT_TIMEOUT` (seconds, fractional values accepted;
minimum 0.001, maximum 86400). Zero does not disable deadlines.

```bash
GB_HTTP_TIMEOUT=180 GB_HTTP_READ_TIMEOUT=45 gb pr view 7
GB_GIT_TIMEOUT=600 gb repo clone alice/large-repository
```

HTTP failures retain the method/path, failure phase, cause and known response
status. Query strings and credentials are excluded from request diagnostics;
error bodies are bounded and sensitive details are omitted/redacted. With
`--json-errors`, a write whose response cannot be confirmed adds
`error.outcome: "unverified"`. `error.cause` distinguishes `timeout`,
`connect_failed`, `body_read_failed`, and `invalid_json`. Inspect the target
before retrying: comments, merges, forks and other writes are never automatically
resent, including after timeouts or an incomplete response body.

Git timeout/Ctrl+C terminates its process group (Unix) or job (Windows), waits
for its child, and closes inherited pipes. Git stdin is closed and Git terminal
credential prompts are disabled (`GIT_TERMINAL_PROMPT=0`). SSH or external helpers
can have their own prompts, within the process deadline. No automatic stash/reset/clean is performed;
clone directories, refs or worktrees may already have changed. Uncatchable process
termination and a deliberately detached Unix helper are outside this cleanup.
Interactive prompts and `gb api --input -` also respond to cancellation and exit
with code 1 (`cancelled` with `--json-errors`). Cancelling input does not submit
the pending operation. If PR checkout is interrupted and its state cannot be
read back, the original mutation error and checkout provenance are retained.

## Environment variables

| Variable        | Description                                               |
| --------------- | --------------------------------------------------------- |
| `GB_TOKEN`      | Access token (takes precedence over file config for auth) |
| `GB_HOST`       | Default GitBucket host or base URL                        |
| `GB_REPO`       | Default repository (`OWNER/REPO`)                         |
| `GB_PROFILE`    | Default configuration profile                             |
| `GB_HTTP_CONNECT_TIMEOUT` | Connection deadline in seconds (default 10) |
| `GB_HTTP_READ_TIMEOUT` | Read inactivity deadline in seconds (default 30) |
| `GB_PAGINATION_MAX_PAGES` | Maximum collection pages before failing (default 1000, range 1–100000) |
| `GB_HTTP_TIMEOUT` | Total HTTP request deadline in seconds (default 120) |
| `GB_GIT_TIMEOUT` | Deadline per Git subprocess in seconds (default 300) |
| `GB_CONFIG_DIR` | Config directory path (default: `~/.config/gb/`)          |
| `GB_PROTOCOL`   | Protocol override when using `GB_TOKEN` with a plain host |
| `GB_USER`       | Username for GitBucket web sessions and token-authenticated Git transfers |
| `GB_WEB_USER`   | Explicit Web actor override; permits a different actor from the REST token |
| `GB_PASSWORD`   | Password for GitBucket web-session fallbacks              |
| `GB_JSON_ERRORS` | Print failures as structured JSON on stderr              |
| `NO_COLOR`      | Disable colored output                                    |

## Configuration file

Credentials are stored in `~/.config/gb/config.toml` (or under `GB_CONFIG_DIR`).
Updates use a temporary file in the same directory, sync it, then atomically
replace `config.toml`. Concurrent `login`, `logout`, and `config` changes reload
the latest settings under an OS file lock, waiting up to five seconds. Login
performs authentication before acquiring this lock. Readers see a complete old
or new file; unchanged settings retain their original formatting and comments.

The empty `config.toml.lock` file remains after commands finish. Do not delete it:
the OS releases its lock when the process exits, including after a crash.
Configuration and lock files must be regular files; symlinks and Windows reparse
points are rejected. Set `GB_CONFIG_DIR` to the directory containing the real
configuration instead.

On Unix-like systems, configuration and lock files use `0600` permissions, and
new configuration directories use `0700`. Existing directory permissions are
preserved. On Windows, files inherit the directory's access permissions; use a
private configuration directory. File-access conflicts are retried for up to
one second per operation while retaining the old configuration and synced
temporary file until replacement succeeds. Atomic replacement requires filesystem support;
locks coordinate participating `gb` commands, not manual editors or older
versions. Unix also syncs the parent directory. If this final sync fails, the
command reports that replacement already occurred: reload before retrying.
These steps do not guarantee durability on every filesystem or during power loss.

```toml
default_host = "https://gitbucket.example.com/gitbucket"

[hosts."https://gitbucket.example.com/gitbucket"]
token = "your-personal-access-token"
user = "alice"
protocol = "https"
```

Plain host keys also work:

```toml
[hosts."gitbucket.example.com"]
token = "your-personal-access-token"
user = "alice"
protocol = "https"
```

Profiles can select host and repository defaults, and can optionally carry credentials scoped to that profile:

```toml
default_profile = "work"

[profiles.work]
default_host = "gitbucket.example.com"
default_repo = "alice/project"

[profiles.work.hosts."gitbucket.example.com"]
token = "your-work-token"
user = "alice"
protocol = "https"
```

`--hostname` / `GB_HOST` and `--repo` / `GB_REPO` override profile defaults. Profile-scoped credentials are tried before global `[hosts]` credentials for the same host.

The same GitBucket host can be used with multiple accounts by storing credentials under different profiles:

```sh
gb auth login --profile work -H gitbucket.example.com -t "$WORK_TOKEN"
gb auth login --profile personal -H gitbucket.example.com -t "$PERSONAL_TOKEN"

gb auth status --profile work
gb auth status --profile personal --json
gb issue list --profile work -R alice/project
```

`gb auth status --profile <name>` prints only that profile and shows its effective actor, including whether credentials came from the profile, global host entry, or environment token. `gb auth status --json` exposes the same information in `effective_actor` for scripts.

When `GB_TOKEN` is set, protocol selection uses this order:

1. URL scheme from `--hostname` or `GB_HOST` when present
2. `GB_PROTOCOL`
3. Matching stored host configuration
4. Default `https`

## Roadmap

Current priority is to harden CI and Docker-backed verification around the existing command set before adding larger new areas.

Near term:

- Keep strengthening test and Docker-backed E2E coverage
- Keep `cargo clippy --all-targets --all-features -- -D warnings` green in CI

After that:

- Richer issue/PR metadata handling

Lower priority / re-evaluate later:

- Webhook and collaborator operations
- Admin-oriented user management flows

## Configuration commands

```bash
gb config path
gb config list
gb config get default-host
gb config get default-profile
gb config get host --host gitbucket.example.com/gitbucket --field protocol
gb config get profile work --json
gb config set host --host gitbucket.example.com/gitbucket --protocol http --default
gb config set profile work --default-host gitbucket.example.com --default-repo alice/project
gb config set default-profile work
gb config unset default-host
gb config unset default-profile
```

`gb config` manages the local `config.toml` file. It currently supports:

- printing the config file path
- listing saved hosts, profiles, and stored defaults
- reading saved host `user` / `protocol` / `has-token` values
- updating saved host `user` / `protocol` values
- creating or updating profile defaults
- setting or clearing `default_host` and `default_profile`

Token creation and token printing remain under `gb auth`.

## API command

```bash
gb api user
gb api /api/v3/user
gb api repos/alice/my-app/issues --input body.json
echo '{"state":"closed"}' | gb api repos/alice/my-app/issues/1 -X PATCH --input -
```

`gb api` calls the GitBucket REST API directly using the configured host and token.
It accepts endpoint paths relative to `/api/v3`, full API paths such as `/api/v3/user`, or absolute URLs under the configured GitBucket API base.
All authenticated API requests are checked after URL normalization, including
relative and percent-encoded paths. Encoded deployment names and equivalent
encodings in pagination links are supported. Paths that escape the API base, URL
credentials, and fragments are rejected before the request. Authenticated API
requests do not follow redirects; configure the direct GitBucket base URL.
Base URLs must use HTTP(S), without credentials, query strings, or fragments.
When `--input` is given and `-X/--method` is omitted, `gb api` defaults to `POST`.
JSON responses are pretty-printed, and zero-byte success responses (including
204) print `null`. Top-level strings and objects containing `status`/`body` are
preserved as returned. `gb api` does not apply PR-command wrapper compatibility
or treat non-JSON text as JSON. Internal diff/HTML reads use a separate text
contract; typed JSON endpoints reject missing or invalid JSON instead of silently
switching to another endpoint. PR detail fallback is limited to a JSON `null`.

## Completion command

```bash
gb completion bash > ~/.local/share/bash-completion/completions/gb
gb completion zsh > ~/.zfunc/_gb
gb completion fish > ~/.config/fish/completions/gb.fish
gb completion powershell > gb.ps1
```

`gb completion` prints shell completion scripts to stdout.
It currently supports `bash`, `zsh`, `fish`, and `powershell`.
The command does not install the scripts for you; redirect the output to the appropriate location for your shell.

## Label commands

```bash
gb label list
gb label list --json
gb label view bug
gb label create needs-review --color abcdef
gb label edit needs-review --name reviewed
gb label edit reviewed --color 123abc --web
gb label delete reviewed --yes
```

Label creation and rename use REST and verify stored values. Rename supplies the
current color when `--color` is omitted. Colors accept six hex digits with an
optional `#`. A no-op edit sends no mutation.

GitBucket 4.44 rejects a same-name REST color update with 422. Pass `--web` to
permit the verified in-place Web route when that exact limitation is encountered;
it checks the numeric ID, name and current color before submitting the form and
verifies the same ID and saved values afterward. A server accepting the REST
request needs no Web credentials, even with `--web`. Conflicting renames, 404s and
other REST failures do not trigger this route. Labels are never deleted/recreated
or temporarily renamed to change their color. Unknown HTML layouts or changed
forms fail before submission.

Label descriptions are not persisted by the verified 4.44 REST/Web contracts.
`--description` and `--remove-description` remain recognized but return an explicit
unsupported error before mutation, including mixed rename/description requests.
This CLI does not infer write support on another instance from HTTP success or a
read-only field. Descriptions returned on reads are still displayed. Server fixes
and capability requirements are recorded in [the upstream follow-up](docs/UPSTREAM_LABEL_API.md).

Issue label edits use individual removals and additive requests, preserving labels
outside the requested delta. Issue create/edit label names are matched exactly
after comma splitting, including surrounding spaces and Unicode whitespace.
Use `bug,urgent` without padding unless the padding belongs to the label name.
Additions must already exist in the repository:
`gb label create` them first. This preflight avoids the pinned server's implicit
creation of misspelled labels. Repeated options are deduplicated; if a label is
both removed and added, addition wins. Label-only edits need only API credentials.
For mixed metadata/label edits, metadata is applied and verified first, then labels
are changed and read back. Failures report confirmed steps and inspection-before-
retry guidance. These operations are not atomic; unrelated writers can still race
with snapshots. Unsafe dot-segment/path names are rejected before requests.

## Milestone commands

```bash
gb milestone list
gb milestone list --state all --json
gb milestone view 7
gb milestone create v1.0 --description "First release" --due-on 2026-04-01
gb milestone edit 7 --title v1.0.1 --state closed
gb milestone edit 7 --description '' --due-on ''
gb milestone delete 7 --yes
```

`gb milestone` manages repository milestones.
It currently supports listing, viewing, creating, editing, and deleting milestones in the target repository.
Edits preserve unspecified title, state, description, and the exact existing due
timestamp. Use `--description ''` or `--due-on ''` to clear a field. Explicit
`--due-on` values accept `YYYY-MM-DD` or RFC3339 and use the calendar-date portion
at midnight UTC for REST; leaving the option out does not renormalize stored times.

REST is tried first for create/edit/delete. GitBucket 4.44.0 requires a complete
replacement for milestone PATCH and restricts REST titles to at most 100 ASCII
letters/digits or `-+_.`. `gb` merges the current values before PATCH, and uses
JSON `null` to clear description/due date. An ordinary `404`, missing milestone,
or permission failure does not trigger Web sign-in. Delete uses REST only;
the pinned server treats deleting an already absent milestone as success.

Japanese and space-containing titles are retained exactly. If REST rejects such
a title with its known validation response, `gb` confirms the target still exists
(or checks the repository's milestone list for creation), then uses the Web route;
this can require `GB_USER` / `GB_PASSWORD`. A server that accepts the title over
REST needs no Web credentials. Web create/edit results are checked before success
is printed; if verification fails, inspect the milestone before retrying.

The Web form trims leading/trailing characters through U+0020 (ASCII controls and
space), and accepts only calendar dates. Other Unicode spacing in nonblank titles
and descriptions is retained. Content edits are refused before Web authentication
if they would trim text or resend an existing due timestamp without an explicit
`--due-on` setting/clear. Set that option
explicitly or change to a REST-compatible title. State-only Web actions do not
resubmit description or due date. A Web date is interpreted by the server's form;
if its timezone produces a different instant, verification reports the mismatch
instead of success. Concurrent edits can still race between the initial read and
replacement; this is not a server-side conditional update.

## Testing

For the current automated test layout, release checks, and Docker-backed E2E workflow, see [TESTING.md](./TESTING.md).

## Acknowledgements

This project exists thanks to [GitBucket](https://gitbucket.github.io/) and the community around it. Thank you for building and maintaining the software that made this CLI worth creating.

## License

MIT
