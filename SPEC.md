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
| `gh issue` | `gb issue` | ✅ Implemented | Issue create/view/edit/comment/state workflows |
| `gh pr` | `gb pr` | ✅ Implemented | PR create/view/edit/comment/state/merge/worktree workflows |
| `gh browse` | `gb browse` | ✅ Implemented | Opens browser |
| `gh label` | `gb label` | ✅ Implemented | Label definition CRUD |
| `gh api` | `gb api` | ✅ Implemented | Raw REST API access |
| `gh config` | `gb config` | ✅ Implemented | Local config inspection and updates |
| `gh completion` | `gb completion` | ✅ Implemented | Shell completion generation |
| `gh gist` / `gh project` / `gh codespace` | — | ❌ Out of scope | No corresponding GitBucket feature/API |
| `gh run` / `gh workflow` / `gh cache` | — | ❌ Out of scope | No Actions-like CI feature in GitBucket core |

### 2.2 GitBucket-specific opportunities (planned)

| Command | Description | Status |
| --- | --- | --- |
| `gb milestone` | Milestone management | ✅ Implemented | REST read/write + title-limited Web support |
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
| `--profile <NAME>` | — | `GB_PROFILE` | Configuration profile |
| `--json-errors` | — | `GB_JSON_ERRORS` | Print command failures as structured JSON on stderr |
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
gb auth status [--json]
```

`--profile <NAME>` limits output to that profile and prints the profile's effective actor. `--json` prints structured output with a top-level `effective_actor` object containing `host`, `user`, `protocol`, and `credential_source`.

Example output:

```text
gitbucket.example.com
  ✓ Logged in as alice
  Protocol: https
```

Profile example:

```text
Profile: work
  Default host: gitbucket.example.com
  Default repo: alice/project
  Effective actor: alice @ gitbucket.example.com (profile credentials)
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
- Prints the stored `default_host` and `default_profile` when set
- Lists saved host and profile entries with `user`, `protocol`, and whether a token is configured
- Redacts token values; use `gb auth token` when the raw token is needed

#### `gb config get default-host`

```text
gb config get default-host
```

Print the stored `default_host` value.

#### `gb config get default-profile`

```text
gb config get default-profile
```

Print the stored `default_profile` value.

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

#### `gb config get profile`

```text
gb config get profile <NAME> [--json]
```

Print profile defaults and profile-scoped saved hosts. Token values are redacted.

#### `gb config set default-host`

```text
gb config set default-host <HOST>
```

Set the stored default host. The target host must already exist in saved config.

#### `gb config set default-profile`

```text
gb config set default-profile <NAME>
```

Set the stored default profile. The target profile must already exist in saved config.

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

#### `gb config set profile`

```text
gb config set profile <NAME> [--default-host <HOST>] [--default-repo <OWNER/REPO>]
```

Create or update a saved profile. `--default-repo` must use `OWNER/REPO` format.

#### `gb config unset default-host`

```text
gb config unset default-host
```

Clear the stored `default_host` value.

#### `gb config unset default-profile`

```text
gb config unset default-profile
```

Clear the stored `default_profile` value.

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
| `OWNER/REPO` | — | Repository (omit to auto-detect from git remote or pass with `-R/--repo`) |
| `--repo <OWNER/REPO>` | `-R` | Target repository |
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

For `OWNER/REPO`, HTTP(S) clones reuse the selected host/profile user and token
through an ephemeral Git credential helper. `GB_USER` supplies the username when
using `GB_TOKEN` without a saved user. Full URLs use Git's own authentication;
HTTP URLs containing a password are rejected before invoking Git.

The helper only supplies credentials within the configured scheme/host/port/base
path, and validates the exact Git credential context again after URL rewriting.
It does not persist credentials, invoke configured credential-store helpers, or
put the token in argv/remote URLs. Token-bearing transfers reject redirects,
including URL-specific redirect settings for the rewritten URL, and restrict
rewritten transports to HTTP(S). Credential path checks account for Git's
percent-decoding and removal of leading/trailing slashes, and reject other
path differences.
Secrets are passed through the subprocess environment and remain accessible to
local processes with sufficient inspection privileges. SSH/external origins do
not receive the selected GitBucket token.

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
| `--description <TEXT>` | `-d` | Recognized but unsupported; rejected before mutation |

