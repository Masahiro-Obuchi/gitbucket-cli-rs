# GitBucket command patterns

The examples use POSIX shell syntax. Substitute the confirmed instance URL, repository, and profile. Use `--profile` when the user selected a profile. For scripts, capture the exit status, stdout, and stderr separately.

## Verify the target

```sh
host='https://bucket.example/gitbucket'
repo='alice/project'
profile='work'

gb -H "$host" --profile "$profile" --json-errors auth status --json
gb -H "$host" -R "$repo" --profile "$profile" --json-errors issue view 7 --json --no-pager
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr view 42 --json --no-pager
```

If the requested profile is unclear, `gb config list --json` can show saved defaults without printing credentials. It lists the full inventory. Report the entries relevant to the requested target. Check status online before describing any saved username as the verified actor.

For an issue change, read the issue and any referenced labels, assignees, or milestone first. Once the requested effect is authorized, use the narrow operation. For example, `issue edit 7 --add-label triage` adds one existing label while leaving unrelated labels in place. The readback is `issue view 7 --json --no-pager` on the same target.

## Create a PR from a published branch

Verify the base repository and the published head selected by the user. An explicit `--head OWNER:BRANCH` selects a fork branch on the server. To use the current local branch, omit `--head` and let `gb` verify that the local commit matches the published ref. Omit `--base` to use the target repository's default branch. Keep any Git push within the authorized task.

After the user authorizes this PR, run the create command with its actual title and Markdown body:

```sh
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr create \
  --head alice:feature/report --title 'Add the report' \
  --body-file report.md --detect-existing --json
```

`--detect-existing` can return a matching open PR. It uses canonical head/base identity and complete pagination. Treat an incomplete lookup as an error. If a create result is uncertain, look for the exact open head/base PR before another submission. Use the verified PR number and URL when reporting the result.

## Read a PR and its review conversation

```sh
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr comment list 42 \
  --kind all --json --no-pager
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr diff 42 --no-pager
```

The comment list object has `comments` for ordinary discussion, `review_comments` for the visible Web conversation, and `comment_coverage` describing what was requested and read. In `pr view --comments --json`, fetched discussion appears in `comment_details`. The original API `comments` field retains its own value. A successful visible-conversation read can leave historical completeness, original positions, and original Markdown unknown. Explain these limits when they affect the requested review. If a private review cannot be read with the available authorized credentials, report the missing coverage.

`pr diff` prints a raw patch against the saved base and head commits. It requires a local Git repository and may fetch and retain Git objects. A private HTTP(S) Git fetch with `GB_TOKEN` also needs the matching `GB_USER`. SSH uses the local Git credentials. Capture large patches directly to a file. `pr checkout` changes local workspace state and belongs to an authorized checkout task.

## Post an inline review comment

Use this only when the user wants an inline comment at an identifiable position. Read the saved `base.sha` and `head.sha`, inspect the saved diff, and select the path and old or new **file** line. Check the saved pair again before writing. If it changed, inspect the new comparison and confirm the intended position before submitting. For renames use the destination path. For deleted files use the original path and old side. If the saved comparison or position is unavailable, report that blocker.

After the position and body are authorized and the verified Web credentials are available:

```sh
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr comment 42 \
  --file src/report.rs --line 18 --side new --commit "$reviewed_head" \
  --body-file review.md --request-id review-42-input-check --web --json
```

Use `--commit` to pin the reviewed head in the client's checks. A GitBucket server without atomic change guards can still race with a push. The result includes the comment's qualified identity and verification status.

To edit or reply, derive the ID from the visible inline record and select `--kind inline`. Do not use an ordinary discussion ID with an inline command. A reply targets the current position group. It does not establish a persistent parent relationship. Ask before converting a requested inline comment into a general discussion comment.

## Reconcile an uncertain write

- **Issue or PR edit:** Read the same target again and compare the specific requested fields. For a partial result, identify each confirmed, failed, or unverified step before considering another mutation.
- **Ordinary comment:** List ordinary comments on the same resource and compare the verified author and the new record. If several records fit, report the ambiguity. In a PR, `pr comment list NUMBER --kind discussion --json` provides the ordinary discussion records. Use `--edit-last` only when the intended target is the authenticated actor's latest ordinary comment.
- **Inline create or reply:** Inspect the visible review conversation on the same PR. Use the original `--request-id` and the same intent to let `gb` reconcile an uncertain submission against its private retry record. Preserve that record and the target and position it describes. If reconciliation remains unverified or the recorded comparison changed, stop and report it.
- **PR creation:** Look for a matching open PR using canonical head owner, branch, and base. A missing result from a failed or incomplete list is inconclusive.

## Read through the raw API

```sh
gb -H "$host" --profile "$profile" --json-errors api --method GET "repos/$repo"
```

Relative endpoints resolve under the instance's `/api/v3`. The raw command returns an endpoint's own JSON and pagination behavior. Prefer the dedicated collection commands for complete issue and PR lists. Sending `--input FILE` or `--input -` supplies JSON and defaults the method to POST. For an authorized mutation, use only the approved method, exact endpoint, and payload.
