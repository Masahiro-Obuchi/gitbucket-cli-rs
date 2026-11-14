# Upstream requirements for complete review access

Formal approval/request-changes/dismissal resources and atomic expected-head
merge require separate server support; see [review and merge requirements](REVIEW_MERGE.md).
The comment interfaces below cannot be treated as formal review decisions.

GitBucket 4.44.0 has distinct IssueComment and CommitComment stores. Its PR review
URL is advertised, but the PR review REST route is absent in the pinned live
fixture. The Web conversation provides useful visible data; a complete persisted
review reader needs more server support.

Observed server constraints:

- `getPullRequestComments` gathers comments for commits in the current comparison,
  including commit comments whose stored issueId is not exposed. History changes
  can remove records from this view without deleting them.
- Grouping a whole-commit comment and an IssueComment by the same numeric key can
  suppress the ordinary record in the Web view (commit comments are gathered
  first). Inline groups use file/original-position keys instead.
- The conversation renders current filename/line attributes and a current-head
  marker, but omits original commit/line metadata and stored issueId. Updates can
  remap positions; visible positions do not prove a valid location in Git.
- HTML contains rendered bodies, a local display time, and grouping, rather than
  universally readable Markdown, full timestamps and explicit parent/thread IDs.
  The edit-data route is permission-gated, so it is not a complete read API for
  other authors' comments.

A server endpoint should provide token-authenticated, repository-scoped review
enumeration with an explicit PR association policy and stable pagination. It
should expose storage kind/ID, actual issueId, current/original commit IDs,
path/old/new lines, raw Markdown, author, creation/update timestamps with timezone,
HTML URL, and explicitly supported thread/reply relationships. Missing information
must remain unknown. Persisted PR-bound history should be enumerable after
force-push or fork removal, independently of the current comparison. Existing
ordinary comments must not be folded into a conflicting numeric-ID namespace.

An adapter should use such an endpoint only after its contracts are verified.
Filling an advertised URL or interpreting an arbitrary 404 as capability evidence
would not establish support. The current CLI's `--kind all` means both source
kinds within the documented coverage; it cannot promise all stored historical
reviews with the present 4.44 server interfaces.

Pinned primary sources:

- [PullRequestService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/PullRequestService.scala): commit enumeration, grouping and position updates.
- [CommitsService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/CommitsService.scala): CommitComment storage/read methods.
- [Comment model](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/model/Comment.scala): separate tables and original/current position fields.
- [Conversation controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/PullRequestsController.scala), [group template](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/twirl/gitbucket/core/helper/commitcomments.scala.html), [comment template](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/twirl/gitbucket/core/helper/commitcomment.scala.html).
- [Web read/edit controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/RepositoryViewerController.scala): per-comment edit-data permission checks.