Behavior:

- Prompts for the label name when omitted
- Accepts colors with or without a leading `#`
- Normalizes colors to lowercase 6-digit hex before sending the API request
- Description write options fail before changes because the verified API/Web
  contracts do not persist them; unknown instances are not assumed to support them.
- Reads back stored values before success; failed verification reports possibly
  applied changes and asks the user to inspect before retrying.

#### `gb label edit`

```text
gb label edit <NAME> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--name <NAME>` | — | Rename the label |
| `--color <HEX>` | `-c` | New 6-digit hex color |
| `--description <TEXT>` | `-d` | Recognized but unsupported; rejected before mutation |
| `--remove-description` | — | Recognized but unsupported; rejected before mutation |
| `--web` | — | Permit verified in-place color updates after same-name REST rejection |

Behavior:

- Requires at least one change option
- Accepts colors with or without a leading `#`
- Normalizes colors to lowercase 6-digit hex before sending the API request
- Percent-encodes the current label name in the API path. Empty names, dot segments,
  backslashes and controls (including ambiguous decoded forms) fail before requests.
- Fetches current fields and always supplies name/color for PATCH. Rename keeps the
  stored color when omitted. No-op requests need no PATCH or Web session.
- Only a same-name, non-no-op REST update rejected with 422 permits Web color
  supplementation, and only with `--web`. A new-name conflict or other HTTP failure
  is returned directly. Servers accepting REST need no Web credentials.
- The Web adapter identifies one numeric ID from the row's repository/name link,
  confirms the edit form's name/color and ID, updates that ID in place, and verifies
  identity plus persisted values. No delete/create or temporary rename is used.
- Rejects unrecognized/ambiguous layout and snapshot mismatches before form mutation.
  Web name trimming, commas unsupported by the form, or a description that cannot be
  preserved cause an explicit error before Web authentication. The Web adapter uses
  the real HTML parser and sanitized pinned-server fixtures, including proxy links.
- Description changes/clears are unsupported before any mutation; existing read
  descriptions are echoed for REST preservation and checked afterward.
- Read/merge/write and Web steps are not a conditional transaction. See
  `docs/UPSTREAM_LABEL_API.md` for remaining server-side work.

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
- Tries REST first. Only a `404` combined with a title outside the confirmed 4.44.0
  REST restriction (100 ASCII letters/digits or `-+_.`) permits Web supplementation.
  Other REST errors remain errors; `404` alone never means an endpoint is missing.
- Before Web authentication, validates text round-tripping and reads the repository
  milestone list; missing/inaccessible repositories and existing duplicate titles
  fail without Web creation. After submission, requires a newly observed milestone
  with the requested stored fields; failed verification is an error with retry guidance.
- Titles must be nonblank and at most 100 UTF-16 code units, matching the server.
  Japanese/space-containing titles are never rewritten into ASCII.

#### `gb milestone edit`

```text
gb milestone edit <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--title <TEXT>` | `-t` | Updated title |
| `--description <TEXT>` | `-d` | Updated description, or an empty string to clear |
| `--due-on <DATE>` | — | Updated due date as `YYYY-MM-DD`, RFC3339, or an empty string to clear |
| `--state <STATE>` | `-s` | Updated state: `open` or `closed` |

Behavior:

- Requires at least one explicit change
- Distinguishes unchanged, set and clear intent for nullable fields. Fetches the
  current milestone and sends the complete title/state/description/due_on payload;
  omitted options retain their stored values. Closed state and existing timestamps
  are preserved, without normalizing an unchanged timestamp. Clear uses JSON null.
- Explicit due-date input retains the existing CLI calendar-date semantics: the
  date portion of `YYYY-MM-DD`/RFC3339 becomes midnight UTC for REST.
