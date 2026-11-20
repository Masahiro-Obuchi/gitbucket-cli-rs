# Automation

Use `--json` on commands that support structured output and `--json-errors` for structured failures. See [input and terminal display](../usage/input-and-display.md) for body sources, editor behavior, and display escaping.

## Environment variables

| Variable | Description |
| --- | --- |
| `GB_HOST` | Default hostname or base URL |
| `GB_REPO` | Default repository (`OWNER/REPO`) |
| `GB_PROFILE` | Default configuration profile |
| `GB_TOKEN` | Access token override |
| `GB_PROTOCOL` | Protocol selection for a bare target, including saved credentials |
| `GB_USER` | Username for Web sessions and token-authenticated Git; does not establish the REST actor |
| `GB_WEB_USER` | Explicit Web user override; authorizes an actor different from the REST token |
| `GB_PASSWORD` | Password for GitBucket web-session fallbacks |
| `GB_HTTP_CONNECT_TIMEOUT` | HTTP connect deadline in seconds (default 10) |
| `GB_HTTP_READ_TIMEOUT` | HTTP read inactivity deadline in seconds (default 30) |
| `GB_PAGINATION_MAX_PAGES` | Maximum pages per collection traversal (default 1000, range 1–100000); pending next page at the cap is an error |
| `GB_HTTP_TIMEOUT` | HTTP total deadline including body in seconds (default 120) |
| `GB_GIT_TIMEOUT` | Deadline per Git subprocess including output (default 300) |
| `GB_CONFIG_DIR` | Custom config directory |
| `GB_JSON_ERRORS` | Print command failures as structured JSON on stderr |
| `NO_COLOR` | Disable colored output (terminal/toolchain dependent) |
| `GB_EDITOR`, `VISUAL`, `EDITOR` | Editor selection in that order for explicit `--editor` |
| `PAGER` | Pager command for interactive output |


## HTTP and Git deadlines

REST and Web requests use a 10-second connection deadline, a 30-second read inactivity deadline, and a 120-second total request deadline including its body. Each Git subprocess has a 300-second deadline, including output collection. Configure them with `GB_HTTP_CONNECT_TIMEOUT`, `GB_HTTP_READ_TIMEOUT`, `GB_HTTP_TIMEOUT`, and `GB_GIT_TIMEOUT` (seconds, fractional values accepted; minimum 0.001, maximum 86400). Zero does not disable deadlines.

```bash
GB_HTTP_TIMEOUT=180 GB_HTTP_READ_TIMEOUT=45 gb pr view 7
GB_GIT_TIMEOUT=600 gb repo clone alice/large-repository
```

HTTP failures retain the method/path, failure phase, cause and known response status. Query strings and credentials are excluded from request diagnostics; error bodies are bounded and sensitive details are omitted/redacted. With `--json-errors`, a write whose response cannot be confirmed adds `error.outcome: "unverified"`. `error.cause` distinguishes `timeout`, `connect_failed`, `body_read_failed`, and `invalid_json`. Inspect the target before retrying: comments, merges, forks and other writes are never automatically resent, including after timeouts or an incomplete response body.

Git timeout/Ctrl+C terminates its process group (Unix) or job (Windows), waits for its child, and closes inherited pipes. Git stdin is closed and Git terminal credential prompts are disabled (`GIT_TERMINAL_PROMPT=0`). SSH or external helpers can have their own prompts, within the process deadline. No automatic stash/reset/clean is performed; clone directories, refs or worktrees may already have changed. Uncatchable process termination and a deliberately detached Unix helper are outside this cleanup. Interactive prompts and `gb api --input -` also respond to cancellation and exit with code 1 (`cancelled` with `--json-errors`). Cancelling input does not submit the pending operation. If PR checkout is interrupted and its state cannot be read back, the original mutation error and checkout provenance are retained.

### HTTP response contracts

