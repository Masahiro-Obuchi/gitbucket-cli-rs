# Explicit input and human display

PR-16 changes the defaults for noninteractive creation and PR base selection.
It adds no automatic push or editor process.

## Text input

`issue create/edit/comment`, `pr create/edit/comment`, and inline PR
create/edit/reply accept `--body-file FILE` or `--body-file -` for UTF-8 stdin.
The option conflicts with `--body`, including an explicitly empty `--body`.
Only the selected source is consumed. Stdin is never implicitly used as a body,
and no command combines body input and token input from the same stream.
Invalid UTF-8, unreadable files and input cancellation fail before mutation.

REST bodies retain leading/trailing spaces, LF/CRLF and final newlines exactly
as supplied; input does not trim or render Markdown. An empty issue/PR description
is valid, including an empty file used to clear a description on edit. Ordinary
and inline comments must contain non-whitespace text. Inline writes retain the
existing GitBucket Web form restrictions and trimming described in
[review writes](REVIEW_WRITES.md); they do not promise byte preservation by Web.

With terminal stdin and stderr, creation may prompt for missing title/body and
comments may prompt for missing body. Without those terminals, or with
`--json-errors`, prompts do not start: missing required input returns
`input_required` / exit 1, with an option hint. Create requires a title; its body
is optional and omitted when unspecified. Use `--body ''` to skip an optional
body prompt at a terminal. `--detect-existing` may return an existing PR without
a title. Existing explicit confirmation flags still apply to destructive commands.

```sh
gb issue create --title 'Bug report' --body-file report.md
printf '## Details\n\nMore text\n' | gb pr comment 5 --body-file -
gb pr edit 5 --body-file empty.md
gb auth login --hostname https://gitbucket.example.com/gitbucket --with-token < token.txt
```

`auth login --with-token` reads one nonempty UTF-8 token from stdin with an
optional trailing LF or CRLF. Embedded whitespace/control characters, extra
lines and inputs over 64 KiB are rejected without echoing the input. The token
is verified with `/user` and saved using the existing protected config writer.
It conflicts with the retained `--token/-t` compatibility option. Noninteractive
login also requires an explicit hostname; stdin reads remain cancellable.
Known login tokens are carried through final runtime-error reporting for stdin,
argv and prompted input. Both text and JSON diagnostics redact them, including
context added by the command after the HTTP layer has already cleaned an error.

## PR creation

When `--base` is omitted, `gb` reads the target repository's `default_branch`.
It does not prompt with `main` or guess a fallback. Missing metadata requires
`--base`; a lookup error remains an error. Explicit `--base` remains supported.

An omitted `--head` selects the current local branch. Before creating, `gb`
compares local HEAD's full commit ID with the live `refs/heads/BRANCH` advertised
by the head repository, using the existing scoped Git authentication and timeouts.
It does not trust potentially stale upstream tracking refs. Missing, ahead,
behind or diverged heads fail before POST. `--head-owner` checks that owner's
same-named repository. Detached HEAD requires explicit `--head`.

A matching commit and the base are reported on stderr (suppressed in JSON-error
mode). Uncommitted/untracked changes produce a notice because they cannot be
included. No branch, tracking ref, tag, index, working tree or FETCH_HEAD is
updated by this check, and no fetch/push occurs.

Explicit `--head BRANCH` / `--head OWNER:BRANCH` deliberately selects a published
branch without requiring a local checkout or matching local commits. Use it when
the remote content is intentional. This check is advisory protection around
creation, not an atomic server guard: another writer can move a branch after
the comparison. Existing-PR lookup is read-only and may return before this check.

## Display

Tables measure Unicode display columns. Truncation preserves whole grapheme
clusters, including combining marks and emoji sequences. Table cells escape
newlines/tabs, CR, C0/C1 controls and bidi formatting controls as visible text.
Scalar titles also escape line breaks; multiline bodies preserve LF and tabs.

Human stdout/stderr and pager text allow only basic ANSI colors, bold/dim and
their resets (SGR 0–2, 22, 30–37, 39, 40–47, 49, 90–97, 100–107).
Other escapes, including cursor commands, OSC hyperlinks, conceal/blink, CR
overwrites and bidi overrides are displayed literally. This applies even without
a TTY. Terminal/font Unicode widths can still differ from the Unicode tables.

JSON retains the original values, escaped by the JSON serializer rather than
rewritten for display. Completion and raw PR diff streams also bypass human
display conversion so they remain usable as shell code and patches. Use
`gb api` or JSON for exact REST content. Existing pager and BrokenPipe behavior
is unchanged.