- Only a `404` on a title outside the known REST title restriction permits Web use.
  REST-compatible titles, missing targets and other HTTP failures do not trigger it.
  Reloads the milestone before Web authentication and merges current values again.
  If the refreshed title is REST-compatible, retries REST with that snapshot.
  Repeating a state-only request for the current state needs no mutation or Web authentication.
- Web content forms trim leading/trailing U+0000..U+0020 characters, matching
  Java `String.trim`. Preflight validation rejects those values and retains other
  Unicode spacing in nonblank titles/descriptions. Web forms also cannot guarantee
  the instant of an existing due date (date-only input/server timezone).
  Refuse lossy text or an unchanged nonempty due timestamp before authentication;
  users must explicitly set/clear `--due-on` or use a REST-compatible title.
  The legacy unset-date sentinel is treated as unset. State-only Web updates send
  only the state action and retain description/due date, including timestamps.
- Read back Web results and verify title/state/description/due instant before
  success. A server form timezone mismatch, validation page, failed read-back or
  later state failure is reported as unverified/possibly partially applied; inspect
  the milestone before retrying. Never silently declare a failed Web form successful.
- Read/merge/write is a snapshot-based compatibility adapter, not a conditional
  server transaction; concurrent unrelated writers can still race.

#### `gb milestone delete`

```text
gb milestone delete <NUMBER> [OPTIONS]
```

| Option | Description |
| --- | --- |
| `--yes` | Skip confirmation prompt |

Behavior:

- Tries `DELETE /repos/{owner}/{repo}/milestones/{number}` first
- Returns REST errors directly, including `404`; never prompts for Web credentials.
- GitBucket 4.44.0 returns `204` even when already absent, which is treated as success.

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
| `--no-pager` | — | `false` | Do not use a pager |

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
| `--json` | — | Print JSON |
| `--no-pager` | — | Do not use a pager |

Shows title, state, author, created date, labels, assignees, milestone when present, body, and optional comments.
Comments are hidden by default; pass `--comments` to include them in the CLI output.

#### `gb issue create`

Create an issue.

```text
gb issue create [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--title <TEXT>` | `-t` | Issue title (prompted when omitted) |
| `--body <TEXT>` | `-b` | Issue body |
| `--label <LABEL>` | `-l` | Repeatable or comma-separated label option |
| `--assignee <USER>` | `-a` | Repeatable or comma-separated assignee option |

#### `gb issue edit`

Edit an issue.

```text
gb issue edit <NUMBER> [OPTIONS]
```

| Option | Description |
| --- | --- |
| `--title <TEXT>` | Updated title |
| `--body <TEXT>` | Updated body |
| `--add-label <LABEL>` | Add a label; repeatable or comma-separated |
| `--remove-label <LABEL>` | Remove a label; repeatable or comma-separated |
| `--add-assignee <USER>` | Add an assignee; repeatable or comma-separated |
| `--remove-assignee <USER>` | Remove an assignee; repeatable or comma-separated |
| `--milestone <NUMBER>` | Set the milestone |
| `--remove-milestone` | Clear the current milestone |
| `--state <STATE>` | Updated state: `open` or `closed` |

Behavior:

- Requires at least one explicit change
- Fetches and validates the current Issue number before edits. Label additions
  preflight the existing repository labels, avoiding implicit server-side creation.
- Applies labels through POST additions and per-label DELETE operations, never
  Issue PATCH or a full-set PUT. Removals run before additions; addition wins if
  both are requested. Fresh label reads avoid replacing unrelated concurrent labels.
- Metadata and labels are separate phases. Metadata is applied and verified before
  label writes. Label-only requests send no Issue PATCH and open no Web session.
- Reads stored labels/metadata before success. Partial failures identify confirmed
  steps and require inspection before retrying; these phases are not atomic.
- Rejects `--milestone` and `--remove-milestone` together before any API call
- Title/body/assignee/milestone/state may still use Web when Issue PATCH is unavailable; label changes use their own REST routes regardless of Issue PATCH support.

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
| `--edit-last` | — | Edit your last comment instead of adding a new one |

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
| `--no-pager` | — | `false` | Do not use a pager |

