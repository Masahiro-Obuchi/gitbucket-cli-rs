# GitBucket CLI (`gb`) Specification

## 1. Overview

`gb` is a command-line tool for operating GitBucket from the terminal.
It follows the command design style of GitHub CLI (`gh`) and uses the GitBucket REST API (`/api/v3`).

- **Command name:** `gb`
- **Language:** Rust (edition 2021)
- **Target platforms:** Linux / macOS / Windows
- **License:** MIT

---

## 2. Design principles

### 2.1 Compatibility with GitHub CLI concepts

| GitHub CLI | GitBucket CLI | Status | Notes |
| --- | --- | --- | --- |
| `gh auth` | `gb auth` | ✅ Implemented | PAT-based authentication |
| `gh repo` | `gb repo` | ✅ Implemented | Core repo operations |
| `gh issue` | `gb issue` | ✅ Implemented | Basic issue workflows |
| `gh pr` | `gb pr` | ✅ Implemented | Basic PR workflows |
| `gh browse` | `gb browse` | ✅ Implemented | Opens browser |
| `gh label` | `gb label` | ✅ Implemented | Label definition CRUD (no edit yet) |
| `gh api` | `gb api` | ✅ Implemented | Raw REST API access |
| `gh config` | `gb config` | ✅ Implemented | Local config inspection and updates |
| `gh completion` | `gb completion` | ✅ Implemented | Shell completion generation |
| `gh gist` / `gh project` / `gh codespace` | — | ❌ Out of scope | No corresponding GitBucket feature/API |
| `gh run` / `gh workflow` / `gh cache` | — | ❌ Out of scope | No Actions-like CI feature in GitBucket core |

### 2.2 GitBucket-specific opportunities (planned)

| Command | Description | Status |
| --- | --- | --- |
| `gb milestone` | Milestone management | ✅ Implemented | REST read + GitBucket-compatible write fallback |
| `gb user` | User/admin workflows | 💤 Backlog |
| `gb webhook` | Webhook management | 💤 Backlog |
| `gb repo collaborator` | Collaborator management | 💤 Backlog |

---

## 3. Command reference

### 3.1 Global options

Available on all commands:

| Option | Short | Env var | Description |
| --- | --- | --- | --- |
| `--hostname <HOST>` | `-H` | `GB_HOST` | GitBucket host or base URL (e.g. `gitbucket.example.com` or `https://gitbucket.example.com/gitbucket`) |
| `--repo <OWNER/REPO>` | `-R` | `GB_REPO` | Target repository |
| `--help` | `-h` | — | Show help |
| `--version` | `-V` | — | Show version |

---

### 3.2 `gb auth` — authentication

#### `gb auth login`

Authenticate to a GitBucket instance.

```text
gb auth login [OPTIONS]
```

| Option | Short | Default | Description |
| --- | --- | --- | --- |
| `--hostname <HOST>` | `-H` | interactive prompt | Hostname or base URL |
| `--token <TOKEN>` | `-t` | interactive prompt | Personal access token |
| `--protocol <PROTOCOL>` | — | `https` | `https` or `http` |

Behavior:

1. Prompt for hostname/token when omitted.
2. Validate credentials by calling `GET /user`.
3. Save the host profile to `config.toml` on success and mark it as the default host.

Examples:

```bash
gb auth login
gb auth login -H gitbucket.example.com -t <TOKEN>
gb auth login -H localhost:8080 --protocol http
gb auth login -H https://gitbucket.example.com/gitbucket -t <TOKEN>
```

#### `gb auth logout`

Remove saved credentials for a host.

```text
gb auth logout [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--hostname <HOST>` | `-H` | Host to remove (defaults to current default host) |

#### `gb auth status`

Show current authentication entries.

```text
gb auth status
```

Example output:

```text
gitbucket.example.com
  ✓ Logged in as alice
  Protocol: https
```

#### `gb auth token`

Print token for script integration.

```text
gb auth token [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--hostname <HOST>` | `-H` | Host to print token for |

---

### 3.3 `gb config` — local configuration

`gb config` manages the local CLI configuration file and saved host metadata.
It operates on stored values only and does not read `GB_HOST`, `GB_TOKEN`, or other runtime overrides.

