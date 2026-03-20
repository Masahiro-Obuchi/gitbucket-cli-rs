# GitBucket CLI

`gb` is a command-line interface for GitBucket.
It is inspired by [GitHub CLI (`gh`)](https://cli.github.com/) and implemented in Rust.

```bash
$ gb issue list
#    STATE   TITLE                          AUTHOR   LABELS
#1   OPEN    Fix login page bug             alice    bug
#3   OPEN    Add dark mode support          bob      enhancement

$ gb pr create -t "Add feature X" -H feature/x -B main
✓ Created pull request #5: Add feature X
```

## Installation

### Build from source

```bash
git clone https://github.com/your-org/gitbucket-cli-rs.git
cd gitbucket-cli-rs
cargo build --release
cp target/release/gb ~/.local/bin/
```

**Requirements:** Rust 1.70+ and `git`

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
gb auth login -H https://localhost/gitbucket
# or
gb auth login -H localhost/gitbucket --protocol https
```

### 2) Repository operations

```bash
gb repo list
gb repo view alice/my-app
gb repo create my-new-repo
gb repo clone alice/my-app
gb repo fork alice/my-app
```

### 3) Issue operations

```bash
gb issue list
gb issue create -t "Bug report"
gb issue view 1
gb issue close 1
gb issue comment 1 -b "Fixed!"
```

### 4) Pull request operations

```bash
gb pr list
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
| `gb repo list [OWNER]`        | List repositories                    |
| `gb repo view [OWNER/REPO]`   | Show repository details              |
| `gb repo create [NAME]`       | Create a repository                  |
| `gb repo clone <REPO>`        | Clone a repository                   |
| `gb repo delete [OWNER/REPO]` | Delete a repository                  |
| `gb repo fork [OWNER/REPO]`   | Fork a repository                    |
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

If `-R/--repo` is omitted, `gb` tries to detect `OWNER/REPO` from `git remote get-url origin`.

```bash
cd ~/projects/my-app
gb issue list
```

Supported remote URL formats include:

- `https://gitbucket.example.com/alice/my-app.git`
- `https://gitbucket.example.com/git/alice/my-app.git`
- `git@gitbucket.example.com:alice/my-app.git`

## Output formats

```bash
gb issue list
gb issue list --json
gb issue view 1 --web
```

## Environment variables

| Variable        | Description                                               |
| --------------- | --------------------------------------------------------- |
| `GB_TOKEN`      | Access token (takes precedence over file config for auth) |
| `GB_HOST`       | Default GitBucket host or base URL                        |
| `GB_REPO`       | Default repository (`OWNER/REPO`)                         |
| `GB_CONFIG_DIR` | Config directory path (default: `~/.config/gb/`)          |
| `NO_COLOR`      | Disable colored output                                    |

## Configuration file

Credentials are stored in `~/.config/gb/config.toml` (or under `GB_CONFIG_DIR`).

```toml
[hosts."gitbucket.example.com"]
token = "your-personal-access-token"
user = "alice"
protocol = "https"
```

Subpath deployments can also be stored:

```toml
[hosts."https://localhost/gitbucket"]
token = "your-personal-access-token"
user = "alice"
protocol = "https"
```

## License

MIT