With `--state open --json`, `gb` fetches each listed pull request detail before printing JSON so fork `head`/`base` repository information is as reliable as `gb pr view --json`. The issue-listing recovery for GitBucket versions that omit repository-visible open PRs from `/pulls` still applies.

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
| `--json` | — | Print JSON |
| `--no-pager` | — | Do not use a pager |

With `--json`, `gb` prints the pull request API payload. When `--comments` is also passed, the JSON object includes a top-level `comments` array fetched from the PR comments endpoint.

#### `gb pr create`

Create a pull request.

```text
gb pr create [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--title <TEXT>` | `-t` | PR title (prompted when omitted) |
| `--body <TEXT>` | `-b` | PR body |
| `--head <BRANCH>` | — | Head branch (uses current git branch when omitted). For cross-repo PRs, pass `OWNER:BRANCH` or use `--head-owner OWNER --head BRANCH` |
| `--head-owner <OWNER>` | — | Owner for the head branch; sends the head as `OWNER:BRANCH` |
| `--base <BRANCH>` | `-B` | Base branch (prompts, default `main`) |
| `--json` | — | Print the created pull request as JSON |
| `--detect-existing` | — | Return an existing open PR for the same head/base instead of creating a duplicate |

On normal text output, `gb pr create` prints the created PR number/title, resolved `Head:` and `Base:` repository refs when the API returns them, and a canonical browser URL using `/pull/{number}`.
With `--detect-existing`, `gb` checks open pull requests for a matching head/base before prompting for title/body or creating a new PR. If the pull request listing is incomplete or lacks comparable refs, `gb` also checks PRs exposed through the issue listing and fetches PR details before deciding no match exists. If create fails after that check, `gb` checks again and returns the matching open PR when one exists.

Numbered PR operations validate the target through `GET /repos/{owner}/{repo}/pulls/{number}`
first and reject returned-number mismatches. The compatibility lookup for null
single-PR responses searches only the PR collection, not ordinary issues. This
check also precedes comment list/post/edit, merge, checkout/diff and browser view;
comment subresources still use the Issue comments API after validation.

#### `gb pr edit`

Edit supported PR fields through the pull-request endpoint; update assignees separately.

```text
gb pr edit <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--title <TEXT>` | `-t` | New PR title |
| `--body <TEXT>` | `-b` | New PR body |
| `--add-assignee <USER>` | — | Add an assignee; repeatable or comma-separated |
| `--remove-assignee <USER>` | — | Remove an assignee; repeatable or comma-separated |
| `--state <STATE>` | — | Update PR state: `open` or `closed` |
| `--web` | — | Permit Web UI assignee changes; title/body/state remain REST operations |

- Requires at least one edit and rejects blank new titles; an empty body is an
  explicit clear.
- Validates the number through the canonical PR endpoint before any mutation.
  Uses `UpdatePullRequest` and `PATCH /repos/{owner}/{repo}/pulls/{number}` for
  title/body/state only. Never sends a PR edit to Issue PATCH.
- GitBucket 4.44 updates body only inside its title update. Content edits supply
  both the requested/existing title and requested/existing body; absent stored
  bodies stay absent. State-only edits omit both content fields to preserve them.
- Normalizes requested state and skips an already-satisfied state change. Reopening
  a merged PR is refused before any content/assignee edit; title/body edits on
  closed/merged PRs do not implicitly reopen them.
- Checks returned PR number/title/body/state against the intended result before
  continuing. REST errors, including 404, propagate without Web retries, even
  with `--web`. This flag grants permission; it does not force Web use.
- Reads assignees separately through the Issue detail endpoint after PR validation,
  and sends no assignee field in PR PATCH. An actual delta requires `--web`;
  an already-satisfied assignee request needs no Web session.
- Before a mixed REST/Web edit, require Web permission and attempt sign-in before
  REST mutation. Then update REST fields and apply only assignee additions/removals
  through the existing Web adapter. Read back and verify the desired assignee set.