#### `gb config path`

```text
gb config path
```

Print the resolved path to `config.toml`.

#### `gb config list`

```text
gb config list [--json]
```

Behavior:

- Prints the config file path
- Prints the stored `default_host` when set
- Lists saved host entries with `user`, `protocol`, and whether a token is configured
- Redacts token values; use `gb auth token` when the raw token is needed

#### `gb config get default-host`

```text
gb config get default-host
```

Print the stored `default_host` value.

#### `gb config get host`

```text
gb config get host --host <HOST> [--field <FIELD>] [--json]
```

| Option | Description |
| --- | --- |
| `--host <HOST>` | Host or base URL to inspect |
| `--field <FIELD>` | `user`, `protocol`, or `has-token` |
| `--json` | Output the full host summary as JSON |

Host lookup is canonical, so equivalent forms such as `https://host/path`, `host/path`, and `https://host/path/api/v3` resolve to the same saved entry.

#### `gb config set default-host`

```text
gb config set default-host <HOST>
```

Set the stored default host. The target host must already exist in saved config.

#### `gb config set host`

```text
gb config set host --host <HOST> [--user <USER>] [--protocol <PROTOCOL>] [--default]
```

| Option | Description |
| --- | --- |
| `--host <HOST>` | Host or base URL to update |
| `--user <USER>` | Username stored for GitBucket web-session fallbacks |
| `--protocol <PROTOCOL>` | Stored protocol (`http` or `https`) |
| `--default` | Also make this host the stored default host |

The target host must already exist in saved config.

#### `gb config unset default-host`

```text
gb config unset default-host
```

Clear the stored `default_host` value.

---

### 3.4 `gb api` — raw REST API access

`gb api` calls the GitBucket REST API directly using the configured host and token.
It is intended as a low-level escape hatch for endpoints that do not yet have a dedicated top-level command.

```text
gb api <ENDPOINT> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--method <METHOD>` | `-X` | HTTP method to use (`GET` by default, or `POST` when `--input` is present) |
| `--input <PATH>` | `-i` | JSON request body file path, or `-` to read JSON from stdin |

Behavior:

- Relative endpoints are resolved under `/api/v3`, so `user` becomes `/api/v3/user`
- Paths prefixed with `/api/v3` are accepted without duplicating the prefix
- Absolute URLs are allowed only when they stay under the configured GitBucket API base
- Successful JSON responses are printed to stdout as pretty JSON
- Empty success responses print `null`

Examples:

```bash
gb api user
gb api /api/v3/user
gb api repos/alice/project/issues --input body.json
echo '{"state":"closed"}' | gb api repos/alice/project/issues/1 -X PATCH --input -
```

---

### 3.5 `gb completion` — shell completion generation

`gb completion` prints shell completion scripts to stdout.

```text
gb completion <SHELL>
```

Supported shells:

- `bash`
- `zsh`
- `fish`
- `powershell`

Behavior:

- Generates completions from the current CLI command tree
- Writes the script to stdout
- Does not install or source the script automatically

Examples:

```bash
gb completion bash > ~/.local/share/bash-completion/completions/gb
gb completion zsh > ~/.zfunc/_gb
```

---

### 3.6 `gb repo` — repository operations

#### `gb repo list`

List repositories.

```text
gb repo list [OWNER] [OPTIONS]
```

| Argument/Option | Description |
| --- | --- |
| `OWNER` | User or group name (omit to list your repos) |
| `--json` | Print JSON |

Table output columns:

- `NAME`
- `DESCRIPTION`
- `VISIBILITY`

#### `gb repo view`

Show repository details.

```text
gb repo view [OWNER/REPO] [OPTIONS]
```

| Argument/Option | Short | Description |
| --- | --- | --- |
| `OWNER/REPO` | — | Repository (omit to auto-detect from git remote or pass globally with `gb -R OWNER/REPO repo view`) |
| `--web` | `-w` | Open in browser |

#### `gb repo create`

Create a repository.

```text
gb repo create [NAME] [OPTIONS]
```

| Argument/Option | Short | Default | Description |
| --- | --- | --- | --- |
| `NAME` | — | interactive prompt | Repository name |
| `--description <TEXT>` | `-d` | none | Repository description |
| `--private` | — | `false` | Create as private |
| `--add-readme` | — | `false` | Auto-initialize README |
| `--group <GROUP>` | — | none | Create under a group (`--org` remains accepted as an alias) |