| Operation | Successful response |
| --- | --- |
| Typed JSON endpoint | Decode the expected model. Empty, malformed, and incompatible bodies are errors. |
| Raw `gb api` | Preserve strings, arrays, and objects. Zero-byte success prints JSON `null`. Nonempty non-JSON bodies are errors. |
| Text reads | Read diff or HTML text. An empty successful response is empty text. |
| Empty mutations | Read the HTTP response to completion before reporting success. |
| Saved PR details | JSON `null` and a fully read HTTP 200 with a zero-byte body can trigger [saved-commit recovery](pull-request-snapshots.md). |
| PR creation | Accept the GitBucket 4.44.0 JSON string wrapper containing the PR object. |
| PR merge | Accept the verified `ActionResult` envelope with the matching status, object headers, and a string body holding the merge result. The result requires a Boolean `merged` field. |

Ordinary numbered PR commands require a detail object. Wrapper decoding is specific to PR creation and merge. Authenticated transfers stay within the configured instance and API base. Configure the direct URL for an instance that redirects.

## API JSON

API-backed JSON output preserves the selected response object as JSON values, including fields the CLI does not recognize. Missing keys stay missing, null stays null, and false or empty collections do not become another value. This applies to Issue, PR, discussion-comment, repository, label, milestone and user responses, including nested objects. Formatting and object-key order are not preserved. Known field types and required identities still undergo validation; malformed or incompatible responses are not accepted merely because JSON output was requested. Unknown JSON numbers retain integer and decimal precision, including values outside the typed projection's integer range. Known IDs still require valid unsigned 64-bit integers.

`models/response.rs` keeps immutable original JSON with a typed projection. Serialization uses that original JSON, while commands read the projection through Deref. There is deliberately no mutable Deref: a projection cannot silently drift from the JSON that will be printed. An intentional field transformation rebuilds and validates both representations. Request DTOs remain separate and explicit; unknown response fields are not sent back as mutation payload fields.

The projection may contain defaults used by existing command logic; these do not create JSON facts. In particular, a missing repository private/fork flag is not printed as false merely because the typed projection has a default. Consumers that relied on formerly synthesized null/false/empty keys should check presence explicitly rather than assuming that the API returned those values.

### Selected sources and explicit transformations

PR list JSON continues to use canonical details because the 4.44.0 collection can misidentify fork ownership. Known top-level fields belong to canonical details, including when those fields are absent/null. Unknown top-level collection fields are retained if canonical details do not supply the same key; canonical values win conflicts. Nested head/base/repository data comes from canonical details; unverified summary objects are not merged back into it. Collection ordering, deduplication and complete-traversal guarantees remain unchanged.

Ordinary numbered PR commands require a detail object. The saved-snapshot reader can also recover from a 200/empty response after fork deletion. See [snapshot contracts](pull-request-snapshots.md). Verified PR create/merge envelopes are unwrapped according to their existing route contracts, then the inner resource's fields are preserved. Config/auth summaries are CLI-defined JSON and are not raw API resources. Raw `gb api` retains its existing behavior.

`pr merge --json` is a CLI-defined report: `number`, checked `head_commit`, `atomic=false`, `verification=client_preflight_and_readback`, and the preserved unwrapped API object under `result`. Its result identity may be JGit text rather than a bare hash and is not the reviewed head. See [merge verification](merge.md).

### Discussion arrays

Without `--comments`, view JSON is the selected API object without a comment fetch. With the flag, the command adds results from the requested sources.

| Command | Array location | Existing response field |
| --- | --- | --- |
| `issue view N --comments --json` | `comment_details` | `comments` stays the API count, null or absent; no count is invented |
| `pr view N --comments --json` | `comment_details` when discussion is requested | `comments` stays the API count, null or absent |
| `pr comment list N --json` | `comments` in the object | Each comment object retains API fields |
| `pr comment N --json` | Created/edited object, unchanged | Mutation response retains API fields |

The API count and fetched array are separate observations and can disagree if comments change between requests. Arrays are ordered by comment ID ascending, with pagination/deduplication from the shared reader. A failed later page discards the incomplete source. A PR comment read can still return the other source with its coverage. A collision with a field the CLI needs to add causes an explicit composition error and preserves the original server field. View without `--comments` or raw `gb api` remains available to inspect such a response.

