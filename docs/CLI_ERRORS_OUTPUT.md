# CLI errors and stdout

`--json-errors` changes errors on stderr independently of successful command data
on stdout. It applies to clap argument validation and runtime errors. Set
`GB_JSON_ERRORS=true` (also 1/yes/on) to enable it through the environment;
false/0/no/off disables the environment default. A standalone flag enables it
even after an invalid argument, provided it appears before the `--` terminator.
Flag text inside a value such as `--body=--json-errors`, an attached short-option
value or a larger body string does not enable it. `--json-errors` after `--` is
positional data. Help and version remain normal text on stdout with exit 0.

```sh
gb --json-errors pr view nope
# stderr; exit 2
```

```json
{"error":{"code":"invalid_arguments","message":"Invalid CLI arguments: invalid value for one of the arguments. Run `gb <command> --help` for usage.","cause":"invalid_value","exit_code":2}}
```

Argument errors report the error kind without echoing raw argument values.
Normal text mode keeps clap's detailed usage errors. `cause` distinguishes
invalid_value, unknown_argument, invalid_subcommand, missing_argument,
missing_subcommand, argument_conflict, invalid_utf8 and invalid_value_count;
other parser errors use argument_error.

Runtime errors retain their existing `code`, optional `cause`, HTTP `status`,
mutation `outcome` and per-operation metadata. `exit_code` is the CLI status,
not a child process status. Messages and diagnostic operation/actor strings are
bounded and redact configured environment credentials and an explicit login
token. Transport/Git adapters keep their existing additional redaction. Invalid
configuration TOML is reported without echoing source lines. Original API data
on successful stdout is not filtered or rewritten by error redaction.
Login tokens from argv, stdin or prompts are available to the final reporter;
known credentials are also redacted from runtime errors in normal text mode.

## Prompts, subprocesses and exit status

With JSON errors enabled, interactive input returns `input_required` with
`cause: interactive_input_disabled`, exit 1, before invoking a prompt. Supply
required title/body/credentials and explicit confirmations (`--yes` where
supported). This includes Web-password prompts: supply GB_PASSWORD for a
permitted Web operation. Explicit `gb api --input -` stdin reading remains
available and cancellable. Normal text mode keeps prompts and cancellation.
This changes prior JSON-error behavior on an interactive terminal, where prompt
control sequences could be interleaved with the error record.

The existing Git adapter captures subprocess stdout/stderr. JSON-error mode
suppresses notices and raw Git/pager stderr, leaving one error record. A pager
failure returns `pager_failed`, `cause: nonzero_exit`, exit 1, and
`child_exit_code` when the OS supplies a numeric status. A signal termination can
omit that child code. Other subprocess failures keep their existing error codes.

| CLI exit | Meaning |
| --- | --- |
| 0 | Successful command, help/version, or an otherwise successful stdout pipeline whose consumer closed early. |
| 1 | Runtime/authentication/cancellation/output/pager failure. |
| 2 | Argument parsing or validation failure. |

Status JSON may coexist with unsuccessful runtime status (for example auth status
prints its record before a failed authentication probe). Mutation metadata still
distinguishes confirmed, failed, unverified and unattempted effects. A closed
stdout never turns an existing runtime or argument failure into success.
If writing auth status JSON fails after a rejected probe or missing credentials,
the original authentication error takes precedence over that output error.
When authentication succeeds, a non-pipe stdout failure still reports io_error.

For example, on Linux, if the selected token is rejected with HTTP 401:

```sh
gb --json-errors auth status --json > /dev/full
# stderr retains error.code=api_error, error.status=401 and error.exit_code=1
```

## Output and pager lifecycle

Text macros, tables, direct JSON, generated completion and streamed output use
the common stdout writer. Writes are flushed so failures do not surface only
during process shutdown. BrokenPipe ends output without a panic or diagnostic;
other I/O errors are reported as io_error, exit 1. Print-style functions defer a
write failure to command dispatch so command cleanup still runs and a prior
runtime error takes precedence. Closing stdout does not undo completed server
changes or promise that earlier output was fully consumed.

Completion is generated into memory before writing because the generator's
writer contract assumes successful writes. Help/version use clap's fallible
printer with the same BrokenPipe/other-I/O exit policy.

Terminal paging keeps the existing PAGER selection (default `less -R`), whitespace
argument splitting, and direct-output fallback for empty/unstartable programs.
Redirected stdout or `--no-pager` bypasses paging. Pager stdin is closed after
copying, and the child is waited/reaped even if it closes input early. Exit 0
after early closure is normal; a nonzero exit is now an error instead of being
silently ignored. Cancellation keeps the existing kill/reap behavior during
blocked writes or waits. JSON-error mode discards the pager's stderr so its
diagnostics cannot corrupt the error record. A user-selected pager still owns
its stdout and terminal behavior.