#### `gb repo clone`

Clone a repository with `git clone`.

```text
gb repo clone <REPO> [DIRECTORY]
```

`<REPO>` supports either:

- `OWNER/REPO`
- Full clone URL

#### `gb repo delete`

Delete a repository.

```text
gb repo delete [OWNER/REPO] [OPTIONS]
```

| Argument/Option | Description |
| --- | --- |
| `OWNER/REPO` | Repository to delete (explicit repository required; or pass globally with `gb -R OWNER/REPO repo delete`) |
| `--yes` | Skip confirmation prompt |

#### `gb repo fork`

Fork a repository.

```text
gb repo fork [OWNER/REPO] [OPTIONS]
```

| Argument/Option | Short | Description |
| --- | --- | --- |
| `OWNER/REPO` | — | Repository to fork (or pass globally with `gb -R OWNER/REPO repo fork`) |
| `--group <GROUP>` | — | Group to fork into (defaults to your user; `--org` remains accepted as an alias) |

Implementation detail: if GitBucket returns `404` for the REST fork endpoint, `gb` falls back to the web fork flow and signs in with the configured username or `GB_USER`.

`gb repo delete` behaves similarly: if GitBucket returns `404` for `DELETE /repos/{owner}/{repo}`, `gb` retries through the web danger-zone form.

---

### 3.7 `gb label` — label operations

`gb label` manages repository label definitions in the target repository.

#### `gb label list`

```text
gb label list [--json]
```

Behavior:

- Lists labels in the current repository
- Human output prints `NAME`, `COLOR`, and `DESCRIPTION`
- `--json` prints the raw API payload

#### `gb label view`

```text
gb label view <NAME>
```

Shows the label name, color, description when present, and API URL when present.

#### `gb label create`

```text
gb label create [NAME] [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--color <HEX>` | `-c` | 6-digit hex color (prompted when omitted) |
| `--description <TEXT>` | `-d` | Optional label description |

Behavior:

- Prompts for the label name when omitted
- Accepts colors with or without a leading `#`
- Normalizes colors to lowercase 6-digit hex before sending the API request

#### `gb label delete`

```text
gb label delete <NAME> [OPTIONS]
```

| Option | Description |
| --- | --- |
| `--yes` | Skip confirmation prompt |

---

### 3.8 `gb milestone` — milestone operations

`gb milestone` manages repository milestones in the target repository.

#### `gb milestone list`

```text
gb milestone list [OPTIONS]
```

| Option | Short | Default | Description |
| --- | --- | --- | --- |
| `--state <STATE>` | `-s` | `open` | Filter: `open`, `closed`, `all` |
| `--json` | — | `false` | Print JSON |

Human output columns:

- `#`
- `STATE`
- `TITLE`
- `DUE`
- `OPEN`
- `CLOSED`

#### `gb milestone view`

```text
gb milestone view <NUMBER>
```

Shows the milestone title, state, due date when present, issue counts, description, and URL when present.

#### `gb milestone create`

```text
gb milestone create [TITLE] [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--description <TEXT>` | `-d` | Optional milestone description |
| `--due-on <DATE>` | — | Due date as `YYYY-MM-DD` or RFC3339 |

Behavior:

- Prompts for the title when omitted
- Normalizes `--due-on` to GitBucket-compatible API/web values
- Falls back to the GitBucket web milestone form when the REST create endpoint returns `404`

#### `gb milestone edit`

```text
gb milestone edit <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--title <TEXT>` | `-t` | Updated title |
| `--description <TEXT>` | `-d` | Updated description |
| `--due-on <DATE>` | — | Updated due date as `YYYY-MM-DD`, RFC3339, or an empty string to clear |
| `--state <STATE>` | `-s` | Updated state: `open` or `closed` |

Behavior:

- Requires at least one explicit change
- Falls back to the GitBucket web milestone edit/state routes when the REST update endpoint returns `404`

#### `gb milestone delete`

```text
gb milestone delete <NUMBER> [OPTIONS]
```