`tests/fixtures/json/issue-with-comments.json` and `pr-with-comments.json` pin the ordinary augmented object shapes. PR comment list always includes `review_comments` and `comment_coverage`. PR view with `--comments` always adds coverage and includes the requested arrays. For PR reads, an unavailable source is null and has a separate coverage status and reason. Check coverage even when the command exits 0. See [review scope](review-comments.md). API fields such as review_comments_url are retained as data, not interpreted as proof that the advertised route works on the deployed server.

Inline write commands produce a separate verified operation result rather than pretending the Web reply is an ordinary REST Comment. The result carries qualified identity, Web actor/route, saved comparison, verification type, optional review readback and explicit position-group reply semantics. The nested review remains the reader's rendered representation, including its unknown fields. Deletion returns review null, and a failed verification returns only an operation error. `tests/fixtures/json/review-write.json` pins the result envelope, combined with the existing review record fixture in the HTTP regression. See [write JSON and retry contracts](review-writes.md).

### PR metadata

The typed PR projection now includes nullable draft, labels, assignees and merged_by. Text view shows reported metadata and marks open draft PRs DRAFT in view/list. Missing/null draft is not declared false in JSON or shown as an explicit Draft: no in text. Empty reported labels/assignees display (none). Merged state takes precedence over the draft badge. Unknown metadata remains available to JSON consumers without requiring another CLI model change.

### Validation and upstream baseline

The JSON regressions compare complete objects, not only selected known fields. They cover nested unknown data, Unicode, missing/null/false, empty arrays, canonical-versus-summary precedence, count-versus-array shape, pagination/errors, reserved field collisions, PR-create wrappers and comment mutations. Existing write-payload tests guard the separation between responses and requests.

The live 4.44.0 case compares CLI view JSON with direct REST objects for an Issue and a real draft PR, then compares discussion arrays and their source responses. It runs at root and /gitbucket. The baseline Issue API does not provide a comments count: absence is preserved even after comment_details is added.

Primary response definitions: [ApiIssue](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiIssue.scala), [ApiPullRequest](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiPullRequest.scala), [ApiComment](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiComment.scala). These fixtures cover metadata, IDs and resource URLs.

## Errors and exit status

`--json-errors` changes errors on stderr independently of successful command data on stdout. It applies to clap argument validation and runtime errors. Set `GB_JSON_ERRORS=true` (also 1/yes/on) to enable it through the environment; false/0/no/off disables the environment default. A standalone flag enables it even after an invalid argument, provided it appears before the `--` terminator. Flag text inside a value such as `--body=--json-errors`, an attached short-option value or a larger body string does not enable it. `--json-errors` after `--` is positional data. Help and version remain normal text on stdout with exit 0.

```sh
gb --json-errors pr view nope
# stderr; exit 2
```

```json
{"error":{"code":"invalid_arguments","message":"Invalid CLI arguments: invalid value for one of the arguments. Run `gb <command> --help` for usage.","cause":"invalid_value","exit_code":2}}
```

Argument errors report the error kind without echoing raw argument values. Normal text mode keeps clap's detailed usage errors. `cause` distinguishes invalid_value, unknown_argument, invalid_subcommand, missing_argument, missing_subcommand, argument_conflict, invalid_utf8 and invalid_value_count; other parser errors use argument_error.

Runtime errors retain their existing `code`, optional `cause`, HTTP `status`, mutation `outcome` and per-operation metadata. `exit_code` is the CLI status, not a child process status. Messages and diagnostic operation/actor strings are bounded and redact configured environment credentials and an explicit login token. Transport/Git adapters keep their existing additional redaction. Invalid configuration TOML is reported without echoing source lines. Original API data on successful stdout is not filtered or rewritten by error redaction. Login tokens from stdin or prompts are available to the final reporter; known credentials are also redacted from runtime errors in normal text mode.

