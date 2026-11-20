# GitBucket command patterns

The examples use POSIX shell syntax. Substitute the confirmed instance URL, repository, and profile. Use `--profile` when the user selected a profile. For scripts, capture the exit status, stdout, and stderr separately.

## Verify the target

Check credential scope before the online probe. Use the same host, repository, profile, and credential environment for the operation and its readback.

```sh
host='https://bucket.example/gitbucket'
repo='alice/project'
profile='work'

gb -H "$host" --profile "$profile" --json-errors auth status --json
gb -H "$host" -R "$repo" --profile "$profile" --json-errors issue view 7 --json --no-pager
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr view 42 --json --no-pager
```

If the requested profile is unclear, `gb config list --json` can show saved defaults without printing credentials. It lists the full inventory. Report the entries relevant to the requested target.

For an issue change, read the issue and any referenced labels, assignees, or milestone first. A non-null `pull_request` field in the Issue API response identifies a PR. If the resource type is unclear, verify it before changing it. Once the requested effect is authorized, use the narrow operation. For example, `issue edit 7 --add-label triage` adds one existing label while leaving unrelated labels in place. The readback is `issue view 7 --json --no-pager` on the same target.

## Create a PR from a published branch

Verify the base repository and the published head selected by the user. An explicit `--head BRANCH` selects a published branch in the base repository. `--head OWNER:BRANCH` selects a published branch in the named owner's repository of the same name, which can be a fork. To use the current local branch, omit `--head` and let `gb` verify that the local commit matches the published ref. Omit `--base` to use the target repository's default branch. Keep any Git push within the authorized task.

Use `--detect-existing` if reusing an open PR with the same refs satisfies the request. After authorization, supply the actual title and Markdown body.

```sh
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr create \
  --head feature/report --title 'Add the report' \
  --body-file report.md --detect-existing --json
```

`--detect-existing` checks canonical head/base identity through complete pagination. It can return an existing PR with a different title or body, and the JSON object does not distinguish creation from reuse. Verify any requested fields and report the PR number and URL without assuming a new PR was created.

## Read a PR and its review conversation

```sh
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr comment list 42 \
  --json --no-pager
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr diff 42 --no-pager
```

The comment list object has `comments` for ordinary discussion, `review_comments` for the visible Web conversation, and `comment_coverage` describing what was requested and read. In `pr view --comments --json`, fetched discussion appears in `comment_details`. The original API `comments` field retains its own value. A successful visible-conversation read can leave historical completeness, original positions, and original Markdown unknown. Explain these limits when they affect the requested review. An empty array represents zero results. If coverage is `unavailable`, report the missing source and its reason.

`pr diff` prints a raw patch against the saved base and head commits. It requires a local Git repository and may fetch and retain Git objects. A private HTTP(S) Git fetch with `GB_TOKEN` also needs the matching `GB_USER`. SSH uses the local Git credentials. Capture large patches directly to a file. `pr checkout` changes local workspace state and belongs to an authorized checkout task.

## Post an inline review comment

Use this only when the user wants an inline comment at an identifiable position. Read the saved `base.sha` and `head.sha`, inspect the saved diff, and select the path and old or new **file** line. Check the saved pair again before writing. If it changed, inspect the new comparison and confirm the intended position before submitting. For renames use the destination path. For deleted files use the original path and old side. If the saved comparison or position is unavailable, report that blocker.

Set `reviewed_head` to the full head SHA you reviewed and choose a distinct `request_id` for this submission. After authorization, use the verified Web credentials.

```sh
gb -H "$host" -R "$repo" --profile "$profile" --json-errors pr comment 42 \
  --file src/report.rs --line 18 --side new --commit "$reviewed_head" \
  --body-file review.md --request-id "$request_id" --web --json
```

Use `--commit` to pin the reviewed head in the client's checks. A GitBucket server without atomic change guards can still race with a push. The result includes the comment's qualified identity and verification status.

To edit or reply, use the ID from a current, visible inline record with `--kind inline`. An edit changes that record in every view where it appears. A reply adds a comment to the current position group without a stored parent ID. Ask before converting a requested inline comment into a general discussion comment.

## Reconcile an uncertain write

- **Issue or PR edit.** Read the same target again and compare the specific requested fields. For a partial result, identify each confirmed, failed, or unverified step before considering another mutation.
- **Ordinary comment.** Capture the existing comment IDs before posting when you need to reconcile an uncertain result. For readback, use `issue view NUMBER --comments --json --no-pager` (`comment_details`) or `pr comment list NUMBER --kind discussion --json --no-pager` (`comments`). Look for a new ID with the verified author and submitted body. An older match, ambiguous readback, or incomplete list leaves the outcome unresolved. Use `--edit-last` only when the intended target is the authenticated actor's latest ordinary comment.
- **Inline create or reply.** Inspect the visible review conversation on the same PR. Re-run for reconciliation only if the original receipt remains available under `review-writes/` in the same effective config directory. Keep the same instance, repository, PR, verified Web actor, request ID, comparison, position, body, and reply target. A different actor, config directory, or missing receipt can make the command submit again. If the comparison changed or the outcome remains unverified, stop and report it.
- **PR creation.** Use `pr list --state open --json --no-pager` on the same repository and compare the canonical head repository and both originally resolved branch refs. A failed or incomplete list cannot rule out a matching PR.

## Read through the raw API

```sh
gb -H "$host" --profile "$profile" --json-errors api --method GET "repos/$repo"
```

Relative endpoints resolve under the instance's `/api/v3`. The raw command returns an endpoint's own JSON and pagination behavior. Prefer the dedicated collection commands for complete issue and PR lists. Sending `--input FILE` or `--input -` supplies JSON and defaults the method to POST. For an authorized mutation, use only the approved method, exact endpoint, and payload.