| Option | Description |
| --- | --- |
| `--yes` | Skip confirmation prompt |

Behavior:

- Tries `DELETE /repos/{owner}/{repo}/milestones/{number}` first
- Falls back to the GitBucket web delete route when the REST delete endpoint returns `404`

---

### 3.9 `gb issue` — issue operations

#### `gb issue list`

List issues in the target repository.

```text
gb issue list [OPTIONS]
```

| Option | Short | Default | Description |
| --- | --- | --- | --- |
| `--state <STATE>` | `-s` | `open` | Filter: `open`, `closed`, `all` |
| `--json` | — | `false` | Print JSON |

Table output columns:

- `#`
- `STATE`
- `TITLE`
- `AUTHOR`
- `LABELS`

#### `gb issue view`

View issue details.

```text
gb issue view <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--comments` | `-c` | Include comments |
| `--web` | `-w` | Open in browser |

#### `gb issue create`

Create an issue.

```text
gb issue create [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--title <TEXT>` | `-t` | Issue title (prompted when omitted) |
| `--body <TEXT>` | `-b` | Issue body |
| `--label <LABEL>` | `-l` | Repeatable label option |
| `--assignee <USER>` | `-a` | Repeatable assignee option |

#### `gb issue close`

```text
gb issue close <NUMBER>
```

Implementation detail: if GitBucket returns `404` for the REST issue update endpoint, `gb` falls back to the web issue state flow and signs in with the configured username or `GB_USER`.

#### `gb issue reopen`

```text
gb issue reopen <NUMBER>
```

Implementation detail: same fallback behavior as `gb issue close`.

#### `gb issue comment`

```text
gb issue comment <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--body <TEXT>` | `-b` | Comment body (prompted when omitted) |

---

### 3.10 `gb pr` — pull request operations

#### `gb pr list`

List pull requests.

```text
gb pr list [OPTIONS]
```

| Option | Short | Default | Description |
| --- | --- | --- | --- |
| `--state <STATE>` | `-s` | `open` | Filter: `open`, `closed`, `all` |
| `--json` | — | `false` | Print JSON |

Table output columns:

- `#`
- `STATE`
- `TITLE`
- `BRANCH`
- `AUTHOR`

#### `gb pr view`

```text
gb pr view <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--comments` | `-c` | Include comments |
| `--web` | `-w` | Open in browser |

#### `gb pr create`

Create a pull request.

```text
gb pr create [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--title <TEXT>` | `-t` | PR title (prompted when omitted) |
| `--body <TEXT>` | `-b` | PR body |
| `--head <BRANCH>` | — | Head branch (uses current git branch when omitted) |
| `--base <BRANCH>` | `-B` | Base branch (prompts, default `main`) |

#### `gb pr close`

```text
gb pr close <NUMBER>
```

Implementation detail: this operation updates PR state by calling the **issues** endpoint (`PATCH /repos/{owner}/{repo}/issues/{number}` with `state=closed`).

#### `gb pr merge`

```text
gb pr merge <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--message <TEXT>` | `-m` | Merge commit message |

#### `gb pr checkout`

Checkout the PR head branch locally.

```text
gb pr checkout <NUMBER>
```

Execution flow:

1. Fetch PR metadata from API.
2. Resolve the fetch source from the PR head repository clone URL when available, otherwise use `origin`.
3. Run `git fetch <fetch-source> <head-branch>`.
4. Run `git checkout -B <head-branch> FETCH_HEAD`.

#### `gb pr diff`

Show PR diff locally.

```text
gb pr diff <NUMBER>
```

Execution flow:

1. Fetch PR metadata from API.
2. Run `git fetch origin <base>`.
3. Resolve the fetch source from the PR head repository clone URL when available, otherwise use `origin`.
4. Run `git fetch <fetch-source> <head>`.
5. Run `git diff origin/<base>...FETCH_HEAD`.

#### `gb pr comment`

```text
gb pr comment <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--body <TEXT>` | `-b` | Comment body (prompted when omitted) |

---

### 3.11 `gb browse`

Open the repository page in your browser.

```text
gb browse
```

The repository is resolved from:

1. `--repo/-R`
2. `GB_REPO`
3. Git remote (`origin`)

---

## 4. Authentication and configuration

