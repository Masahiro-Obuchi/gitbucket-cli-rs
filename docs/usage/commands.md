# Command examples

Run `gb --help` to see the top-level commands and `gb <command> <subcommand> --help` for accepted options. For sign-in and defaults, see [authentication and configuration](authentication.md). For PR commands and reviews, see [pull requests](pull-requests.md).

| Global option | Purpose |
| --- | --- |
| `-H`, `--hostname HOST_OR_URL` | Select a GitBucket host or base URL |
| `-R`, `--repo OWNER/REPO` | Select the target repository |
| `--profile NAME` | Use a named configuration profile |
| `--json-errors` | Print structured failures on stderr |
| `-h`, `--help` | Show available options |
| `-V`, `--version` | Show the binary version |

## Repository selection

Repository selection uses `-R/--repo`, then `GB_REPO`, then the active profile's `default_repo`. If none is set, `gb` reads `git remote get-url origin` and checks it against the selected GitBucket instance before making any API request. HTTP(S) origin URLs must match its scheme, host, port, and deployment path. SSH URLs must match its hostname (ignoring ASCII case) and deployment path; the SSH port is independent of the HTTP port. Matching clone URLs may end in `/`. A root-level SSH URL cannot identify a path-prefixed instance unambiguously, so specify `-R` in that case. SSH aliases are not expanded.

On a mismatch, select the intended target explicitly:

```bash
gb -H https://gitbucket.example.com/gitbucket -R alice/my-app issue list
```

Explicit/environment/profile repositories do not need to match the local origin. `OWNER/REPO` must contain exactly two nonempty names, without URL delimiters, percent escapes, whitespace, or `.`/`..` components. Letters, digits, `_`, `-`, `.`, `+`, and non-whitespace Unicode names are supported.

For `gb repo view`, `gb repo delete`, and `gb repo fork`, you can also pass `OWNER/REPO` positionally after the subcommand. Deletion still requires an explicit repository.

```bash
cd ~/projects/my-app
gb issue list
```

Supported remote URL formats include:

- `https://gitbucket.example.com/alice/my-app.git`
- `https://gitbucket.example.com/git/alice/my-app.git`
- `https://gitbucket.example.com/gitbucket/alice/my-app.git`
- `https://gitbucket.example.com/gitbucket/git/alice/my-app.git`
- `git@gitbucket.example.com:alice/my-app.git`
- `ssh://git@gitbucket.example.com:29418/alice/my-app.git`

## Repositories

```sh
gb repo list
gb repo view alice/project
gb repo create project --private
gb repo create project --group team
gb repo clone alice/project
gb repo fork alice/project --group team
```

