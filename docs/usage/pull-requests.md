# Pull requests

Use `gb pr --help` and `gb pr <command> --help` for the options supported by the installed binary. See [repository selection](commands.md#repository-selection) for `-R`, profiles, and Git remote discovery.

## List and view

```sh
gb pr list --state open
gb pr list --state all --json --no-pager
gb pr view 42 --comments
gb pr view 42 --web
```

Lists accept `open`, `closed`, or `all`. JSON lists fetch canonical details for each PR so the result identifies fork heads correctly. Numbered commands such as ordinary view, edit, and discussion require a detail object and check the returned PR number. See [pagination](../reference/automation.md#collection-pagination) and [JSON output](../reference/automation.md#api-json).

## Create

```sh
# From the local feature/report branch
git push -u origin feature/report
gb pr create --title 'Add a report' --body-file report.md
gb pr create --head alice:feature/report --base main --title 'Add a report' --detect-existing
```

The target repository's default branch is used when `--base` is omitted. An inferred head requires a published branch matching the current local commit. An explicit `--head BRANCH` or `--head OWNER:BRANCH` selects published content. `--head-owner` supplies the fork owner separately. The optional `--detect-existing` flag checks canonical open PRs for the same head and base. See [PR creation input and checks](input-and-display.md#pr-creation) and [existing PR detection](../reference/automation.md#canonical-pr-identity).

## Edit, close, and reopen

```sh
gb pr edit 42 --title 'Update the report' --body-file report.md
gb pr edit 42 --add-assignee alice --web
gb pr close 42
gb pr reopen 42
```

Title, body, and state use the PR REST endpoint. Assignee changes require explicit `--web` and verified Web authentication. A combined edit verifies the steps separately and reports partial or uncertain outcomes. See [Web operation routes](../reference/gitbucket-compatibility.md#web-operations).

## Discussion and review comments

```sh
gb pr comment 42 --body 'Please clarify the inputs.'
gb pr comment 42 --edit-last --editor
gb pr comment list 42 --kind all --json
gb pr view 42 --comments --kind review
```

Ordinary discussion is the default. `--edit-last` selects the authenticated user's latest ordinary comment. `--kind review` selects visible inline and commit comments, and `--kind all` selects both sources. See [review output and coverage](../reference/review-comments.md) and [inline create, edit, reply, and delete](../reference/review-writes.md).

## Check out and diff

```sh
gb pr checkout 42
gb pr diff 42 --no-pager
```

Both commands use the PR's saved commit IDs. Checkout requires a clean index and worktree and protects existing branches. Diff uses the saved commits independently of local edits. See [saved commits and fetch behavior](../reference/pull-request-snapshots.md).

## Merge

```sh
gb pr merge 42 --match-head-commit 0123456789abcdef0123456789abcdef01234567
```

Replace the example SHA with the full commit ID you reviewed. GitBucket 4.44.0 permits client-side head checks and readback. The merge is still subject to a race on the server. See [merge checks and review decisions](../reference/merge.md).
