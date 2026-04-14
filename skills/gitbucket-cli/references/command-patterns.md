# Command Patterns

Use these examples as starting points. Check `gb <command> --help` before using less common flags.

## Authentication

```bash
gb auth status
gb auth login -H gitbucket.example.com
gb auth login -H https://gitbucket.example.com/gitbucket
gb auth token -H gitbucket.example.com
```

## Repository Resolution

```bash
gb repo view -R alice/my-app
gb repo view -H https://gitbucket.example.com/gitbucket -R alice/my-app
gb repo list alice
gb repo clone alice/my-app
```

When `-R` is omitted, `gb` can infer the repository from `git remote get-url origin` for supported GitBucket HTTPS, SSH, and `/git/` clone URL forms.

## Issue Workflows

```bash
gb issue list --state open
gb issue view 12 --comments
gb issue create -t "Bug report" -b "Steps to reproduce..."
gb issue edit 12 --add-label bug --add-assignee alice
gb issue comment 12 -b "I reproduced this on the staging instance."
gb issue close 12
gb issue reopen 12
```

Issue label and assignee options may be repeated or comma-separated when supported by the command.

## Pull Request Workflows

```bash
gb pr list --state open
gb pr view 5
gb pr diff 5
gb pr checkout 5
gb pr create -t "Add feature X" --head feature/x -B main
gb pr comment 5 -b "Looks ready from my side."
gb pr merge 5
```

## Label And Milestone Workflows

```bash
gb label list
gb label create bug --color fc2929 --description "Broken behavior"
gb label view bug

gb milestone list --state all
gb milestone create v1.0 --description "First release" --due-on 2026-04-01
gb milestone edit 1 --state closed
```

## Raw API Calls

```bash
gb api /user
gb api /repos/alice/my-app/issues
```

Prefer `gb api` for GitBucket REST endpoints that do not yet have a dedicated top-level command.