- On a later Web/read-back failure, do not print success; identify completed REST
  fields and possibly partial assignee changes, with inspection-before-retry
  guidance. There is no destructive rollback or automatic retry.
- Content pairing uses a read snapshot, and combined REST/Web edits are not atomic.
  The existing shared sign-in helper's principal verification and a generic
  mutation-result schema are follow-up work, not guarantees of this adapter.

#### `gb pr close`

```text
gb pr close <NUMBER>
```

Uses the same validated PR edit path with a state-only PR PATCH. An already
closed PR needs no mutation. No Web session or Issue PATCH is used.

#### `gb pr reopen`

```text
gb pr reopen <NUMBER>
```

Uses the same validated PR edit path with `state=open`. Already-open PRs need no
mutation. Merged PRs are refused. Equivalent to `gb pr edit <NUMBER> --state open`.

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
2. Resolve the head repository clone URL or a remote matching its server and repository path, otherwise use `origin` when repository metadata is absent.
3. Validate the head branch as a full `refs/heads/` ref. Acquire an OS lock in the
   Git common directory (`gb-pr-checkout.lock`), shared with linked worktrees.
   Reject dirty index/worktree state (including untracked files and submodule
   changes hidden by Git ignore settings), operations in
   progress, branches owned by other PR targets, and use in another worktree.
4. Fetch only the head branch into a target-specific internal ref under
   `refs/gb/pr-targets/`, using the same scoped credential helper as
   `repo clone OWNER/REPO`. The namespace is `refs/gb/pr-targets/v1/<SHA-256>`,
   hashing the JSON tuple of canonical web base URL
   (scheme/host/port/deployment path), owner, repository and PR number. The fixed
   64-digit digest keeps ref paths short for long deployment URLs on Windows;
   branch provenance retains the complete tuple for exact target comparison.
   Checkout and diff use this namespace; legacy `refs/gb/pr/<NUMBER>` refs are
   untouched. Fetch uses a fully qualified source ref, no tag following, no
   `FETCH_HEAD` write, no submodule recursion, and an empty refmap to avoid
   configured tracking updates.
   Force is allowed only on the internal destination ref, including force-pushed PRs.
5. Recheck cleanliness, branch tip and worktree ownership after fetching. A
   preexisting `pr-<NUMBER>` must have the exact target recorded in the local
   `branch.pr-<NUMBER>.gb-pr-target` Git config value, and its tip must equal the
   fetched head commit. Otherwise fail without resetting it. No fast-forward,
   force/update option or automatic branch renaming is provided.
6. For a new branch, record the target before switching (so a config write error
   leaves HEAD untouched), then run non-forcing `git checkout --no-track -b` from
   the fetched commit. For a reusable branch, switch without resetting its tip.
   Both use `--no-guess`, `--no-overwrite-ignore` and `--no-recurse-submodules`.
   Submodule working trees and HEADs remain untouched regardless of
   `submodule.recurse`; users inspect and update changed submodule commits
   separately. On a pre-switch failure,
   restore the previous metadata; do not reset, clean or stash user work.

Unrecorded legacy branches are refused even at the same commit. Users can rename
these branches themselves to retain them before retrying. A lock conflict fails
with a retry message; the empty lock file is never unlinked, and OS process exit
releases the lock. This coordinates `gb` checkouts, not unrelated Git commands.
Repository hooks retain Git's normal behavior. If checkout switches but a
post-checkout hook subsequently fails, report the completed switch explicitly,
retain the provenance record, and do not attempt a destructive rollback. A
failed operation may leave fetched objects/internal refs; it never resets existing
user branch tips. Repositories and their hooks/configuration remain trusted.

The token is not embedded in Git argv or persisted in Git configuration. HTTP
redirects are disabled for transfers using the GitBucket token. A URL rewritten
outside the exact approved Git credential context receives no token. Same-named
repositories on other servers or deployments are not interchangeable remotes.

#### `gb pr diff`

Show PR diff locally.

```text
gb pr diff <NUMBER> [--no-pager]
```

| Option | Short | Description |
| --- | --- | --- |
| `--no-pager` | — | Do not use a pager |