Repository deletion requires an explicit target and confirmation. Use `--yes` for an already confirmed unattended deletion. For fork and delete behavior on the verified server, see [Web operation routes](../reference/gitbucket-compatibility.md#web-operations). See [Git authentication](authentication.md#git-authentication) for HTTP(S) and SSH clone credentials.

## Issues

```sh
gb issue list --state all
gb issue view 7 --comments --no-pager
gb issue create --title 'Bug report' --body-file report.md --label bug
gb issue edit 7 --title 'Updated bug report' --add-label urgent
gb issue comment 7 --body 'Reproduced'
gb issue comment 7 --edit-last --editor
gb issue close 7
gb issue reopen 7
```

Issue lists accept `open`, `closed`, or `all`. Issue metadata and state can require Web authentication on GitBucket 4.44.0. Label-only issue edits use the dedicated REST endpoints. See [body input](input-and-display.md#text-input) and [JSON output](../reference/automation.md#discussion-arrays).

## Label commands

```bash
gb label list
gb label list --json
gb label view bug
gb label create needs-review --color abcdef
gb label edit needs-review --name reviewed
gb label edit reviewed --color 123abc --web
gb label delete reviewed --yes
```

Label creation and rename use REST and verify stored values. Rename supplies the current color when `--color` is omitted. Colors accept six hex digits with an optional `#`. A no-op edit sends no mutation.

GitBucket 4.44 rejects a same-name REST color update with 422. Pass `--web` to permit the verified in-place Web route when that exact limitation is encountered; it checks the numeric ID, name and current color before submitting the form and verifies the same ID and saved values afterward. A server accepting the REST request needs no Web credentials, even with `--web`. Conflicting renames, 404s and other REST failures do not trigger this route. Labels are never deleted/recreated or temporarily renamed to change their color. Unknown HTML layouts or changed forms fail before submission.

Label descriptions are not persisted by the verified 4.44 REST/Web contracts. `--description` and `--remove-description` remain recognized but return an explicit unsupported error before mutation, including mixed rename/description requests. This CLI does not infer write support on another instance from HTTP success or a read-only field. Descriptions returned on reads are still displayed. Server fixes and capability requirements are recorded in [the upstream follow-up](../reference/gitbucket-compatibility.md#label-api).

Issue label edits use individual removals and additive requests, preserving labels outside the requested delta. Issue create/edit label names are matched exactly after comma splitting, including surrounding spaces and Unicode whitespace. Use `bug,urgent` without padding unless the padding belongs to the label name. Additions must already exist in the repository: `gb label create` them first. This preflight avoids the pinned server's implicit creation of misspelled labels. Repeated options are deduplicated; if a label is both removed and added, addition wins. Label-only edits need only API credentials. For mixed metadata/label edits, metadata is applied and verified first, then labels are changed and read back. Failures report confirmed steps and tell you to inspect the issue before retrying. These operations are not atomic; unrelated writers can still race with snapshots. Unsafe dot-segment/path names are rejected before requests.

## Milestone commands

```bash
gb milestone list
gb milestone list --state all --json
gb milestone view 7
gb milestone create v1.0 --description "First release" --due-on 2026-04-01
gb milestone edit 7 --title v1.0.1 --state closed
gb milestone edit 7 --description '' --due-on ''
gb milestone delete 7 --yes
```

Milestone lists accept `open`, `closed`, or `all`. Edits preserve unspecified title, state, description, and the exact existing due timestamp. Use `--description ''` or `--due-on ''` to clear a field. Explicit `--due-on` values accept `YYYY-MM-DD` or RFC3339 and use the calendar-date portion at midnight UTC for REST; leaving the option out does not renormalize stored times.

REST is tried first for create/edit/delete. GitBucket 4.44.0 requires a complete replacement for milestone PATCH and restricts REST titles to at most 100 ASCII letters/digits or `-+_.`. `gb` merges the current values before PATCH, and uses JSON `null` to clear description/due date. An ordinary `404`, missing milestone, or permission failure does not trigger Web sign-in. Delete uses REST only; the pinned server treats deleting an already absent milestone as success.

Japanese and space-containing titles are retained exactly. If REST rejects such a title with its known validation response, `gb` confirms the target still exists (or checks the repository's milestone list for creation), then uses the Web route; this can require `GB_USER` / `GB_PASSWORD`. A server that accepts the title over REST needs no Web credentials. Web create/edit results are checked before success is printed; if verification fails, inspect the milestone before retrying.

The Web form trims leading/trailing characters through U+0020 (ASCII controls and space), and accepts only calendar dates. Other Unicode spacing in nonblank titles and descriptions is retained. Content edits are refused before Web authentication if they would trim text or resend an existing due timestamp without an explicit `--due-on` setting/clear. Set that option explicitly or change to a REST-compatible title. State-only Web actions do not resubmit description or due date. A Web date is interpreted by the server's form; if its timezone produces a different instant, verification reports the mismatch instead of success. Concurrent edits can still race between the initial read and replacement; this is not a server-side conditional update.

## Browser access

```sh
gb browse -R alice/project
gb issue view 7 --web
gb pr view 42 --web
```

## Raw REST API

```sh
gb api user
gb api repos/alice/project
gb api repos/alice/project/issues --input issue.json
gb api repos/alice/project/issues/7 --method PATCH --input -
```

Relative endpoints resolve under `/api/v3`. Absolute URLs must stay within the configured GitBucket API base. `--input FILE` sends JSON from a file, and `--input -` reads stdin. The default method is GET, or POST with `--input`. See [HTTP behavior and deadlines](../reference/automation.md#http-and-git-deadlines).

## Shell completion

```sh
gb completion bash > ~/.local/share/bash-completion/completions/gb
gb completion zsh > ~/.zfunc/_gb
gb completion fish > ~/.config/fish/completions/gb.fish
```

Load the generated file according to your shell's completion configuration. `gb completion --help` lists all supported shells.

## Agent skill example

The [sample GitBucket CLI skill](../../skills/gitbucket-cli/SKILL.md) documents commands for a shell-enabled assistant. Its [command patterns](../../skills/gitbucket-cli/references/command-patterns.md) include authentication, issue and PR workflows.
