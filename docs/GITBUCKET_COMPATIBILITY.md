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
| PR title/body/state | `PATCH /pulls/{number}` updates title/body/state; a subsequent state-only PATCH preserves title/body | Use the PR endpoint, without requiring Web authentication for these fields |
| Issue-style edit of a PR | `PATCH /issues/{number}` returns 404 and leaves the title unchanged | Do not interpret this as proof that PR REST editing is unavailable |
| Inline comments | A Web post to `/{owner}/{repo}/commit/{sha}/comment/new` with file, new line, PR ID and content appears on the PR page | Inline comments require a distinct model and supported retrieval/posting path |
| Inline comment listing | The Web-created inline comment is absent from `GET /issues/{number}/comments`; `GET /pulls/{number}/comments` returns 404 | Issue comments cannot represent the entire PR conversation |
| Milestone state-only PATCH | `PATCH /milestones/{number}` with only state returns 404 | Supply required fields; this response does not imply an absent endpoint |
| Milestone complete PATCH | Title/state/description/due_on updates preserve explicitly supplied values | Merge intended changes with existing values before sending |
| Milestone title-only PATCH | Reopens a closed milestone and replaces the previous description/due date | An ordinary partial PATCH is unsafe for this server contract |
| Milestone title syntax | A title containing Japanese text and a space is rejected with 404 | Distinguish input constraints from missing REST support; preserve an appropriate Web path |
| Milestone deletion | `DELETE /milestones/{number}` returns 204 | REST deletion exists |
| Label creation | Name/color work, but the supplied description is not returned as a stored value | Do not silently report unsupported description changes as successful |
| Label color-only PATCH | `PATCH /labels/{name}` without name returns 404 | The endpoint expects both name and color |
| Label same-name PATCH | Supplying the existing name and a new color returns 422; the old color remains | Merely filling in missing fields does not solve color editing on 4.44.0 |

These contracts were exercised in both root and `/gitbucket` proxy deployments.
The tests create disposable PRs and an inline comment, and create/delete their
own milestones and labels. Run them only against the bootstrapped test instance,
never against a production repository. Credentials and session cookies are not
written into committed fixtures or assertion messages intentionally.

## Fixture strategy

The PR fixture creates an actual branch and commit, then adds an inline comment
on the new file's first line. Milestone and label fixtures use unique names and
read back their values. They are generated afresh instead of committing an
entire authenticated HTML page, runtime credentials, or server-specific IDs.
This makes the contracts repeatable across both deployment paths.

Before adding an HTML parser, capture and sanitize focused HTML fragments from
these fixtures. Old-side comments, renamed/deleted files, replies, outdated
comments after a head update, fork removal, and persisted SHA retrieval still
need dedicated fixtures. The present tests establish a first baseline; they do
not finish the entire review compatibility matrix.

## Source-confirmed items awaiting dedicated live contracts

The review also identified APIs for Issue label add/remove/replace, missing
Issue PATCH support, and differences between PR metadata and assignee updates.
Those observations still require dedicated live regression cases as the relevant
fixes are implemented. Do not turn a source observation into a verified CLI
compatibility claim without testing its result.

Relevant pinned upstream sources:

- [PR API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiPullRequestControllerBase.scala)
- [Issue API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueControllerBase.scala)
- [Issue comment API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueCommentControllerBase.scala)
- [Milestone API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueMilestoneControllerBase.scala)
- [Milestone input](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/CreateAMilestone.scala)
- [Label API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueLabelControllerBase.scala)
- [Web commit comments](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/RepositoryViewerController.scala)

## Validation record for the initial baseline

- Existing root-deployment CLI E2E: 19 passed.
- New root-deployment contract tests: 3 passed.
- Subpath-deployment CLI and contract E2E together: 22 passed.
- Regular unit/integration tests: 246 passed; 22 Docker tests ignored by default.
- Locked check, Clippy with warnings denied, and formatting check: passed.

This record describes the initial PR's local validation, not an automatically
updated compatibility report. When the pinned image changes, revalidate and
update the contract expectations deliberately.
