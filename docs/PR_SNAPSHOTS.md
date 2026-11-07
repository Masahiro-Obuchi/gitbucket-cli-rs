# Saved pull-request commits

`pr diff` compares the exact `base.sha` and `head.sha` returned for a PR at the
start of the command. `pr checkout` checks out that exact saved head. Both need a
local Git repository and complete, nonzero 40- or 64-digit hexadecimal commit
IDs. Branch names, fetched tips, merge commits and advertised `.diff` URLs cannot
substitute for missing identities.

| PR state | Comparison / checkout meaning |
| --- | --- |
| Open | Current saved comparison in the API response; a subsequent push may update the server's saved pair for a later invocation |
| Closed, unmerged | Pair retained when the server stopped updating the closed PR |
| Merged | Original saved comparison and head, rather than a merge commit or the present base/head branches |

This is a snapshot of the response, not an atomic snapshot across API and Git.
An update during fetch is harmless when the requested objects remain available;
otherwise the command fails instead of switching to another pair.

## Responsibilities

- `domain/pull_request.rs`: validated immutable `CommitId` and the comparison
  identity `PullRequestSnapshot`.
- `application/pull_request.rs`: resolve a numbered PR, reject another number or
  base repository, validate saved SHAs and distinguish canonical repository
  metadata from the collection's untrustworthy fork metadata.
- `git/snapshot.rs`: obtain and verify Git objects, apply existing scoped Git
  authentication and retain verified objects under private refs.
- CLI: checkout safety/provenance, diff formatting and common output/pager use.

The normal typed PR endpoint still rejects empty JSON responses. The snapshot
reader has one separately named response contract: a fully read HTTP 200 with
zero bytes is treated as absent canonical detail, as observed on 4.44.0 after a
fork is deleted. JSON null also represents absent detail. HTTP errors, 204,
whitespace, malformed JSON and interrupted/stalled responses do not trigger
recovery. After absent detail, it completes the PR collection and requires the
requested number and saved SHA pair. Collection head ownership is discarded;
only objects in the selected base repository or its retained PR refs can restore
such a fork. Ordinary Issue collections are never used as proof.

## Obtaining objects

Already present objects are usable by their full verified ID, including after a
source repository goes offline. For a missing head, the reader tries the selected
base repository's actual 4.44.0 `refs/pull/N/head`, then a full-SHA fetch, then
canonical head-repository SHA/branch hints. If canonical head ownership is absent,
a same-name branch in the selected base is only an object-fetch hint. For a
missing base, it tries its full SHA and base-branch hint. Fetching a branch is
successful only if the saved object is then available and resolves to precisely
the required commit ID; the branch's current tip is never accepted in its place.
This also supports servers that disallow direct SHA fetches but expose a retained
PR ref. If no allowed source supplies the saved object, recovery fails.

Fetch sources use canonical clone metadata or an instance-derived URL and a
matching local remote with the same server and repository identity. There is no
unconditional `origin` fallback to an unrelated repository. Existing credential
boundaries and redirect restrictions apply. Fetches disable tag following,
tracking ref updates, FETCH_HEAD writes and submodule recursion. Temporary refs
are under `refs/gb/pr-targets/v1/<target digest>/fetch/<role>`. Verified commits
are retained under bounded-length `refs/gb/pr-targets/v2/<digest>` refs keyed by
the target and saved commit, so local garbage collection need not discard a
previously fetched snapshot. User branches and legacy PR refs are not reset.
These private refs/cache objects may remain after a failed command.

Git replacement objects are disabled while verifying and consuming snapshots.
Diff uses the two saved trees directly, without recomputing merge-base against
moving branches. In 4.44.0 the saved base is the server's comparison starting
commit; redoing a three-dot comparison can change its meaning and requires
ancestry that a shallow clone might not have. Exact tree comparison can work
without deepening a shallow clone when both commit/tree contents are available.
Missing trees/history or an unavailable retained object remains a real limit.

Diff enables binary patches, full object IDs and rename detection; disables
external diff/textconv/color and working-directory-relative output; and includes
submodule commit changes without checking out submodules. Git output goes to a
temporary file, rather than an in-memory patch buffer. Only after Git succeeds is
the file streamed to the shared pager/output path. A failed Git command prints no
partial patch. Equal saved trees legitimately produce an empty successful diff.
TTY output uses PAGER (default less -R), while `--no-pager` and redirected stdout
bypass it. Pager stdin is closed and the child is reaped, including early exit.

`diff_unavailable` causes include `missing_snapshot`,
`git_repository_unavailable`, `head_object_unavailable`,
`base_object_unavailable` and `git_diff_failed`. API/decode and Git
cancellation/deadline failures retain their original error classification.
Checkout retains its clean-worktree checks, shared lock, branch provenance,
non-forcing switch and post-hook/cancellation outcome handling.

## Evidence and limits

Pinned [ApiPullRequest](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiPullRequest.scala)
maps saved commitIdFrom/commitIdTo to base.sha/head.sha.
[PullRequestService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/PullRequestService.scala)
creates the saved pair and updates open PRs on head pushes.
[JGitUtil](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/util/JGitUtil.scala)
implements the comparison starting point and retained PR-head ref.

Live root and `/gitbucket` fixtures exercise closed branch reuse/deletion,
merged branch reuse, and deleted forks. The deleted-fork contract reads the
200/empty response directly, verifies the collection's saved head SHA and the
advertised PR ref, then restores diff and checkout through the CLI. Local Git
regressions additionally cover force-push, shallow clones, offline sources,
replacement refs, binary/rename/large patches, terminal paging, authenticated
fetch boundaries and unchanged checkout safeguards.

This baseline does not promise recovery if the server removes both retained
objects/refs and the original fork and no verified local objects remain. It does
not add inline comments or claim support for every GitBucket version's lifecycle
semantics. Future review-comment positions should use these immutable commit
identities, with separately verified comment/diff coordinate models.
