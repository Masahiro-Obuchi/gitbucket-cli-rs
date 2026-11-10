# Writing inline review comments

```sh
gb pr comment 42 --file src/lib.rs --line 18 --side new --commit <FULL_HEAD_SHA> --body-file review.md --web --json
gb pr comment edit 42 --kind inline --id 123 --body 'Updated **review**' --web --json
gb pr comment reply 42 --kind inline --id 123 --body '確認しました' --request-id review-42-followup --web --json
gb pr comment delete 42 --kind inline --id 123 --web --json
```

The existing body-only command and `--edit-last` continue to use ordinary
discussion REST comments. `--body-file` reads a UTF-8 file and conflicts with
`--body`; omission prompts for text. Inline mutations require explicit `--web`.
ID commands also require `--kind inline`: an IssueComment with the same number
is never selected. Whole-commit and file-level writes are not supported here.

## Saved position and actor

Inline writes require a local Git repository. The CLI obtains the saved PR
base/head objects using the same authenticated, bounded fetch adapter as
checkout/diff. It uses the exact trees, with replacement objects, external diff
and text conversion disabled. The working tree/index is not the source of the
comment coordinates. Running in a subdirectory still uses repository-root paths;
pathspec metacharacters are literal.

Use the destination filename for a rename, and the old filename for a deletion.
`--side old|new` and `--line` identify file lines, not patch offsets. Only regular
UTF-8 text files and lines in a diff hunk with three context lines are supported;
binary files, symlinks, submodules, missing sides and unchanged/out-of-hunk lines
are refused. Context lines carry both old/new numbers. The chosen line and up to
three preceding hunk rows are sent as GitBucket's stored diff snippet. Git and
JGit can choose different rename/hunk boundaries; the saved Git validation is
explicit, and post-write Web coordinates must also match.
The selected file's patch is produced on disk and limited to 32 MiB before it is
loaded for position validation; larger diffs fail before submission.

Paths must round-trip through GitBucket's form: control characters, backslashes,
absolute/dot-segment paths and boundary whitespace are rejected. The 4.44 form
trims filenames, so a whitespace-bearing Git path cannot be safely posted by
substituting its trimmed name. Bodies normalize CR/CRLF to LF and trim only the
server form's boundary characters U+0000 through U+0020; nonempty text is required.

Creation and replies require an open, unmerged PR. `--commit` optionally binds
any mutation to a complete saved head SHA; omission fixes the head read at start.
Existing IDs must be visible, current inline records whose path/line agrees with
that diff. Outdated and file-level records are refused because their original
coordinates are not exposed by the read contract.

Writes always use a Cookie session. Existing REST/Web actor verification and
explicit `GB_WEB_USER` rules apply; passwords are not persisted. Editing/deleting
is restricted to the verified Web actor's own records, and the permission-gated
edit form must be readable before submitting. Administrator privileges do not
silently widen that CLI rule. Replies can target another author's current record.
CommitComment edits/deletes affect the record wherever it is displayed, including
other PR/commit views; selected-PR visibility is not exclusive stored membership.

## Verification and replies

The saved base/head comparison is checked immediately before submission and
again after readback. Create/reply also recheck that the PR is open and unmerged
immediately before the POST. New records must have a unique ID absent from the
initial collection and match kind, actor, path, old/new lines and current head. The
permission-gated edit form supplies original Markdown for exact body comparison;
rendered HTML alone cannot verify the submitted body. Edits verify the same ID
and body; deletion requires both conversation absence and a resource GET 404.
An already matching edit is a read-only no-op.

Replies create another comment at the validated position. They do not persist a
parent ID. Success additionally requires the observed position-group ID to match
the selected target; JSON `reply_to` records the CLI's selected target, with
`reply_semantics: same_web_position_group_not_parent_id`. This is not evidence
of a stored reply chain. If a remapped original group cannot be confirmed, the
new submission remains unverified and is not repeated or automatically deleted.

Success JSON includes `status`, `action`, qualified `identity`, `web_actor`,
`route: web`, `reused`, `review`, `reply_to`, `reply_semantics`, `verification`,
`base_commit_id` and `head_commit_id`. Deletion has `review: null`. The nested
review retains the [reader's source representation](REVIEW_COMMENTS.md), including
unknown stored PR association/original metadata and Web display positions;
`verification` separately records this operation's Git/body or absence check.
Normal output prints the qualified ID and available URL. Readback failure prints
no success JSON and uses the existing operation error with `outcome: unverified`.
When submission and verification both fail, the original transport status/cause
is retained. No POST is automatically retried.

## Retry records

Before create/reply POST, the CLI durably reserves an immutable file under
`GB_CONFIG_DIR/review-writes` (the normal config directory when unset). Files hold
hashes of the intent/comparison and the initial review IDs, without a token,
password or body text. Reservation uses exclusive file creation; a partial,
corrupt or conflicting receipt fails before another POST. Unix directory/file
permissions are 0700/0600; Windows inherits its parent ACL. Symlink/reparse
receipt paths are refused. Keep these records when backing up the configuration.

Without `--request-id`, the retry key derives from instance, repository, PR,
Web actor, coordinates, normalized body and optional reply target. A retry with
the same key only reads the server: a unique new matching record returns
`reused: true`; otherwise it fails without resubmission. A changed comparison for
the same reserved intent is rejected. An edit, deletion or hidden historical
record can prevent later confirmation of an earlier create request.

Use a stable, unique `--request-id` (1–128 bytes) in automation so retries retain
their identity even if defaults/coordinates change. Reusing it with a different
intent/comparison is refused. Choose a **new** key for an intentional identical
submission, or only after inspecting an unresolved earlier attempt. Do not
delete receipts merely to get past an unverified result. Sharing neither the
config directory nor an actor/key does not coordinate other machines/clients.

## Server boundaries

The verified 4.44 routes are AJAX POST `commit/<SHA>/comment/_data/new`,
`commit_comments/edit/<ID>`, `commit_comments/delete/<ID>`, and permission-gated
GET `commit_comments/_data/<ID>?dataType=html`. No verified review REST mutation
replaces them. Existing ordinary comment operations remain REST-only.

The server has no conditional head/body write or idempotency-key contract. A
push/edit/delete can race after preflight and before POST; post-read verification
detects inconsistency but cannot atomically prevent or roll back an applied
write. Concurrent external submissions can also make reconciliation ambiguous.
The visible conversation cannot prove complete history or stored PR membership.
Other releases/layouts and server-side hooks/plugins remain unverified.

Pinned sources: [controller and forms](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/RepositoryViewerController.scala),
[comment storage](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/CommitsService.scala),
[browser comment form](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/twirl/gitbucket/core/repo/commentform.scala.html),
[edit form](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/twirl/gitbucket/core/repo/editcomment.scala.html).