Execution flow:

1. Fetch PR metadata from API.
2. Resolve the base and head fetch sources from PR repository clone URLs when available, otherwise use matching local remotes or `origin`.
3. Run `git fetch <base-fetch-source> <base>`.
4. Run `git fetch <fetch-source> <head>`.
5. Run `git diff <base-ref>...<head-ref>`.
6. If the live branch diff is unavailable, fetch a saved PR diff from `diff_url`/`patch_url`, the issue PR metadata, or the GitBucket web diff URL.

If the API response lacks the PR head/base refs, a branch fetch fails, or a non-open PR produces an empty local diff and no saved PR diff can be fetched, `gb pr diff` exits non-zero and reports that the diff is unavailable on stderr. With `--json-errors`, the error includes `code: "diff_unavailable"` and a machine-readable `cause` such as `missing_head_ref`, `base_fetch_failed`, `head_fetch_failed`, or `empty_live_diff`.

#### `gb pr comment`

```text
gb pr comment <NUMBER> [OPTIONS]
gb pr comment list <NUMBER> [OPTIONS]
```

| Option | Short | Description |
| --- | --- | --- |
| `--body <TEXT>` | `-b` | Comment body (prompted when omitted) |
| `--edit-last` | — | Edit your last comment instead of adding a new one |
| `--json` | — | Print the created or edited comment object as JSON |
| `--no-pager` | — | Do not use a pager for `gb pr comment list` |

On normal text output, `gb pr comment` prints the comment ID. If the API returns `html_url`, it also prints the comment URL.

`gb pr comment list <NUMBER>` lists comments on the pull request. Pass `--json` to print the full comment array, which can be used to verify whether a previous comment mutation succeeded before retrying. Pass `--no-pager` to force direct stdout output.

---

### 3.11 `gb browse`

Open the repository page in your browser.

```text
gb browse
```

The repository is resolved from:

1. `--repo/-R`
2. `GB_REPO`
3. Active profile's `default_repo`
4. Git remote (`origin`)

---

## 4. Authentication and configuration

### 4.1 Authentication method

- Personal Access Token (PAT) only.
- Stored per host.
- Verified during login via `GET /user`.
- Successful login updates the saved `default_host`.
- When a profile is selected, successful login writes the host entry under that profile and updates the profile's `default_host`.
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
default_profile = "work"

[hosts."https://gitbucket.example.com/gitbucket"]
token = "your-token"
user = "alice"
protocol = "https"

[profiles.work]
default_host = "gitbucket.example.com"
default_repo = "alice/project"

[profiles.work.hosts."gitbucket.example.com"]
token = "your-work-token"
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

Configuration mutations use a single `AuthConfig::update` transaction: acquire
an OS lock on the stable sibling `config.toml.lock`, reload the latest TOML,
validate and apply the change, serialize, then persist before returning success.
Lock contention waits up to five seconds and fails without printing success or
changing configuration. Locks release on process exit; the empty lock file stays
and must not be deleted. Login verifies credentials before entering this
transaction and applies the verified credentials to the latest configuration.
Prompts and network requests must not run inside the transaction.

Changed content is written to a same-directory temporary file, synced, and
atomically replaced. Unix additionally syncs the parent directory; a failure at
this last stage reports that replacement already occurred and asks the user to
reload before retrying. Earlier write/replace failures retain the previous file;
ordinary errors clean up the temporary file. Abrupt termination can leave a
private temporary file. Unchanged settings preserve the original bytes (including
comments) and do not create an otherwise absent configuration. Actual mutations
serialize TOML, as before. Invalid existing TOML is never overwritten by a
mutation. Reads observe complete old or new files without taking the writer lock.

On Unix, configuration and lock files use `0600`; newly created configuration
directories use `0700`, without changing existing directories. Windows files
inherit directory access permissions. Transient Windows access/sharing conflicts
are retried for up to one second per filesystem operation. Replacement retries
retain the same synced temporary file; they never delete the old configuration
first, and persistent failures clean up the temporary file. Configuration and
lock paths reject
symlinks, reparse points, and non-regular files; `GB_CONFIG_DIR` can instead name
the directory holding the real configuration. The parent directory must be
trusted. Locking coordinates participating writers only, and atomicity/durability
depend on filesystem support; manual editors, older clients, network filesystems,
and power loss are not covered by a universal durability guarantee.

