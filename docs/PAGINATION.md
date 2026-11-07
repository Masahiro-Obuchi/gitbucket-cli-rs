# Complete Issue, PR and comment collections

The collection reader lives in `src/api/pagination.rs`. Endpoint wrappers select
an explicit contract instead of inferring pagination from an arbitrary row count.
The contracts are pinned to GitBucket 4.44.0 and tested directly over REST in
`contract_fixed_pages_all_comments_and_canonical_fork_identity`.

| Collection | No Link header | Order returned by gb |
| --- | --- | --- |
| Issues | Fixed 25-row pages; request page 2, 3, ... until a short/empty page | Number descending |
| Pull requests | Same fixed 25-row pages | Number descending |
| Issue/PR discussion comments | One response contains all comments, regardless of count | Comment ID ascending |

GitBucket ignores `per_page` on these routes. The Issue collection excludes PRs;
it cannot repair an incomplete PR listing. The CLI does not query Issues as a
fallback for PR lists or existing-PR detection. Other collections and raw `gb api`
retain their endpoint behavior; this is not a global pagination heuristic.

For servers that provide Link metadata, `rel=next` takes precedence. Multiple
header fields and relation lists are accepted. Once pagination Link relations
(`next`, `prev`, `first`, `last`) are present, absence of next ends traversal,
even if the terminal page has 25 rows. Unrelated relations such as `canonical`,
`self`, or `describedby` do not disable the fixed-page contract.
Absolute, root-relative, path-relative and query-relative targets resolve against
the actual response URL. Every next target must stay inside the configured API
base and the same collection endpoint, without credentials or fragments. Omitted
initial filters such as `state=closed` are restored; conflicting filters fail.

Rows are deduplicated by Issue/PR number (the API's dummy `id` is unsuitable), or
comment ID, retaining the first observation. Final ordering is explicit because
the server need not preserve the order within a page. Zero IDs, cyclic URLs,
nonempty pages without any new IDs (including terminal pages),
malformed/ambiguous next Links, unexpected
no-Link Issue/PR pages larger than 25, and an exhausted safety cap are errors.
HTTP and decode failures retain their transport error code, cause and status.
The command returns no partial JSON array/table and does not decide that a PR is
absent after an incomplete search. Diagnostics do not include next-Link URLs.

`GB_PAGINATION_MAX_PAGES` defaults to 1000, accepts integers from 1 to 100000, and
bounds each collection traversal. At an exact 25-row boundary, a final empty
request may be needed and counts towards this cap. Hitting the cap with another
page pending fails with `pagination_incomplete`, cause `page_limit`; it is not a
display limit. Other pagination causes include `pagination_cycle`,
`pagination_no_progress`, `invalid_item_id`, `unknown_pagination`,
`invalid_link_header`, `ambiguous_next_link`, `unsafe_next_link` and
`changed_filter`. No `--limit` is introduced: display and internal searches both
finish traversal before succeeding. A future display limit must remain separate
from the complete search required before PR creation.

## Canonical PR identity

The 4.44.0 PR collection can report a fork's head repository and label as belonging
to the base repository. `pr list --json` therefore fetches every listed PR's
canonical detail, for every state filter. A missing, failed or wrong-number
detail fails the list; metadata is not silently left unverified. Table output uses
summary fields and avoids these extra detail requests.

`pr create --detect-existing` first completes the open PR collection. Different
head/base branch refs can exclude candidates; collection ownership cannot.
Possible candidates always require canonical detail before comparing repository
identity and refs. Missing or conflicting identities fail before creating a PR.
If create fails, the same complete check can recover a matching open PR; a failed
recheck preserves the original create error/status/outcome with added context.
Cancellation before recheck exits without starting it. Cancellation during a
running recheck retains the original POST error/status/outcome and adds context
that the recheck was cancelled. Lookup diagnostics never assert that an earlier
create could not have succeeded. The POST is never retried.

Only an explicit JSON null single-PR detail activates the existing compatibility
lookup. It searches the complete PR collection and withholds `head`, whose fork
identity cannot be verified from that collection. Basic viewing/comment operations
can establish the PR number; Git operations requiring the head may fail rather
than fetching from a potentially unrelated repository. HTTP failures, malformed
JSON and interrupted responses do not activate this fallback.

Traversal is not a snapshot: concurrent creation/deletion/state changes can move
rows between pages. Deduplication removes repeated observations but cannot recover
records the server never exposes. Detection and creation are not atomic, so this
option cannot guarantee uniqueness under concurrent writers. Review-thread and
inline-comment types are separate from ordinary discussion comments; they belong
to subsequent remediation units.

## Upstream evidence

- [Issue API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueControllerBase.scala)
  and [PR API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiPullRequestControllerBase.scala)
  pass the requested page into the search service.
- [IssuesService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/IssuesService.scala)
  selects Issues versus PullRequests, slices with PullRequestLimit and joins the
  base repository for the collection; [PullRequestService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/PullRequestService.scala)
  defines the 25-row limit and retrieves canonical head details.
- [Comment API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueCommentControllerBase.scala)
  returns the complete comment collection without page/per_page slicing.
- [ApiPullRequest](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiPullRequest.scala)
  constructs head repository metadata and label from the supplied repository.
