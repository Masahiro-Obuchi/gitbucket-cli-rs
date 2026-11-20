# Input and terminal display

Use `gb <command> -h` for a quick option list and `--help` for additional usage notes.

## Text input

`issue create/edit/comment`, `pr create/edit/comment`, and inline PR create/edit/reply accept `--body-file FILE` or `--body-file -` for UTF-8 stdin. The option conflicts with `--body`, including an explicitly empty `--body`. Only the selected source is consumed. Stdin is never implicitly used as a body, and no command combines body input and token input from the same stream. Invalid UTF-8, unreadable files and input cancellation fail before mutation.

REST bodies retain leading/trailing spaces, LF/CRLF and final newlines exactly as supplied; input does not trim or render Markdown. An empty issue/PR description is valid, including an empty file used to clear a description on edit. Ordinary and inline comments must contain non-whitespace text. Inline writes retain the existing GitBucket Web form restrictions and trimming described in [review writes](../reference/review-writes.md); they do not promise byte preservation by Web.

With terminal stdin and stderr, creation may prompt for missing title/body and comments may prompt for missing body. Without those terminals, or with `--json-errors`, prompts do not start: missing required input returns `input_required` / exit 1, with an option hint. Create requires a title; its body is optional and omitted when unspecified. Use `--body ''` to skip an optional body prompt at a terminal. `--detect-existing` may return an existing PR without a title. Existing explicit confirmation flags still apply to destructive commands.

```sh
gb issue create --title 'Bug report' --body-file report.md
printf '## Details\n\nMore text\n' | gb pr comment 5 --body-file -
gb pr edit 5 --body-file empty.md
gb auth login --hostname https://gitbucket.example.com/gitbucket --with-token < token.txt
```

`auth login --with-token` reads one nonempty UTF-8 token from stdin with an optional trailing LF or CRLF. Embedded whitespace/control characters, extra lines and inputs over 64 KiB are rejected without echoing the input. The token is verified with `/user` and saved using the existing protected config writer. Noninteractive login also requires an explicit hostname. Stdin reads remain cancellable. Known login tokens from stdin and prompts are carried through final runtime-error reporting. Both text and JSON diagnostics redact them, including context added by the command after the HTTP layer has already cleaned an error.

## Explicit editor input

`--editor/-e` is available anywhere Issue/PR description or comment bodies accept `--body`/`--body-file`, including inline create/edit/reply. These three input sources are mutually exclusive, even when `--body` is empty. An editor is never launched just because a body is missing. Existing prompt/file/stdin behavior remains the default. Editor input requires terminal stdin and stderr and is disabled by `--json-errors`; rejected input returns `input_required` before starting an editor or accessing the API.

Selection uses the first **set** variable among `GB_EDITOR`, `VISUAL`, `EDITOR`, then `vi` on Unix or `notepad.exe` on Windows. A set but empty, invalid UTF-8 or incorrectly quoted setting fails instead of choosing another editor. Commands use POSIX-style argument quoting on all platforms, without a shell, expansion, redirection or command substitution. Quote executable paths/arguments containing spaces; use single quotes around Windows paths to preserve backslashes. The body file is appended as one argument. GUI editors must wait for the document to close, for example `GB_EDITOR='code --wait'`.

New descriptions/comments/replies start empty. Description edits preload the current API body; `--edit-last` preloads the authenticated user's latest ordinary comment. Inline edits preload the permission-checked raw Markdown after target, author and saved-position validation. These reads and Web authentication can precede editing; content changes cannot. Editing does not lock the server record or add atomic expected-head/body guarantees: concurrent updates can still race.

Exit zero with changed UTF-8 text submits that text through the existing route. Whitespace, CRLF/LF and final newlines are retained for REST; inline Web trimming and restrictions still apply. Deleting an existing description to an empty file clears it; blank comments remain invalid. Closing without changing bytes cancels the whole command, including other requested metadata changes, with exit 1 and `cancelled`. Use `--body ''` for an explicitly empty new description.