### 4.3 Credential precedence

1. `GB_TOKEN` environment variable
2. Profile-scoped host entry in `config.toml`
3. Global host entry in `config.toml`

When `GB_TOKEN` is set, protocol resolution uses this order:

1. URL scheme embedded in `--hostname/-H` or `GB_HOST`
2. `GB_PROTOCOL`
3. Matching stored host configuration from `config.toml`
4. Default `https`

### 4.4 Profile resolution order

1. `--profile`
2. `GB_PROFILE`
3. Saved `default_profile` in `config.toml`
4. No active profile

If a selected profile does not exist, commands fail with a clear configuration error.

### 4.5 Hostname resolution order

1. `--hostname/-H`
2. `GB_HOST`
3. Active profile's `default_host`
4. Saved `default_host` in `config.toml` when no profile is active
5. Lexicographically first configured host in `config.toml` as a backward-compatible fallback when no profile is active

Stored hosts are matched canonically, so equivalent forms such as `https://host/path`, `host/path`, and `https://host/path/api/v3` resolve to the same saved entry.

### 4.6 Authenticated request URL boundaries

API requests, including `gb api` and pagination, are validated in the HTTP client
against the normalized API base. Web-origin text fetches are restricted to the
configured deployment base. Validation includes percent-decoded traversal paths;
encoded path separators are allowed only when the interpreted path remains
inside the boundary. Request and configured base paths are compared at the same
decoding level; equivalent encoded path segments also match in pagination and
origin-text URLs. URL credentials and fragments are rejected. Authenticated
API client requests do not follow redirects, so proxies must expose a direct
base URL. The separate password-based Web session flow retains its own redirect
handling; this change does not replace that flow.

Base URLs must use HTTP(S) and have no credentials, query, or fragment.

---

## 5. Repository auto-detection

When CLI/environment/profile repository selection is absent, `gb` parses and validates `git remote get-url origin` against the selected GitBucket instance before issuing requests.

### 5.1 Supported git remote formats

- HTTPS: `https://host/owner/repo.git`
- GitBucket HTTPS with `/git/`: `https://host/git/owner/repo.git`
- Path-prefixed HTTPS: `https://gitbucket.example.com/gitbucket/owner/repo.git`
- Path-prefixed GitBucket HTTPS with `/git/`: `https://gitbucket.example.com/gitbucket/git/owner/repo.git`
- SCP-like SSH: `git@host:owner/repo.git`
- SSH URI with an independent SSH port: `ssh://git@host:29418/owner/repo.git`
- Bracketed IPv6 SSH authorities are supported.

### 5.2 Resolution order

1. `--repo/-R`
2. `GB_REPO`
3. Active profile's `default_repo`
4. `git remote get-url origin`

For origin-derived targets, HTTP(S) scheme/hostname/port and deployment path must
match the configured instance. The relative path must be `OWNER/REPO[.git]` or,
for HTTP(S), `git/OWNER/REPO[.git]`; a trailing slash is accepted. SSH matches
hostname ignoring ASCII case and matches the deployment path, not the HTTP port;
SSH aliases are not resolved. If a subpath cannot be established
from SSH, the command fails before any API request and suggests an explicit
`--hostname`/`--repo`. Explicit/environment/profile repository choices bypass
origin inference, preserving intentional cross-repository operations.

Repository names contain exactly two nonempty safe path components. Slash,
backslash, URL delimiters/escapes, whitespace/control characters, and complete
`.`/`..` components are rejected. Non-whitespace Unicode names are supported.
The selected configuration is loaded once for a repository context; API and Git
clients use the same resolved host credentials.

If origin is unavailable, `RepoNotFound` is returned. Malformed or mismatched
origin targets fail with an explicit target-selection error.

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

