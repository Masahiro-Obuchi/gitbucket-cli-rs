# GitBucket compatibility baseline

The Docker E2E fixture is pinned to GitBucket **4.44.0** in
`docker/e2e/compose.yaml`. This is a verified baseline, not a claim that all
GitBucket releases or authentication plugins behave identically.

`tests/e2e_smoke/api_contract.rs` calls REST and Web endpoints directly. These
tests record server contracts that the CLI must accommodate; they do **not**
assert that the corresponding CLI defects have already been fixed. Keep server
contract tests separate from CLI regression tests and validate stored values,
not just HTTP success or a success message.

## Contracts verified against the pinned server

All REST paths below are relative to `/api/v3/repos/{owner}/{repo}`.

| Operation | Observed server contract | Consequence for the CLI |
| --- | --- | --- |
| Issue and PR list pagination | Fixed 25-row pages, ignores per_page, no Link; Issues excludes PRs | Traverse the PR and Issue collections separately; no Issue-list recovery for PRs |
| Discussion comment collection | Returns all comments even with page=2 and per_page=1 | Do not apply fixed-page probing to comment counts |
| Fork PR ownership | Collection reports the base repository as the fork head; canonical detail reports the actual fork | Verify candidates and JSON metadata with canonical detail |
| Saved PR comparisons | Closed/merged PRs retain base/head SHAs after branch reuse; refs/pull/N/head preserves the saved head | Diff/checkout must use saved object IDs, not current branch tips |
| Deleted fork detail | Detail returns 200 with zero bytes; collection still exposes saved SHAs and base repository retains PR head ref | Snapshot-specific absent-detail recovery can restore verified Git objects without trusting collection head ownership |
| PR response metadata | draft/labels/assignees/merged_by, nullable mergeable, id and resource URLs are returned | Preserve original JSON; expose reported PR metadata without inventing absent values |
| Formal review decisions | Authenticated review GET/APPROVE/REQUEST_CHANGES POST and dismissal PUT return 404 in pinned core, with no discussion writes | No formal decision adapter or ordinary-comment substitute; human decision status stays unknown |
| Merge expected-head condition | A direct merge with an incorrect sha still succeeds; response identity is JGit AnyObjectId text | Do not claim atomic protection by sending sha; client head checks and canonical readback are supplementary |
| Draft merge | Core rejects merging a reported draft; CLI refuses before PUT | Retain draft/readiness state and do not infer approval from comments |
| Issue JSON comment count | The 4.44 Issue response omits a count even when comments exist | Keep absence; add fetched discussions under comment_details without synthesizing comments |
| PR title/body/state | `PATCH /pulls/{number}` updates title/body/state; a subsequent state-only PATCH preserves title/body | Use the PR endpoint, without requiring Web authentication for these fields |
| Issue-style edit of a PR | `PATCH /issues/{number}` returns 404 and leaves the title unchanged | Do not interpret this as proof that PR REST editing is unavailable |
| Inline comments | A Web post to `/{owner}/{repo}/commit/{sha}/comment/new` with file, new line, PR ID and content appears on the PR page | Inline comments require a distinct model and supported retrieval/posting path |
| Inline comment listing | `GET /issues/{number}/comments` returns a separately created ordinary comment but excludes the Web-created inline comment; `GET /pulls/{number}/comments` returns 404 | Issue comments cannot represent the entire PR conversation |
| Milestone state-only PATCH | `PATCH /milestones/{number}` with only state returns 404 | Supply required fields; this response does not imply an absent endpoint |
| Milestone complete PATCH | Title/state/description/due_on updates preserve explicitly supplied values | Merge intended changes with existing values before sending |
| Milestone title-only PATCH | Reopens a closed milestone, empties the description and clears the due date; verified with a subsequent GET | An ordinary partial PATCH is unsafe for this server contract |
| Milestone title syntax | A title containing Japanese text and a space is rejected with 404 | Distinguish input constraints from missing REST support; preserve an appropriate Web path |
| Milestone deletion | `DELETE /milestones/{number}` returns 204 | REST deletion exists |
| Label creation | Name/color persist, but the supplied description is absent/null on a subsequent GET | Do not silently report unsupported description changes as successful |
| Label incomplete PATCH | `PATCH /labels/{name}` with only color or only name returns 404 | The endpoint expects both name and color |
| Label same-name PATCH | Supplying the existing name and a new color returns 422; the old color remains | Merely filling in missing fields does not solve color editing on 4.44.0 |
| Label rename PATCH | Supplying a fresh name and color succeeds; GET finds the new values at the new name and returns 404 for the old name | The REST update route exists, despite the same-name constraint |

PR-06 additionally verifies that the numeric-ID Web edit updates color without
changing label identity or Issue associations. Its label-only Issue edits use the
existing dedicated add/remove APIs without Web credentials. The scoped HTML
parser is covered by sanitized row/form fixtures in `tests/fixtures/`, including
root-relative links from the `/gitbucket` proxy. Description writes now fail
before mutation; [upstream requirements](UPSTREAM_LABEL_API.md) remain explicit.

