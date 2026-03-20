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
| `gh label` | `gb label` | 📋 Planned | Phase 2 |
| `gh api` | `gb api` | 📋 Planned | Phase 3 |
| `gh config` | `gb config` | 📋 Planned | Phase 3 |
| `gh completion` | `gb completion` | 📋 Planned | Phase 3 |
| `gh gist` / `gh project` / `gh codespace` | — | ❌ Out of scope | No corresponding GitBucket feature/API |
| `gh run` / `gh workflow` / `gh cache` | — | ❌ Out of scope | No Actions-like CI feature in GitBucket core |

### 2.2 GitBucket-specific opportunities (planned)

| Command | Description | Status |
| --- | --- | --- |
| `gb milestone` | Milestone management | 📋 Planned (Phase 2) |
| `gb user` | User/admin workflows | 📋 Planned (Phase 2) |
| `gb webhook` | Webhook management | 📋 Planned (Phase 3) |
| `gb repo collaborator` | Collaborator management | 📋 Planned (Phase 3) |

---

## 3. Command reference

### 3.1 Global options

Available on all commands:

| Option | Short | Env var | Description |
| --- | --- | --- | --- |
| `--hostname <HOST>` | `-H` | `GB_HOST` | GitBucket host or base URL (e.g. `gitbucket.example.com` or `https://localhost/gitbucket`) |
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
3. Save the host profile to `config.toml` on success.

Examples:

```bash
gb auth login
gb auth login -H gitbucket.example.com -t <TOKEN>
gb auth login -H localhost:8080 --protocol http
gb auth login -H https://localhost/gitbucket -t <TOKEN>
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

### 3.3 `gb repo` — repository operations

#### `gb repo list`

List repositories.

```text
gb repo list [OWNER] [OPTIONS]
```

| Argument/Option | Description |
| --- | --- |
| `OWNER` | User or organization name (omit to list your repos) |
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
| `OWNER/REPO` | — | Repository (omit to auto-detect from git remote) |
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
| `--org <ORG>` | — | none | Create under an organization |

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
| `OWNER/REPO` | Repository to delete (omit to auto-detect) |
| `--yes` | Skip confirmation prompt |

#### `gb repo fork`

Fork a repository.

```text
gb repo fork [OWNER/REPO]
```

---

### 3.4 `gb issue` — issue operations

#### `gb issue list`

List issues in the target repository.

```text
gb issue list [OPTIONS]
```

| Option | Short | Default | Description |
| --- | --- | --- | --- |
| `--state <STATE>` | `-s` | `open` | Intended filter: `open`, `closed`, `all` |
| `--json` | — | `false` | Print JSON |

> Current implementation note: `--state` is accepted by CLI, but listing currently uses the default API response without applying explicit state filtering in the request.

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

#### `gb issue reopen`

```text
gb issue reopen <NUMBER>
```

#### `gb issue comment`

```text
gb issue comment <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--body <TEXT>` | `-b` | Comment body (prompted when omitted) |

---

### 3.5 `gb pr` — pull request operations

#### `gb pr list`

List pull requests.

```text
gb pr list [OPTIONS]
```

| Option | Short | Default | Description |
| --- | --- | --- | --- |
| `--state <STATE>` | `-s` | `open` | Intended filter: `open`, `closed`, `all` |
| `--json` | — | `false` | Print JSON |

> Current implementation note: `--state` is accepted by CLI, but listing currently uses the default API response without applying explicit state filtering in the request.

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
| `--head <BRANCH>` | `-H` | Head branch (uses current git branch when omitted) |
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
2. Run `git fetch origin <head-branch>`.
3. Run `git checkout <head-branch>`.

#### `gb pr diff`

Show PR diff locally.

```text
gb pr diff <NUMBER>
```

Execution flow:

1. Fetch PR metadata from API.
2. Run `git fetch origin <head> <base>`.
3. Run `git diff origin/<base>...origin/<head>`.

#### `gb pr comment`

```text
gb pr comment <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--body <TEXT>` | `-b` | Comment body (prompted when omitted) |

---

### 3.6 `gb browse`

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
- Path-prefixed GitBucket deployments are supported by passing a base URL such as `https://localhost/gitbucket`.

### 4.2 Configuration file

Default path:

```text
~/.config/gb/config.toml
```

Override base directory with `GB_CONFIG_DIR`.

Example:

```toml
[hosts."gitbucket.example.com"]
token = "your-token"
user = "alice"
protocol = "https"
```

Path-prefixed instances can also be stored as keys:

```toml
[hosts."https://localhost/gitbucket"]
token = "your-token"
user = "alice"
protocol = "https"
```

### 4.3 Credential precedence

1. `GB_TOKEN` environment variable
2. Host entry in `config.toml`

### 4.4 Hostname resolution order

1. `--hostname/-H`
2. `GB_HOST`
3. First configured host in `config.toml`

---

## 5. Repository auto-detection

When `--repo/-R` is omitted, `gb` tries to parse `git remote get-url origin`.

### 5.1 Supported git remote formats

- HTTPS: `https://host/owner/repo.git`
- GitBucket HTTPS with `/git/`: `https://host/git/owner/repo.git`
- Path-prefixed HTTPS: `https://host/gitbucket/owner/repo.git`
- Path-prefixed GitBucket HTTPS with `/git/`: `https://host/gitbucket/git/owner/repo.git`
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

- Base API URL: `{protocol}://{hostname}/api/v3`
- Auth header: `Authorization: token <PAT>`
- Common methods: `get`, `post`, `patch`, `put`, `delete`
- Planned extension helper: `raw_request` (for future `gb api`)
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

- `GET /repos/{owner}/{repo}/issues`
- `GET /repos/{owner}/{repo}/issues/{number}`
- `POST /repos/{owner}/{repo}/issues`
- `PATCH /repos/{owner}/{repo}/issues/{number}`
- `GET /repos/{owner}/{repo}/issues/{number}/comments`
- `POST /repos/{owner}/{repo}/issues/{number}/comments`

Pull request:

- `GET /repos/{owner}/{repo}/pulls`
- `GET /repos/{owner}/{repo}/pulls/{number}`
- `POST /repos/{owner}/{repo}/pulls`
- `PUT /repos/{owner}/{repo}/pulls/{number}/merge`
- `GET /repos/{owner}/{repo}/issues/{number}/comments` (PR comments)
- `POST /repos/{owner}/{repo}/issues/{number}/comments` (PR comments)

---

## 10. Roadmap

### Phase 2

- `gb label`
- `gb milestone`
- Expand issue/PR metadata handling

### Phase 3

- `gb api`
- `gb config`
- `gb completion`
- Webhook and collaborator operations

### Phase 4

- More tests (unit + integration)
- CI workflows
- Packaging/release automation
- Documentation refinements