### 4.1 Authentication method

- Personal Access Token (PAT) only.
- Stored per host.
- Verified during login via `GET /user`.
- Successful login updates the saved `default_host`.
- Path-prefixed GitBucket deployments are supported by passing a base URL such as `https://gitbucket.example.com/gitbucket`.

### 4.2 Configuration file

Default path:

```text
~/.config/gb/config.toml
```

Override base directory with `GB_CONFIG_DIR`.

Example:

```toml
default_host = "https://gitbucket.example.com/gitbucket"

[hosts."https://gitbucket.example.com/gitbucket"]
token = "your-token"
user = "alice"
protocol = "https"
```

Path-prefixed instances can also be stored as plain host-plus-path keys:

```toml
[hosts."gitbucket.example.com/gitbucket"]
token = "your-token"
user = "alice"
protocol = "https"
```

On Unix-like systems, `config.toml` is written with `0600` permissions.

### 4.3 Credential precedence

1. `GB_TOKEN` environment variable
2. Host entry in `config.toml`

When `GB_TOKEN` is set, protocol resolution uses this order:

1. URL scheme embedded in `--hostname/-H` or `GB_HOST`
2. `GB_PROTOCOL`
3. Matching stored host configuration from `config.toml`
4. Default `https`

### 4.4 Hostname resolution order

1. `--hostname/-H`
2. `GB_HOST`
3. Saved `default_host` in `config.toml`
4. Lexicographically first configured host in `config.toml` as a backward-compatible fallback

Stored hosts are matched canonically, so equivalent forms such as `https://host/path`, `host/path`, and `https://host/path/api/v3` resolve to the same saved entry.

---

## 5. Repository auto-detection

When `--repo/-R` is omitted, `gb` tries to parse `git remote get-url origin`.

### 5.1 Supported git remote formats

- HTTPS: `https://host/owner/repo.git`
- GitBucket HTTPS with `/git/`: `https://host/git/owner/repo.git`
- Path-prefixed HTTPS: `https://gitbucket.example.com/gitbucket/owner/repo.git`
- Path-prefixed GitBucket HTTPS with `/git/`: `https://gitbucket.example.com/gitbucket/git/owner/repo.git`
- SSH: `git@host:owner/repo.git`

### 5.2 Resolution order

1. `--repo/-R`
2. `GB_REPO`
3. `git remote get-url origin`

If parsing fails, `RepoNotFound` is returned.

---

## 6. Output formats

### 6.1 Table output (default)

- Human-readable columns with auto width.
- ANSI-aware width calculations.
- Unicode-safe truncation for long text fields.
- Colored states:
  - `OPEN` (green)
  - `CLOSED` (red)
  - `MERGED` (magenta)

### 6.2 JSON output

Available on list commands (`repo list`, `issue list`, `pr list`) using `--json`.

### 6.3 Browser output

Available on view/browse flows using `--web` or `gb browse`.

---

## 7. Environment variables

| Variable | Description |
| --- | --- |
| `GB_HOST` | Default hostname or base URL |
| `GB_REPO` | Default repository (`OWNER/REPO`) |
| `GB_TOKEN` | Access token override |
| `GB_PROTOCOL` | Protocol override when `GB_TOKEN` is used with a plain hostname |
| `GB_USER` | Username for GitBucket web-session fallbacks |
| `GB_PASSWORD` | Password for GitBucket web-session fallbacks |
| `GB_CONFIG_DIR` | Custom config directory |
| `NO_COLOR` | Disable colored output (terminal/toolchain dependent) |

---

## 8. Error handling and exit codes

### 8.1 Error categories (`GbError`)

- `Auth(String)`
- `Api { status, message }`
- `Config(String)`
- `NotAuthenticated`
- `RepoNotFound`
- `Http(reqwest::Error)`
- `Io(std::io::Error)`
- `Json(serde_json::Error)`
- `TomlSer(toml::ser::Error)`
- `TomlDe(toml::de::Error)`
- `UrlParse(url::ParseError)`
- `Dialoguer(dialoguer::Error)`
- `Other(String)`

### 8.2 Exit codes

- `0`: Success
- `1`: Error (`main` prints `Error: ...` and exits with code 1)

---

## 9. Technical specification