Available on list commands (`repo list`, `issue list`, `pr list`, `label list`, `milestone list`, `config list`) using `--json`.

### 6.3 Browser output

Available on view/browse flows using `--web` or `gb browse`.

### 6.4 Web fallback notices

When a command falls back from a missing REST API endpoint to a GitBucket web UI route, `gb` writes a `Notice:` line to stderr before performing the web session flow. Unsupported fields must return an explicit error that distinguishes "REST API unavailable" from "web fallback does not support this field."

---

## 7. Environment variables

| Variable | Description |
| --- | --- |
| `GB_HOST` | Default hostname or base URL |
| `GB_REPO` | Default repository (`OWNER/REPO`) |
| `GB_PROFILE` | Default configuration profile |
| `GB_TOKEN` | Access token override |
| `GB_PROTOCOL` | Protocol override when `GB_TOKEN` is used with a plain hostname |
| `GB_USER` | Username for GitBucket web-session fallbacks |
| `GB_PASSWORD` | Password for GitBucket web-session fallbacks |
| `GB_CONFIG_DIR` | Custom config directory |
| `GB_JSON_ERRORS` | Print command failures as structured JSON on stderr |
| `NO_COLOR` | Disable colored output (terminal/toolchain dependent) |

---

## 8. Error handling and exit codes

### 8.1 Error categories (`GbError`)

- `Auth(String)`
- `Api { status, message }`
- `Config(String)`
- `NotAuthenticated`
- `RepoNotFound`
- `DiffUnavailable { number, cause, message }`
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

Pass global `--json-errors` (or set `GB_JSON_ERRORS=true`) to print failures as a single JSON object on stderr:

```json
{"error":{"code":"diff_unavailable","message":"Diff unavailable for pull request #9: ...","cause":"empty_live_diff","exit_code":1}}
```

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
- `PATCH /repos/{owner}/{repo}/issues/comments/{comment_id}`

Pull request:

- `GET /repos/{owner}/{repo}/pulls?state={state}`
- `GET /repos/{owner}/{repo}/pulls/{number}`
- `POST /repos/{owner}/{repo}/pulls`
- `PATCH /repos/{owner}/{repo}/pulls/{number}` (title/body/state)
- `PUT /repos/{owner}/{repo}/pulls/{number}/merge`
- `GET /repos/{owner}/{repo}/issues/{number}/comments` (PR comments)
- `POST /repos/{owner}/{repo}/issues/{number}/comments` (PR comments)
- `PATCH /repos/{owner}/{repo}/issues/comments/{comment_id}` (PR comment edit)
- `POST /{owner}/{repo}/issues/{number}/assignee/{new|delete}` (PR assignee Web operation)

Label:

- `GET /repos/{owner}/{repo}/issues/{number}/labels`
- `POST /repos/{owner}/{repo}/issues/{number}/labels` (additions only)
- `DELETE /repos/{owner}/{repo}/issues/{number}/labels/{name}` (one association)
- `GET /{owner}/{repo}/issues/labels` (verified Web identity lookup)
- `GET /{owner}/{repo}/issues/labels/{id}/edit` (verified Web form)
- `POST /{owner}/{repo}/issues/labels/{id}/edit` (in-place color update)

- `GET /repos/{owner}/{repo}/labels`
- `GET /repos/{owner}/{repo}/labels/{name}`
- `POST /repos/{owner}/{repo}/labels`
- `PATCH /repos/{owner}/{repo}/labels/{name}`
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

---

## 10. Roadmap

### Phase 1 next

- Keep strengthening test and E2E coverage around the current command set
- Keep `cargo clippy --all-targets --all-features -- -D warnings` green in CI
- Keep release automation and package metadata aligned before tagging

### Phase 2

- Expand issue/PR metadata handling where GitBucket exposes additional compatible APIs

### Phase 3

- Re-evaluate webhook and collaborator operations after the current command set stabilizes

### Phase 4

- Documentation refinements

### Backlog / re-evaluate later

- `gb user`
- `gb webhook`
- `gb repo collaborator`
