# Reading PR discussion and review comments

```sh
gb pr comment list 42 --json
gb pr view 42 --comments --json
gb pr comment list 42 --kind review --no-pager
gb pr comment list 42 --kind discussion --json
```

`--kind all` is the default for comment lists and for PR view with `--comments`. `--kind discussion` selects ordinary discussion. `--kind review` selects visible file/line and whole-commit comments. `--edit-last` continues to edit only ordinary discussion comments. A comment mutation does not accept the read filter. [Inline writes](review-writes.md) use separate explicit position/ID arguments; the read filter does not authorize a mutation.

| Source | Reader | Credentials |
| --- | --- | --- |
| Ordinary discussion | Complete Issue comments REST collection | Existing API token |
| Visible PR review conversation | GitBucket 4.44 HTML contract | Try the existing token/public read first; only HTTP 401/403 starts cookie sign-in |

Public reviews need no Web password, even when no `GB_USER`/`GB_PASSWORD` is set. Private 4.44 pages deny the token-only Web request with 401. The verified Web session can then use `GB_PASSWORD` or a password saved with `gb auth web-login` in the [credential store](../usage/authentication.md#web-passwords). To enter a password at a terminal while reading comments, select `--kind review` explicitly. Actor checks and explicit `GB_WEB_USER` rules apply. Transport, decode, HTTP 5xx, redirects or unrecognized HTML do not trigger a sign-in retry. The Cookie reader also rejects an expired session/login redirect.

The verified server advertises review URLs but `/pulls/N/comments` returns 404; Issue comments omit CommitComments. This reader uses the proven Web representation without repeatedly probing that unsupported REST route. An advertised URL is not proof of a supported operation. [Upstream API requirements](gitbucket-compatibility.md#upstream-review-api) describe how a complete token-only reader could replace this adapter.

## Output contracts

`pr comment list --json` returns `comments`, `review_comments` and `comment_coverage`. `pr view --comments --json` adds coverage and the requested arrays `comment_details` and `review_comments`. It preserves all API fields, including any `comments` value. Field name collisions return an error.

An empty array means the source was read successfully and contained zero comments. A null value with coverage status `unavailable` means the read failed. Inspect `reason` for the cause, such as `web_password_required` or `unsupported_markup`. A filtered source has status `not_requested`, with a null value in list JSON and an omitted field in view JSON. Scripts that require ordinary discussion alone can pass `--kind discussion` and check `comment_coverage.discussion.status` before interpreting the array.

With `--kind all`, a successful read of either source produces results with exit 0 and reports any unavailable source. If every requested source fails, the command exits 1 with an error. Invalid PR targets and cancellation fail the command.

Both commands use the same application reader and ordering: ordinary comments by ID ascending, then reviews by CommitComment ID ascending. This groups sources rather than inferring chronology across different timestamp formats/timezones. Each source remains a separate array in JSON. Text output includes qualified IDs such as `inline:https://example.test/gitbucket/alice/project:123`.

Review `identity` contains kind (`inline` or `commit`), configured instance, repository and numeric ID. Ordinary identities use kind `discussion` and the same instance/repository context, given in coverage and the text ID. Discussion and review ID 123 are different records; duplicate review IDs are rejected across inline/whole-commit subtypes because they share the CommitComment table.

`body` contains `format: rendered_html`, the rendered DOM HTML, normalized plain text, and `markdown: null`. The Web page does not expose raw Markdown for every author. Creation time is a server-local displayed timestamp; timezone, update time, original commit/lines and reply-parent identity stay null when unavailable. A `thread` identifies the observed Web position group, not a proven reply chain or persistent server thread ID.

`tests/fixtures/json/review-comment.json` pins the expanded review record shape.

## Scope and position limits

`comment_coverage.review.scope` is `visible_pr_conversation`, with status `complete_for_scope` after parsing the recognized page. The source contract is `gitbucket_4_44_conversation_html`. `review_access` distinguishes `token_or_public` from `cookie_session`; `web_actor` is known only for a verified Cookie session. `head_commit_id` records the saved head checked before and after the read. If the head changes, the review source is unavailable with reason `head_changed`. Requests still do not form an atomic snapshot.

The Web page can show commit comments without exposing their stored PR association. A review therefore reports `visible_on_pull_request` for the selected page, while its stored `pull_request` stays null. Visibility does not establish that the record was created specifically for that PR.

Positions report the displayed filename and optional old/new lines, including both values or file-level comments without a line. Significant filename whitespace is preserved when matching the heading and filename attribute. GitBucket 4.44's Web posting form itself trims filenames before storing them; whitespace lost by that server operation cannot be recovered by this reader. `position.source` is `web_display` and `validated_against_git` is false. The pinned `inline-comment` marker identifies current-head display positions; older positions retain unknown commit IDs when the page does not provide them. The `outdated` value follows this Web marker, and whole-commit comments leave it null. This does not certify that a line is still valid in a Git tree. Whole-commit comments carry a verified full SHA from their commit link; their PR URL has no fabricated unique anchor.

4.44 can omit reviews after history replacement and can remap stored display positions after pushes. Its grouping can conflate a whole-commit comment and an ordinary IssueComment with the same numeric ID, omitting a record from the Web conversation. The CLI therefore retrieves discussion comments independently over REST. Consequently `historical_review_complete` is null with an explicit reason, even after every visible review is read. The CLI does not claim complete stored history or recover hidden records by guessing IDs. These limits were reproduced in dedicated fixtures.

## Validation

- `domain/review_comment.rs`: source-specific identities and review data with explicit unknowns.
- `application/pr_comments.rs`: source composition and coverage; null/fully read 200-empty deleted-fork metadata uses the existing PR collection compatibility contract while withholding unverified fork head ownership.
- `api/web/review.rs`: token and Cookie Web adapters plus a DOM parser for the pinned 4.44 conversation layout. Exact PR route, trusted structural containers, identities, paths, author links, timestamps and group membership are checked. Every direct comment element in a group must be recognized; an unknown sibling fails the whole read rather than silently reducing the reported collection. User-body markup cannot add records. Zero reviews still require recognizable PR identity and conversation structure. The adapter does not infer a binary server version from asset filenames; other layouts/releases are unverified.
- CLI/output: shared view/list records, kind filter, saved-head check, and separate source results.

Unit and HTTP regressions cover old/new and nullable lines, same numeric IDs in different stores, groups, Unicode, body representation, unknown/mismatched markup, public password-free reads, private session expiry, HTTP failures, head changes, proxy deployment paths and null detail recovery. Live root and /gitbucket cases cover real rename/deleted files, multiple authors, outdated positions, branch replacement, unchanged discussion-only editing, and both public/private access.