### 9.1 Project layout

```text
src/
  main.rs
  error.rs
  cli/
    mod.rs
    common.rs
    auth.rs
    repo.rs
    issue.rs
    pr.rs
  api/
    mod.rs
    client.rs
    repository.rs
    issue.rs
    pull_request.rs
  models/
    mod.rs
    user.rs
    repository.rs
    issue.rs
    pull_request.rs
    comment.rs
  config/
    mod.rs
    auth.rs
  output/
    mod.rs
    table.rs
```

### 9.2 Main dependencies

| Crate | Purpose |
| --- | --- |
| `clap` | CLI parsing |
| `reqwest` | HTTP client (`rustls-tls-native-roots`) |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization |
| `toml` | Config encoding |
| `dialoguer` | Interactive prompts |
| `colored` | Colorized terminal output |
| `open` | Browser launching |
| `url` | URL parsing |
| `thiserror` | Error definitions |
| `dirs` | Config directory discovery |

### 9.3 API client behavior

- Base API URL: normalized to `{scheme}://{host}{optional-path}/api/v3`
- Auth header: `Authorization: token <PAT>`
- Common methods: `get`, `post`, `patch`, `put`, `delete`
- Raw request helper: `raw_request` (used by `gb api`)
- `web_url(path)`: converts API base URL to browser base URL by stripping `/api/v3`

### 9.4 Current endpoint mapping

Authentication:

- `GET /user` (token validation, login flow)

Repository:

- `GET /users/{owner}/repos`
- `GET /user/repos`
- `GET /repos/{owner}/{repo}`
- `POST /user/repos`
- `POST /orgs/{org}/repos`
- `DELETE /repos/{owner}/{repo}`
- `POST /repos/{owner}/{repo}/forks`

Issue:

- `GET /repos/{owner}/{repo}/issues?state={state}`
- `GET /repos/{owner}/{repo}/issues/{number}`
- `POST /repos/{owner}/{repo}/issues`
- `PATCH /repos/{owner}/{repo}/issues/{number}`
- `GET /repos/{owner}/{repo}/issues/{number}/comments`
- `POST /repos/{owner}/{repo}/issues/{number}/comments`

Pull request:

- `GET /repos/{owner}/{repo}/pulls?state={state}`
- `GET /repos/{owner}/{repo}/pulls/{number}`
- `POST /repos/{owner}/{repo}/pulls`
- `PUT /repos/{owner}/{repo}/pulls/{number}/merge`
- `GET /repos/{owner}/{repo}/issues/{number}/comments` (PR comments)
- `POST /repos/{owner}/{repo}/issues/{number}/comments` (PR comments)

Label:

- `GET /repos/{owner}/{repo}/labels`
- `GET /repos/{owner}/{repo}/labels/{name}`
- `POST /repos/{owner}/{repo}/labels`
- `DELETE /repos/{owner}/{repo}/labels/{name}`

Milestone:

- `GET /repos/{owner}/{repo}/milestones?state={state}`
- `GET /repos/{owner}/{repo}/milestones/{number}`
- `POST /repos/{owner}/{repo}/milestones`
- `PATCH /repos/{owner}/{repo}/milestones/{number}`
- `DELETE /repos/{owner}/{repo}/milestones/{number}`
- `POST /{owner}/{repo}/issues/milestones/new` (GitBucket web fallback)
- `POST /{owner}/{repo}/issues/milestones/{number}/edit` (GitBucket web fallback)
- `GET /{owner}/{repo}/issues/milestones/{number}/open` (GitBucket web fallback)
- `GET /{owner}/{repo}/issues/milestones/{number}/close` (GitBucket web fallback)
- `GET /{owner}/{repo}/issues/milestones/{number}/delete` (GitBucket web fallback)

---

## 10. Roadmap

### Phase 1 next

- Keep strengthening test and E2E coverage around the current command set

### Phase 2

- Expand issue/PR metadata handling

### Phase 3

- Re-evaluate webhook and collaborator operations after the current command set stabilizes

### Phase 4

- More tests (unit + integration)
- CI workflows
- Packaging/release automation
- Documentation refinements

### Backlog / re-evaluate later

- `gb user`
- `gb webhook`
- `gb repo collaborator`
