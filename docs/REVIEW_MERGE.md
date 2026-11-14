# Review decisions and merge guarantees

The verified baseline is GitBucket **4.44.0 core**, using the pinned Docker image
at root and `/gitbucket`. This is not a minimum-version or plugin/SSO compatibility
claim. Other versions and extensions need their own contract tests.

## Supported operations

| Capability | Verified route / CLI behavior | Limit |
| --- | --- | --- |
| Ordinary PR discussion | REST Issue comments, `pr comment` | A comment is not an approval or change request |
| Visible inline review read/write | Version-specific Web adapters; `pr comment --kind review/all` and inline writes | Visible conversation, not complete stored review history; see [read](REVIEW_COMMENTS.md) and [write](REVIEW_WRITES.md) contracts |
| Approve / request changes / dismiss a decision | Core `/pulls/N/reviews` and dismissal routes are absent in 4.44.0; no CLI adapter | No comment is substituted or reported as a formal decision |
| Draft, labels, assignees, merged-by | Reported PR API metadata is shown and retained in JSON | Missing metadata stays unknown |
| Review decision | Human view explicitly reports unknown / not queried | Neither comment text/counts nor a missing reviews URL proves approval |
| Merge | REST, full-head client checks and canonical readback | No atomic expected-head guard; no approval-policy guarantee |
| Atomic expected-head merge | Not implemented by the verified core API or this client | Requires server support, not another local comparison |

The review endpoint observations use a readable PR and authenticated positive
controls. Direct GET, APPROVE/REQUEST_CHANGES POSTs and a dismissal PUT return
404 without adding ordinary comments. The pinned source's route registration
and fallback corroborate that absence; an arbitrary production 404 alone is not
capability detection. No speculative write probe runs during normal CLI use.

`pr view N` shows the complete saved `Head commit` when available and the
server-reported `Mergeable: yes/no/unknown`, in addition to draft/assignment
metadata. It does not query or infer formal review decisions. View/list JSON
retains original API fields, including unknown plugin fields, without adding a
fabricated decision. There is no `pr review --approve` command in this adapter.

## Merge sequence

```sh
gb pr view 5 --no-pager
gb pr merge 5 --match-head-commit 0123456789abcdef0123456789abcdef01234567
gb pr merge 5 --match-head-commit 0123456789abcdef0123456789abcdef01234567 --json
```

`--match-head-commit` takes a complete nonzero 40- or 64-digit hexadecimal SHA.
Use the head you actually reviewed. Without the option, the first saved head
read during this invocation becomes the comparison value; this cannot bind the
operation to an earlier human review.

1. Read the numbered PR and validate its number/base target and saved head.
   Unknown summary head data from the null-detail compatibility path cannot
   authorize a merge. Refuse closed/merged PRs, reported drafts and an explicit
   `mergeable=false`. Missing/null readiness is not interpreted as approval;
   the server still decides whether the merge is permitted.
2. Compare with the supplied head, then re-read canonical details immediately
   before PUT. Check the target, saved head and readiness again. A missing,
   changed, inaccessible or malformed fresh response stops before the merge.
3. Send one merge PUT with the requested commit message. The unsupported `sha`
   payload field is deliberately absent. GitBucket 4.44.0 ignores it: a direct
   test with a wrong SHA still merges. The response's `sha` can be formatted as
   `AnyObjectId[<hash>]`; preserve this opaque result, rather than treating it as
   the saved head or promising a bare merge-commit hash.
4. Require a positive merge response with a nonempty result identity, then read
   canonical details again. Only a closed, `merged=true` PR with the checked
   saved head permits success. The result identity is a different concept from
   the reviewed head; it is not compared for equality with that head.

These steps cannot close the race between the last GET and server-side merge,
or protect a moving base branch. Readback is evidence after the operation, not a
transaction or proof of exclusive execution. No network error, unknown response
or readback failure causes a retry, Web fallback, comment post or rollback.
Server permissions, branch protection and approval enforcement remain server
responsibilities. Client checks cannot supply a missing server approval policy.

## Output and failures

Normal success includes the checked head and `client checks; not atomic`. A
preparation notice on stderr explains the race; `--json-errors` suppresses that
notice but retains the success text or explicit metadata in `--json`.

Successful `--json` is a new CLI report, not a bare API resource:

```json
{
  "number": 5,
  "head_commit": "0123456789abcdef0123456789abcdef01234567",
  "atomic": false,
  "verification": "client_preflight_and_readback",
  "result": {"merged": true, "sha": "AnyObjectId[<hash>]", "message": "..."}
}
```

`result` preserves the original unwrapped API JSON and its unknown fields.
`head_commit` is the saved head observed by the checks. Neither field means an
atomic guarantee.

Failures exit 1. `--json-errors` includes operation `pr.merge` on route `rest`:

- Before PUT: `not_attempted`; local check errors use `merge_check_failed` and
  causes such as `head_mismatch`, `missing_head`, `draft`, `not_open`,
  `not_mergeable`, or `target_mismatch`. An invalid option SHA exits 2 before I/O.
- Definite HTTP 4xx rejection: `failed`, retaining the HTTP status/error.
- Timeout, disconnect, 5xx, malformed/unconfirmed success or failed/contradictory
  readback: `unverified`, with `outcome=unverified`. The merge may already have
  happened. Inspect the PR and repository before another attempt.

A subsequent refusal of an already merged PR describes the new invocation; it
does not prove that a previous uncertain request did nothing.

## What a server extension must guarantee

For an atomic expected-head merge, the server must compare the caller's expected
SHA with the merge input while holding the same lock/transaction that selects
and updates the merged refs. A mismatch must reject without changing refs, issue
state or merge comments. Permission, draft and protected-branch checks must also
apply to that operation. Verify these semantics with a controlled concurrent
head update before enabling a conditional adapter; a version label, accepted
unknown parameter or client GET cannot establish them.

Formal review support likewise needs a verified decision resource with actor,
commit scope, lifecycle/dismissal semantics and readable results. Complete stored
review history has the additional [upstream requirements](UPSTREAM_REVIEW_API.md).
The core's missing features are not marked complete by this PR.

Pinned primary sources: [merge controller](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiPullRequestControllerBase.scala),
[merge input](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/MergeAPullRequest.scala),
[merge service](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/MergeService.scala),
[registered API routes](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/ApiController.scala).

Regression evidence lives in `tests/pr_merge.rs` and the pinned direct/CLI
contracts in `tests/e2e_smoke/review_merge.rs`.
