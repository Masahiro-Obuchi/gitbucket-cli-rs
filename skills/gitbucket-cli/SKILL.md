---
name: gitbucket-cli
description: Use when operating GitBucket repositories, issues, pull requests, labels, milestones, or raw GitBucket API calls with the gb command-line tool. Prefer this skill over GitHub CLI workflows when the target host or repository is a GitBucket instance.
---

# GitBucket CLI

Use `gb` for GitBucket work. Do not assume `gh` can operate against GitBucket.

## Before Running Commands

1. Confirm the target is GitBucket.
2. Resolve the host from `--hostname`, `GB_HOST`, or saved config (run `gb auth login` if none is set).
3. Resolve the repository from `--repo`, `GB_REPO`, or `git remote get-url origin`.
4. Preserve path-prefixed base URLs such as `https://gitbucket.example.com/gitbucket`.
5. Prefer `gb <command> --help` for exact options before constructing uncommon commands.

## Command Families

- Auth: `gb auth login`, `gb auth logout`, `gb auth status`, `gb auth token`
- Config: `gb config path`, `gb config list`, `gb config get`, `gb config set`
- Repositories: `gb repo list`, `gb repo view`, `gb repo create`, `gb repo clone`, `gb repo delete`, `gb repo fork`
- Issues: `gb issue list`, `gb issue view`, `gb issue create`, `gb issue edit`, `gb issue close`, `gb issue reopen`, `gb issue comment`
- Pull requests: `gb pr list`, `gb pr view`, `gb pr create`, `gb pr close`, `gb pr merge`, `gb pr checkout`, `gb pr diff`, `gb pr comment`
- Labels: `gb label list`, `gb label view`, `gb label create`, `gb label delete`
- Milestones: `gb milestone list`, `gb milestone view`, `gb milestone create`, `gb milestone edit`, `gb milestone delete`
- Direct API calls: `gb api <ENDPOINT>`
- Browser navigation: `gb browse`

## Output And Parsing

Use table output for human-facing summaries. Use `--json` when the next step needs structured parsing.

Use `--state open`, `--state closed`, or `--state all` for issue, pull request, and milestone list commands when the desired state matters.

## Web Fallbacks

Some GitBucket actions are not fully covered by the REST API on every server version. When `gb` falls back to the web UI, it may prompt for a password. If automation needs those prompts to be non-interactive, use `GB_USER` and `GB_PASSWORD` only when the user has explicitly provided or approved them.

## References

Read `references/command-patterns.md` when you need concrete command examples for common GitBucket workflows.

For full user-facing details, prefer the repository `README.md`, `SPEC.md`, and live CLI help over copying command reference material into this skill.