These contracts were exercised in both root and `/gitbucket` proxy deployments.
The tests create disposable PRs and an inline comment, and create/delete their
own milestones and labels. Run them only against the bootstrapped test instance,
never against a production repository. Credentials and session cookies are not
written into committed fixtures or assertion messages intentionally.

PR-09 verifies the collection contracts at 0/1/25/26/50/51 rows with an isolated
repository, including all-return comments, open/closed/all, an oldest PR beyond
two pages, edit-last and a real root-owned fork. The CLI checks accompany direct
REST assertions so summary metadata cannot conceal the fork mismatch.
See [collection contracts and limits](PAGINATION.md).

PR-10 adds live closed/merged branch-reuse and deleted-fork contracts. Snapshot
recovery is scoped to exact saved SHAs and retained objects; the 200/empty detail
exception does not relax ordinary typed JSON reads. See [PR snapshots](PR_SNAPSHOTS.md).

PR-11 compares original REST resources with CLI JSON for an Issue and real draft
PR, including comment-array augmentation, at both deployment paths. This fixes
response-field loss without claiming that all advertised URLs work or that the
ordinary discussion collection includes inline comments. See [JSON contracts](JSON_OUTPUT.md).

PR-12 adds a DOM reader for the verified conversation layout, with separate
review records and explicit visible-scope coverage. Live cases cover rename,
deleted files, old/new lines, grouped comments, multiple authors, outdated
positions and history replacement. Public reads are password-free; private Web
reads deny the REST token with 401 and require Cookie authentication. Persisted
association/history and raw Markdown are not all exposed by this server; see
[review contracts](REVIEW_COMMENTS.md) and [upstream needs](UPSTREAM_REVIEW_API.md).

PR-13 verifies inline AJAX create/edit/delete and permission-gated raw Markdown
reads in both layouts. CLI end-to-end contracts exercise create/retry reuse,
edit, position-group reply and deletion; rename new/context lines and deleted
old lines are compared with saved Git positions and Web readback. Ordinary
comments remain on REST. The CLI reserves create/reply intents locally; the
server supplies neither an atomic head guard nor an idempotency key. See
[write contracts and remaining limits](REVIEW_WRITES.md).

## Fixture strategy

The PR fixture uses Git and the CLI to create an actual branch, commit and PR,
then posts an ordinary comment through REST and an inline comment through Web
on the new file's first line. The ordinary comment is a positive control: an
empty or malformed Issue comments listing must fail the contract. Milestone and
label fixtures use unique names and read back their initial and updated values.
Rejected milestone edits are also checked for unintended changes, and deletions
are followed by GET requests proving the resource is gone. These fixtures are
generated afresh instead of committing an
entire authenticated HTML page, runtime credentials, or server-specific IDs.
This makes the contracts repeatable across both deployment paths.

Before extending HTML parsing, capture and sanitize focused fragments. PR-10
covers fork removal and saved SHAs; PR-12 covers old-side, rename/deleted-file,
position-group and outdated Web reads. Explicit reply relationships and complete
stored review history still need server support and dedicated contracts. The
present tests establish a first baseline; they do not finish the entire review
compatibility matrix.

## Completed adapter contracts and remaining unknowns

The dedicated Issue-label delta, PR content and Web-assignee contracts now cover
the earlier source observations; see [Web operations](WEB_OPERATIONS.md),
the pinned `api_contract.rs` cases and their corresponding CLI regressions.
PR-17 adds [review-decision and merge contracts](REVIEW_MERGE.md), including
actual ignored-sha and draft behavior. These do not establish compatibility
with untested versions/extensions, formal decisions, atomic merges, or complete
stored review history. Source observations alone remain insufficient evidence
for another server's behavior.

Relevant pinned upstream sources:

- [PR API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiPullRequestControllerBase.scala)
- [Issue API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueControllerBase.scala)
- [Issue comment API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueCommentControllerBase.scala)
- [Milestone API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueMilestoneControllerBase.scala)
- [Milestone input](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/CreateAMilestone.scala)
- [Label API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueLabelControllerBase.scala)
- [Web commit comments](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/RepositoryViewerController.scala)

## Refactor-series status

PR-00–17 are merged through [PR #56](https://github.com/Masahiro-Obuchi/gitbucket-cli-rs/pull/56).
Later tests cover saved positions/history limits, private reads, inline writes,
profile/migration behavior, explicit input and merge checks. The original numbers
below remain the initial baseline's record. See [the status and remaining-work
register](REFACTOR_STATUS.md) and [TESTING.md](../TESTING.md) for later evidence,
upgrade changes and unverified deployment/server requirements.

## Validation record for the initial baseline

- Existing root-deployment CLI E2E: 19 passed.
- New root-deployment contract tests: 3 passed.
- Subpath-deployment CLI and contract E2E together: 22 passed.
- Regular unit/integration tests: 246 passed; 22 Docker tests ignored by default.
- Locked check, Clippy with warnings denied, and formatting check: passed.

This record describes the initial PR's local validation, not an automatically
updated compatibility report. When the pinned image changes, revalidate and
update the contract expectations deliberately.
