# API JSON and discussion-comment output

API-backed JSON output preserves the selected response object as JSON values,
including fields the CLI does not recognize. Missing keys stay missing, null stays
null, and false or empty collections do not become another value. This applies
to Issue, PR, discussion-comment, repository, label, milestone and user responses,
including nested objects. Formatting and object-key order are not preserved.
Known field types and required identities still undergo validation; malformed or
incompatible responses are not accepted merely because JSON output was requested.
Unknown JSON numbers retain integer and decimal precision, including values
outside the typed projection's integer range. Known IDs still require valid
unsigned 64-bit integers.

`models/response.rs` keeps immutable original JSON with a typed projection.
Serialization uses that original JSON, while commands read the projection through
Deref. There is deliberately no mutable Deref: a projection cannot silently drift
from the JSON that will be printed. An intentional field transformation rebuilds
and validates both representations. Request DTOs remain separate and explicit;
unknown response fields are not sent back as mutation payload fields.

The projection may contain defaults used by existing command logic; these do not
create JSON facts. In particular, a missing repository private/fork flag is not
printed as false merely because the typed projection has a default. Consumers
that relied on formerly synthesized null/false/empty keys should check presence
explicitly rather than assuming that the API returned those values.

## Selected sources and explicit transformations

PR list JSON continues to use canonical details because the 4.44.0 collection can
misidentify fork ownership. Known top-level fields belong to canonical details,
including when those fields are absent/null. Unknown top-level collection fields
are retained if canonical details do not supply the same key; canonical values
win conflicts. Nested head/base/repository data comes from canonical details;
unverified summary objects are not merged back into it. Collection ordering,
deduplication and complete-traversal guarantees remain unchanged.

The existing ordinary null-detail compatibility lookup explicitly withholds the
unverified head as JSON null, updating the projection and original JSON together.
The saved-snapshot reader and its narrow 200/empty exception remain separate;
see [snapshot contracts](PR_SNAPSHOTS.md). Verified PR create/merge envelopes are
unwrapped according to their existing route contracts, then the inner resource's
fields are preserved. Config/auth summaries are CLI-defined JSON and are not raw
API resources. Raw `gb api` retains its existing behavior.

`pr merge --json` is a CLI-defined report: `number`, checked `head_commit`,
`atomic=false`, `verification=client_preflight_and_readback`, and the preserved
unwrapped API object under `result`. Its result identity may be JGit text rather
than a bare hash and is not the reviewed head. See [merge verification](REVIEW_MERGE.md).

## Discussion arrays

Without `--comments`, view JSON is the selected API object without a comment fetch.
With the flag, the command completes ordinary discussion retrieval before printing:

| Command | Array location | Existing response field |
| --- | --- | --- |
| `issue view N --comments --json` | `comment_details` | `comments` stays the API count, null or absent; no count is invented |
| `pr view N --comments --json` | `comments`, preserving the CLI's prior array location | If the API object has `comments`, its original value is retained as `api_comments` |
| `pr comment list N --json` | Root array, unchanged | Each comment object retains API fields |
| `pr comment N --json` | Created/edited object, unchanged | Mutation response retains API fields |

The Issue count and fetched array are separate observations and can disagree if
comments change between requests. Arrays are ordered by comment ID ascending,
with pagination/deduplication from the shared reader. A failed later page prints
no partial JSON. A server field already occupying `comment_details`, or occupying
`api_comments` when it is needed, causes an explicit composition error; no server
field is silently overwritten. View without `--comments` or raw `gb api` remains
available to inspect such a response.

`tests/fixtures/json/issue-with-comments.json` and `pr-with-comments.json` pin the
ordinary augmented object shapes. With --kind review/all, the separate
[review reader](REVIEW_COMMENTS.md) adds review_comments and comment_coverage;
the default ordinary shape remains compatible. API fields such
as review_comments_url are retained as data, not interpreted as proof that the
advertised route works on the deployed server.

Inline write commands produce a separate verified operation result rather than
pretending the Web reply is an ordinary REST Comment. The result carries qualified
identity, Web actor/route, saved comparison, verification type, optional review
readback and explicit position-group reply semantics. The nested review remains
the reader's rendered representation, including its unknown fields. Deletion
returns review null, and a failed verification returns only an operation error.
`tests/fixtures/json/review-write.json` pins the result envelope, combined with
the existing review record fixture in the HTTP regression. See
[write JSON and retry contracts](REVIEW_WRITES.md).

## PR metadata

The typed PR projection now includes nullable draft, labels, assignees and
merged_by. Text view shows reported metadata and marks open draft PRs DRAFT in
view/list. Missing/null draft is not declared false in JSON or shown as an
explicit Draft: no in text. Empty reported labels/assignees display (none).
Merged state takes precedence over the draft badge. Unknown metadata remains
available to JSON consumers without requiring another CLI model change.

## Validation and upstream baseline

The JSON regressions compare complete objects, not only selected known fields.
They cover nested unknown data, Unicode, missing/null/false, empty arrays,
canonical-versus-summary precedence, count-versus-array shape, pagination/errors,
reserved field collisions, PR-create wrappers and comment mutations. Existing
write-payload tests guard the separation between responses and requests.

The live 4.44.0 case compares CLI view JSON with direct REST objects for an Issue
and a real draft PR, then compares discussion arrays and their source responses.
It runs at root and /gitbucket. The baseline Issue API does not provide a comments
count: absence is preserved even after comment_details is added.

Primary response definitions:
[ApiIssue](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiIssue.scala),
[ApiPullRequest](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiPullRequest.scala),
[ApiComment](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiComment.scala).
These define metadata, dummy IDs and resource URLs previously omitted by the CLI.
