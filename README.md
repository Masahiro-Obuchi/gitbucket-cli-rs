# GitBucket CLI

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
```

## Installation

### Install with Cargo

```bash
cargo install --git https://github.com/Masahiro-Obuchi/gitbucket-cli-rs --locked gb
```

To install a specific version tag:

```bash
cargo install --git https://github.com/Masahiro-Obuchi/gitbucket-cli-rs --tag v0.1.0 --locked gb
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
gb label create bug --color fc2929 --description "Broken behavior"
gb label view bug
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
gb issue close 1
gb issue comment 1 -b "Fixed!"
```

### 6) Pull request operations

```bash
gb pr list --state closed
gb pr create
gb pr view 5
gb pr merge 5
gb pr checkout 5
gb pr diff 5
```

## Command reference

| Command                       | Description                          |
| ----------------------------- | ------------------------------------ |
| `gb auth login`               | Authenticate to a GitBucket instance |
| `gb auth logout`              | Remove auth for a host               |
| `gb auth status`              | Show current auth status             |
| `gb auth token`               | Print access token                   |
| `gb config`                   | Manage local CLI configuration       |
| `gb api <ENDPOINT>`           | Call the GitBucket REST API directly |
| `gb repo list [OWNER]`        | List repositories                    |
| `gb repo view [OWNER/REPO]`   | Show repository details              |
| `gb repo create [NAME]`       | Create a repository                  |
| `gb repo clone <REPO>`        | Clone a repository                   |
| `gb repo delete [OWNER/REPO]` | Delete a repository                  |
| `gb repo fork [OWNER/REPO]`   | Fork a repository                    |
| `gb label list`               | List labels                          |
| `gb label view <NAME>`        | Show label details                   |
| `gb label create [NAME]`      | Create a label                       |
| `gb label delete <NAME>`      | Delete a label                       |
| `gb milestone list`           | List milestones                      |
| `gb milestone view <NUMBER>`  | Show milestone details               |
| `gb milestone create [TITLE]` | Create a milestone                   |
| `gb milestone edit <NUMBER>`  | Edit a milestone                     |
| `gb milestone delete <NUMBER>` | Delete a milestone                   |
| `gb issue list`               | List issues                          |
| `gb issue view <NUMBER>`      | Show issue details                   |
| `gb issue create`             | Create an issue                      |
| `gb issue close <NUMBER>`     | Close an issue                       |
| `gb issue reopen <NUMBER>`    | Reopen an issue                      |
| `gb issue comment <NUMBER>`   | Add an issue comment                 |
| `gb pr list`                  | List pull requests                   |
| `gb pr view <NUMBER>`         | Show PR details                      |
| `gb pr create`                | Create a pull request                |
| `gb pr close <NUMBER>`        | Close a pull request                 |
| `gb pr merge <NUMBER>`        | Merge a pull request                 |
| `gb pr checkout <NUMBER>`     | Checkout a PR branch                 |
| `gb pr diff <NUMBER>`         | Show PR diff                         |
| `gb pr comment <NUMBER>`      | Add a PR comment                     |
| `gb browse`                   | Open repository in browser           |

## Global options

```text
-H, --hostname <HOST>    GitBucket host or base URL
-R, --repo <OWNER/REPO>  Target repository
-h, --help               Show help
-V, --version            Show version
```

## Repository auto-resolution

If `-R/--repo` is omitted, `gb` tries to detect `OWNER/REPO` from `git remote get-url origin`. For `gb repo view`, `gb repo delete`, and `gb repo fork`, you can also pass `OWNER/REPO` positionally after the subcommand.

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

## Output formats

```bash
gb issue list
gb issue list --json
gb issue view 1 --web
```

## State Filters

`gb issue list` and `gb pr list` support `--state open`, `--state closed`, and `--state all`.
Invalid values are rejected before the API call is made.

## GitBucket Web Fallbacks

Some GitBucket actions are only exposed through the web UI, not the REST API.
When `gb repo delete`, `gb repo fork`, `gb issue close`, or `gb issue reopen` hits that case, `gb` falls back to a short web sign-in flow and may prompt for your password.
Use `GB_USER` and `GB_PASSWORD` to preseed those prompts when needed.

## Pull Request Checkout And Diff

`gb pr checkout` and `gb pr diff` work with same-repository PRs and fork-based PRs.
When the PR head repository is available from the API response, `gb` fetches from that repository's clone URL and operates on `FETCH_HEAD`.

## Environment variables

| Variable        | Description                                               |
| --------------- | --------------------------------------------------------- |
| `GB_TOKEN`      | Access token (takes precedence over file config for auth) |
| `GB_HOST`       | Default GitBucket host or base URL                        |
| `GB_REPO`       | Default repository (`OWNER/REPO`)                         |
| `GB_CONFIG_DIR` | Config directory path (default: `~/.config/gb/`)          |
| `GB_PROTOCOL`   | Protocol override when using `GB_TOKEN` with a plain host |
| `GB_USER`       | Username for GitBucket web-session fallbacks              |
| `GB_PASSWORD`   | Password for GitBucket web-session fallbacks              |
| `NO_COLOR`      | Disable colored output                                    |

## Configuration file

Credentials are stored in `~/.config/gb/config.toml` (or under `GB_CONFIG_DIR`).
On Unix-like systems, the file is written with `0600` permissions.

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

When `GB_TOKEN` is set, protocol selection uses this order:

1. URL scheme from `--hostname` or `GB_HOST` when present
2. `GB_PROTOCOL`
3. Matching stored host configuration
4. Default `https`

## Roadmap

Current priority is to deepen the existing command set before adding many new top-level areas.

Near term:

- More test and Docker-backed E2E coverage

After that:

- Richer issue/PR metadata handling

Lower priority / re-evaluate later:

- `gb completion`
- Webhook and collaborator operations
- Admin-oriented user management flows

## Configuration commands

```bash
gb config path
gb config list
gb config get default-host
gb config get host --host gitbucket.example.com/gitbucket --field protocol
gb config set host --host gitbucket.example.com/gitbucket --protocol http --default
gb config unset default-host
```

`gb config` manages the local `config.toml` file. It currently supports:

- printing the config file path
- listing saved hosts and the stored default host
- reading saved host `user` / `protocol` / `has-token` values
- updating saved host `user` / `protocol` values
- setting or clearing `default_host`

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
When `--input` is given and `-X/--method` is omitted, `gb api` defaults to `POST`.
JSON responses are pretty-printed, and empty success responses print `null`.

## Label commands

```bash
gb label list
gb label list --json
gb label view bug
gb label create needs-review --color abcdef --description "Needs extra review"
gb label delete needs-review --yes
```

`gb label` manages repository label definitions through the GitBucket REST API.
It currently supports listing, viewing, creating, and deleting labels in the target repository.
Colors accept 6-digit hex values with or without a leading `#`.

## Milestone commands

```bash
gb milestone list
gb milestone list --state all --json
gb milestone view 7
gb milestone create v1.0 --description "First release" --due-on 2026-04-01
gb milestone edit 7 --title v1.0.1 --state closed
gb milestone delete 7 --yes
```

`gb milestone` manages repository milestones.
It currently supports listing, viewing, creating, editing, and deleting milestones in the target repository.
`--due-on` accepts `YYYY-MM-DD` or RFC3339. When GitBucket returns `404` for milestone create or edit over REST, `gb` falls back to the GitBucket web UI flow and may prompt for `GB_USER` / `GB_PASSWORD`.

## Testing

For the current automated test layout and recommended commands, see [TESTING.md](./TESTING.md).

## Acknowledgements

This project exists thanks to [GitBucket](https://gitbucket.github.io/) and the community around it. Thank you for building and maintaining the software that made this CLI worth creating.

## License

MIT
