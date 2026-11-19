# GitBucket compatibility

The Docker E2E fixture verifies GitBucket **4.44.0 core** at the site root and under `/gitbucket`. Use [the testing guide](../maintainers/testing.md#live-gitbucket-tests) to reproduce the server checks. Validate other server builds, plugins, proxy paths, authentication methods, Git transports, and certificate setups on their intended deployment.

## Verified server behavior

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
| PR title/body/state | `PATCH /pulls/{number}` updates title/body/state. Body-only PATCH is ignored and title-only PATCH clears the body. State-only PATCH preserves title and body. | Send title and body together for content edits. State-only requests use REST. |
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

## Web operations

The CLI keeps operation-specific adapters and a shared progress/result model. The API and Web clients handle transport and forms; command handlers plan intent, preflight known constraints, invoke the adapter and verify the stored effect. `src/operation.rs` records confirmation without assuming that earlier writes can be rolled back. It preserves underlying error codes/statuses in JSON failures.

These are confirmed GitBucket **4.44.0** differences, not promises about every server version. REST routes of unknown capability are tried once; explicit known rejections can select the corresponding Web adapter. Authentication failures, 5xx, network failures and parse failures never trigger a blind Web write retry.

| Operation | Confirmed REST support / restriction | Web route and confirmation |
| --- | --- | --- |
| PR title/body/state | Supported by PR PATCH. Content edits send the title and body together. | No Web retry, even with `--web`. Validate returned fields. |
| PR assignees | Not writable through the verified PR PATCH. | Explicit `--web`; authenticate before REST fields in mixed edits. Verify each delta through Issue GET. |
| Issue labels | Dedicated additive POST and individual DELETE supported. | No Web. Check additions exist before metadata, preserve unrelated labels, verify each mutation. |
| Issue metadata/state | Issue PATCH absent in 4.44; behavior on other instances remains unknown until requested. | Initially readable target required before a 404 fallback. Preflight forms and referenced targets, then verify fields after each form. Lifecycle never substitutes comment creation. |
| Milestones | REST supported; 4.44 restricts titles to a narrow ASCII set. Replacement requests require all retained fields. | Only the confirmed title restriction permits supplementation. Preserve/reject date/whitespace constraints before writes, verify content before changing state. |
| Repository label color | Same-name REST PATCH rejected with 422 in 4.44. Rename supported with complete name/color. | Only explicit `--web` after that rejection. Prepare and validate the numeric-ID form first; verify unchanged ID and stored color. |
| Label descriptions | Verified REST/Web routes do not persist them. | Unsupported set/clear fails before any mutation. |
| Repository fork | Route absent in 4.44; other compatible servers can support it. `--group` uses REST `organization`. | A readable source and nonconflicting destination are required. Verify destination and immediate parent; poll reads only for asynchronous creation. |
| Repository delete | Route absent in 4.44; other compatible servers can support it. | Read target before deletion, require a confirming 404 after deletion. A missing initial target never causes Web deletion. |

4.44's repository API omits `fork`, `parent` and `source`. This is **unknown fork metadata**, not evidence that a repository is not a fork. When required, the strict header adapter checks both the destination link and its `forked from` link, which renders the immediate parent. Root-relative proxy links are resolved within the configured deployment and must match the exact expected repository, including when an owner matches the deployment prefix. Public verification requests use an absolute deployment URL and need no password; Web writes and private cookie-session reads use the verified Web session. Unknown layouts, inaccessible proof and inconsistent destinations remain errors.

Web HTTP 200 only establishes that a response was received. Verification errors are `unverified`, with no successful output. Multi-step errors list confirmed, rejected, unverified and unattempted steps. A confirmed earlier step remains recorded when a later request fails; neither automatic rollback nor write retry is performed. Concurrent writers and later permission changes can invalidate a snapshot after confirmation; these operations are not transactions.

Web principal verification requires password login and a cookie-authenticated `/api/v3/user`. The default Web user must equal the REST token user. An explicit `GB_WEB_USER` authorizes a different Web actor; notices show both actors and JSON operation records include the verified Web actor. Interactive SSO/OIDC is not implemented. Redirects stay within the selected origin and deployment. This boundary check includes repeated percent decoding, so encoded traversal cannot forward sign-in forms or cookies outside the selected deployment. Mutation outcomes use that request's redirect history, preserving uncertainty when a redirect returns to the original URL and avoiding false redirects for Unicode URL encoding. Cookie state remains shared with the verified session.

Primary contracts:

- [ApiUserControllerBase](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiUserControllerBase.scala) and [IndexController](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/IndexController.scala): cookie principal and password/SSO sign-in.
- [IssuesController](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/IssuesController.scala): trimmed forms and state route.
- [AccountController](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/AccountController.scala), [RepositoryCreationService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/RepositoryCreationService.scala), [ApiRepository](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiRepository.scala), [repository menu](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/twirl/gitbucket/core/menu.scala.html): asynchronous fork and parent proof.
- [GitHub's compatible fork request](https://docs.github.com/en/rest/repos/forks#create-a-fork): REST `organization` field; this does not imply that 4.44 implements that route.

REST and Web share the bounded transport policy in `src/limits.rs` and `src/api/transport.rs`. Incomplete responses preserve transport causes and known status, and an ambiguous write reports an unverified outcome without retries. The separate Git supervisor applies its deadline through group/job termination and child reaping. See [deadlines and diagnostics](automation.md#http-and-git-deadlines).


## Label API

### Same-name color updates

The [REST controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueLabelControllerBase.scala) accepts `CreateALabel(name, color)` for PATCH. A missing field causes 404. A complete same-name request returns 422 because its collision check finds the label being edited. A fresh-name PATCH succeeds and updates the existing label ID.

The upstream correction should exclude the current numeric ID from the collision check. Another label with the destination name must still cause a conflict. A future update-specific request type could make name/color independently optional and preserve unspecified values rather than requiring the client to echo them. Tests should prove that color-only, same-name and rename updates retain the same ID and all Issue-label associations, while a conflicting rename changes nothing. Do not implement a color update as delete/create or as a temporary rename.

The [Web controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/LabelsController.scala) already updates by numeric ID and excludes that ID from name collision checks. The CLI uses that in-place route only with `label edit --web` after a same-name REST 422, validates the row and edit form, and checks the ID and stored color afterward. Once REST is corrected, the same client request can succeed without Web credentials.

### Description persistence and capability evidence

The [input type](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/CreateALabel.scala) and [API representation](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiLabel.scala) contain name/color, and the Web form contains labelName/labelColor. A supplied REST description is ignored and cannot be read back. Adding an HTTP parameter alone does not add persistence.

Supporting descriptions upstream requires a storage/model migration plus explicit create/read/update/clear semantics in REST (and Web if offered). Preserve descriptions when other fields change. Define whether clear means null, an empty string or both, and verify stored values after create/set/clear and after rename/color-only updates. Expose an unambiguous versioned capability or documented contract so a client does not infer write support from one read-only field or a successful HTTP status. The CLI rejects description write options before mutation until a supported adapter can confirm their semantics; it still displays descriptions from reads.

### Stable identity and concurrency

Exposing a numeric label ID in the REST representation would remove the need to identify a Web row through sanitized HTML. An ID-based resource route would also make names that are ambiguous URL dot segments safely addressable. The client rejects ambiguous names rather than letting path normalization target another API.

Conditional updates (for example, a checked version/ETag) would be needed to prevent all races between reading current fields and writing a replacement. Client-side snapshot and form checks do not provide a server-side transaction. The Issue label add route also creates unknown repository labels; the client checks additions before any edit, but a concurrent deletion can still race with that server behavior.

## Upstream review API

Formal approval/request-changes/dismissal resources and atomic expected-head merge require separate server support; see [review and merge requirements](merge.md). The comment interfaces below cannot be treated as formal review decisions.

GitBucket 4.44.0 has distinct IssueComment and CommitComment stores. Its PR review URL is advertised, but the PR review REST route is absent in the pinned live fixture. The Web conversation provides useful visible data; a complete persisted review reader needs more server support.

Observed server constraints:

- `getPullRequestComments` gathers comments for commits in the current comparison, including commit comments whose stored issueId is not exposed. History changes can remove records from this view without deleting them.
- Grouping a whole-commit comment and an IssueComment by the same numeric key can suppress the ordinary record in the Web view (commit comments are gathered first). Inline groups use file/original-position keys instead.
- The conversation renders current filename/line attributes and a current-head marker, but omits original commit/line metadata and stored issueId. Updates can remap positions; visible positions do not prove a valid location in Git.
- HTML contains rendered bodies, a local display time, and grouping, rather than universally readable Markdown, full timestamps and explicit parent/thread IDs. The edit-data route is permission-gated, so it is not a complete read API for other authors' comments.

A server endpoint should provide token-authenticated, repository-scoped review enumeration with an explicit PR association policy and stable pagination. It should expose storage kind/ID, actual issueId, current/original commit IDs, path/old/new lines, raw Markdown, author, creation/update timestamps with timezone, HTML URL, and explicitly supported thread/reply relationships. Missing information must remain unknown. Persisted PR-bound history should be enumerable after force-push or fork removal, independently of the current comparison. Existing ordinary comments must not be folded into a conflicting numeric-ID namespace.

An adapter should use such an endpoint only after its contracts are verified. Filling an advertised URL or interpreting an arbitrary 404 as capability evidence would not establish support. The current CLI's `--kind all` means both source kinds within the documented coverage; it cannot promise all stored historical reviews with the present 4.44 server interfaces.

Pinned primary sources:

- [PullRequestService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/PullRequestService.scala): commit enumeration, grouping and position updates.
- [CommitsService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/CommitsService.scala): CommitComment storage/read methods.
- [Comment model](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/model/Comment.scala): separate tables and original/current position fields.
- [Conversation controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/PullRequestsController.scala), [group template](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/twirl/gitbucket/core/helper/commitcomments.scala.html), [comment template](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/twirl/gitbucket/core/helper/commitcomment.scala.html).
- [Web read/edit controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/RepositoryViewerController.scala): per-comment edit-data permission checks.


## Upstream sources

- [PR API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiPullRequestControllerBase.scala)
- [Issue API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueControllerBase.scala)
- [Issue comment API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueCommentControllerBase.scala)
- [Milestone API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueMilestoneControllerBase.scala)
- [Milestone input](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/CreateAMilestone.scala)
- [Label API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueLabelControllerBase.scala)
- [Web commit comments](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/RepositoryViewerController.scala)