### Prompts, subprocesses and exit status

With JSON errors enabled, interactive input returns `input_required` with `cause: interactive_input_disabled`, exit 1, before invoking a prompt. Supply the required options and explicit confirmations (`--yes` where supported). A Web sign-in that needs a password returns `web_password_required` when `GB_PASSWORD` is unset or empty and a prompt is unavailable. Supply `GB_PASSWORD` for the authorized operation. Explicit `gb api --input -` stdin reading remains available and cancellable.

The existing Git adapter captures subprocess stdout/stderr. JSON-error mode suppresses notices and raw Git/pager stderr, leaving one error record. A pager failure returns `pager_failed`, `cause: nonzero_exit`, exit 1, and `child_exit_code` when the OS supplies a numeric status. A signal termination can omit that child code. Other subprocess failures keep their existing error codes.

| CLI exit | Meaning |
| --- | --- |
| 0 | Successful command, help/version, or an otherwise successful stdout pipeline whose consumer closed early. |
| 1 | Runtime/authentication/cancellation/output/pager failure. |
| 2 | Argument parsing or validation failure. |

Status JSON may coexist with unsuccessful runtime status (for example auth status prints its record before a failed authentication probe). Mutation metadata still distinguishes confirmed, failed, unverified and unattempted effects. A closed stdout never turns an existing runtime or argument failure into success. If writing auth status JSON fails after a rejected probe or missing credentials, the original authentication error takes precedence over that output error. When authentication succeeds, a non-pipe stdout failure still reports io_error.

For example, on Linux, if the selected token is rejected with HTTP 401:

```sh
gb --json-errors auth status --json > /dev/full
# stderr retains error.code=api_error, error.status=401 and error.exit_code=1
```

### Output and pager lifecycle

Text macros, tables, direct JSON, generated completion and streamed output use the common stdout writer. Writes are flushed so failures do not surface only during process shutdown. BrokenPipe ends output without a panic or diagnostic; other I/O errors are reported as io_error, exit 1. Print-style functions defer a write failure to command dispatch so command cleanup still runs and a prior runtime error takes precedence. Closing stdout does not undo completed server changes or promise that earlier output was fully consumed.

Completion is generated into memory before writing because the generator's writer contract assumes successful writes. Help/version use clap's fallible printer with the same BrokenPipe/other-I/O exit policy.

Terminal paging keeps the existing PAGER selection (default `less -R`), whitespace argument splitting, and direct-output fallback for empty/unstartable programs. Redirected stdout or `--no-pager` bypasses paging. Pager stdin is closed after copying, and the child is waited/reaped even if it closes input early. Exit 0 after early closure is normal; a nonzero exit is now an error instead of being silently ignored. Cancellation keeps the existing kill/reap behavior during blocked writes or waits. JSON-error mode discards the pager's stderr so its diagnostics cannot corrupt the error record. A user-selected pager still owns its stdout and terminal behavior.

## Collection pagination

The collection reader lives in `src/api/pagination.rs`. Endpoint wrappers select an explicit contract instead of inferring pagination from an arbitrary row count. The contracts are pinned to GitBucket 4.44.0 and tested directly over REST in `contract_fixed_pages_all_comments_and_canonical_fork_identity`.

| Collection | No Link header | Order returned by gb |
| --- | --- | --- |
| Issues | Fixed 25-row pages; request page 2, 3, ... until a short/empty page | Number descending |
| Pull requests | Same fixed 25-row pages | Number descending |
| Issue/PR discussion comments | One response contains all comments, regardless of count | Comment ID ascending |

GitBucket ignores `per_page` on these routes. The Issue collection excludes PRs; it cannot repair an incomplete PR listing. The CLI does not query Issues as a fallback for PR lists or existing-PR detection. Other collections and raw `gb api` retain their endpoint behavior; this is not a global pagination heuristic.