An unsuccessful exit or launch/command/content failure returns an error without submitting content; process/config/content failures use `editor_failed`, with the child status retained for nonzero exits. Ctrl+C/SIGTERM cancels editing, terminates and reaps the selected process, then releases its temporary file. Unix editors retain the foreground terminal process group; the editor is responsible for its own background helpers. Windows additionally supervises the process job. There is no human-editing deadline and no automatic retry. Unix terminal modes are saved before launch and restored after the selected process is stopped/reaped, on success, failure and cancellation. Missing, unreadable or nonregular edited files report `editor_failed` with cause `invalid_body`. For a missing file the diagnostic is `Editor failed: The edited body must be a readable regular file.`

The body lives in a random temporary directory, never a workspace file. On Unix the directory is mode 0700 and the initial file mode 0600. Windows inherits the temporary parent's ACL; use a private user temporary directory. The initial handle closes before editor launch so replacement saves work on Windows too. After exit, only a regular file is read; symlinks/reparse points and special files are rejected. Temporary data is removed on success, failure and cancellation where the filesystem permits cleanup; abrupt termination can leave private temporary data. Editor backup files within that directory are cleaned with it. The editor is a trusted local program and may manage its own other files.

Editor stdin/stderr use the terminal, and its stdout is directed to terminal stderr so `--json` output stays clean. The editor controls its own terminal UI; gb's display sanitization applies to gb's own output. No body text or command arguments are included in gb's editor-failure diagnostics. For example:

```text
Editor failed: The editor exited unsuccessfully; no body was submitted.
```

```sh
GB_EDITOR='code --wait' gb issue create --title 'Bug report' --editor
gb pr edit 5 --editor
gb pr comment 5 --edit-last --editor
gb pr comment edit 5 --kind inline --id 12 --web --editor
```

## PR creation

When `--base` is omitted, `gb` reads the target repository's `default_branch`. It does not prompt with `main` or guess a fallback. Missing metadata requires `--base`; a lookup error remains an error. Explicit `--base` remains supported.

An omitted `--head` selects the current local branch. Before creating, `gb` compares local HEAD's full commit ID with the live `refs/heads/BRANCH` advertised by the head repository, using the existing scoped Git authentication and timeouts. It does not trust potentially stale upstream tracking refs. Missing, ahead, behind or diverged heads fail before POST. `--head-owner` checks that owner's same-named repository. Detached HEAD requires explicit `--head`.

A matching commit and the base are reported on stderr (suppressed in JSON-error mode). Uncommitted/untracked changes produce a notice because they cannot be included. No branch, tracking ref, tag, index, working tree or FETCH_HEAD is updated by this check, and no fetch/push occurs.

Explicit `--head BRANCH` / `--head OWNER:BRANCH` deliberately selects a published branch without requiring a local checkout or matching local commits. Use it when the remote content is intentional. This check is advisory protection around creation, not an atomic server guard: another writer can move a branch after the comparison. Existing-PR lookup is read-only and may return before this check.

## Display

Tables measure Unicode display columns. Truncation preserves whole grapheme clusters, including combining marks and emoji sequences. Table cells escape newlines/tabs, CR, C0/C1 controls and bidi formatting controls as visible text. Scalar titles also escape line breaks; multiline bodies preserve LF and tabs. This includes Issue edit confirmations and Milestone view/create/edit output, including a repeated state change that needs no mutation. For example, a title containing a newline and a tab appears as:

```text
✓ Updated issue #1: title\nforged\tfield
```

Only the displayed title is escaped; request payloads retain the original value.

Human stdout/stderr and pager text allow only basic ANSI colors, bold/dim and their resets (SGR 0–2, 22, 30–37, 39, 40–47, 49, 90–97, 100–107). Other escapes, including cursor commands, OSC hyperlinks, conceal/blink, CR overwrites and bidi overrides are displayed literally. This applies even without a TTY. Terminal/font Unicode widths can still differ from the Unicode tables.

JSON retains the original values, escaped by the JSON serializer rather than rewritten for display. Completion and raw PR diff streams also bypass human display conversion so they remain usable as shell code and patches. Use `gb api` or JSON for exact REST content. See [pager and pipe behavior](../reference/automation.md#output-and-pager-lifecycle).