For servers that provide Link metadata, `rel=next` takes precedence. Multiple header fields and relation lists are accepted. Once pagination Link relations (`next`, `prev`, `first`, `last`) are present, absence of next ends traversal, even if the terminal page has 25 rows. Unrelated relations such as `canonical`, `self`, or `describedby` do not disable the fixed-page contract. Absolute, root-relative, path-relative and query-relative targets resolve against the actual response URL. Every next target must stay inside the configured API base and the same collection endpoint, without credentials or fragments. Omitted initial filters such as `state=closed` are restored; conflicting filters fail.

Rows are deduplicated by Issue/PR number (the API's dummy `id` is unsuitable), or comment ID, retaining the first observation. Final ordering is explicit because the server need not preserve the order within a page. Zero IDs, cyclic URLs, nonempty pages without any new IDs (including terminal pages), malformed/ambiguous next Links, unexpected no-Link Issue/PR pages larger than 25, and an exhausted safety cap are errors. HTTP and decode failures retain their transport error code, cause and status. The command returns no partial JSON array/table and does not decide that a PR is absent after an incomplete search. Diagnostics do not include next-Link URLs.

`GB_PAGINATION_MAX_PAGES` defaults to 1000, accepts integers from 1 to 100000, and bounds each collection traversal. At an exact 25-row boundary, a final empty request may be needed and counts towards this cap. Hitting the cap with another page pending fails with `pagination_incomplete`, cause `page_limit`; it is not a display limit. Other pagination causes include `pagination_cycle`, `pagination_no_progress`, `invalid_item_id`, `unknown_pagination`, `invalid_link_header`, `ambiguous_next_link`, `unsafe_next_link` and `changed_filter`. No `--limit` is introduced: display and internal searches both finish traversal before succeeding. A future display limit must remain separate from the complete search required before PR creation.

### Canonical PR identity

The 4.44.0 PR collection can report a fork's head repository and label as belonging to the base repository. `pr list --json` therefore fetches every listed PR's canonical detail, for every state filter. A missing, failed or wrong-number detail fails the list; metadata is not silently left unverified. Table output uses summary fields and avoids these extra detail requests.

`pr create --detect-existing` first completes the open PR collection. Different head/base branch refs can exclude candidates; collection ownership cannot. Possible candidates always require canonical detail before comparing repository identity and refs. Missing or conflicting identities fail before creating a PR. If create fails, the same complete check can recover a matching open PR; a failed recheck preserves the original create error/status/outcome with added context. Cancellation before recheck exits without starting it. Cancellation during a running recheck retains the original POST error/status/outcome and adds context that the recheck was cancelled. Lookup diagnostics never assert that an earlier create could not have succeeded. The POST is never retried.

Ordinary numbered PR commands require a detail object and verify its number and any reported base repository. A separate [saved-snapshot reader](pull-request-snapshots.md) can recover the verified 200/empty deleted-fork detail response using saved SHAs from the PR collection and retained Git objects. It verifies the saved comparison independently of the collection's fork ownership metadata.

Traversal is not a snapshot: concurrent creation/deletion/state changes can move rows between pages. Deduplication removes repeated observations but cannot recover records the server never exposes. Detection and creation are not atomic, so this option cannot guarantee uniqueness under concurrent writers. See [review comments](review-comments.md) for the separate inline and commit comment source.

### Upstream evidence

- [Issue API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueControllerBase.scala) and [PR API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiPullRequestControllerBase.scala) pass the requested page into the search service.
- [IssuesService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/IssuesService.scala) selects Issues versus PullRequests, slices with PullRequestLimit and joins the base repository for the collection; [PullRequestService](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/service/PullRequestService.scala) defines the 25-row limit and retrieves canonical head details.
- [Comment API](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/controller/api/ApiIssueCommentControllerBase.scala) returns the complete comment collection without page/per_page slicing.
- [ApiPullRequest](https://github.com/gitbucket/gitbucket/blob/4.44.0/src/main/scala/gitbucket/core/api/ApiPullRequest.scala) constructs head repository metadata and label from the supplied repository.
