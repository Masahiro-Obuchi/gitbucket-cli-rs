# Authentication, profiles and instance keys

Runtime credentials and settings operations have different lookup rules.
Runtime authentication selects `GB_TOKEN`, then the selected profile's matching
credential, then a matching global credential. Management operations (`config
get/set/unset host`, default-host and logout) address only their selected scope.
They never silently edit or remove a global fallback credential.

Scope follows `--profile`, `GB_PROFILE`, then saved `default_profile`. Auth and
config commands accept `--global` to bypass the saved default profile. Combining
it with an explicit `--profile` or `GB_PROFILE` is an error. Login can create a
new profile; other scoped operations require an existing profile. Named profile
operations and default-profile operate on the profile registry. `config list`
always shows the full saved inventory without tokens.

```sh
gb config get host --host https://bucket.example/gitbucket --profile work
gb config set host --host https://bucket.example/gitbucket --user alice --profile work
gb config get host --host https://bucket.example/gitbucket --global
gb config get default-repo --profile work
gb config unset default-host --profile work
gb config unset default-repo --profile work
gb config unset host --host https://bucket.example/gitbucket --profile work
gb config unset profile personal
gb auth logout --profile work -H https://bucket.example/gitbucket
gb auth logout --global -H https://bucket.example/gitbucket
```

Profile logout keeps the profile's default target, so remaining global fallback
can still be selected and diagnosed. It preserves other profiles, global entries
and environment variables. Logout reports the remaining possibility of fallback
and the presence of `GB_TOKEN`. Removing a profile also clears default-profile
when it points to that profile; it does not delete global or other profiles'
credentials. Clearing profile default-host instead removes that routing choice.
Removing the global default credential clears the global default. Set another
default explicitly with `gb config set default-host` when needed.

## Verified status and offline inventory

```sh
gb auth status --profile work --json
gb auth status --offline --profile work --json
gb auth status --global --json
```

Status normally sends one `GET /api/v3/user` to the selected instance with the
effective token. Other saved hosts/profiles are inventory and receive no probes.
The selected profile can use global fallback, and an environment-only token and
`GB_HOST` work without a saved login. A stale stored user or `GB_USER` cannot
establish ownership of a token. Web operations with an environment token and no
username hint use the verified REST login; explicit Web actor rules still apply.
Git HTTP authentication with `GB_TOKEN` requires `GB_USER`; a saved user's name
is no longer inherited as the username for an unrelated environment token.

JSON retains `active_profile`, `effective_actor`, `hosts` and `profiles`, and adds
`offline`. The selected actor contains `host`, normalized `instance`, nullable
`user`, nullable `stored_user`, `protocol`, `credential_source`, `verification`
and nullable `error` (`code`, `status`, `cause`). `user` is populated only after
verification; `stored_user` is an unverified saved hint and is null for an
environment token. The active profile's actor mirrors the selected result;
inactive profiles have no effective actor. Inventory `user` fields remain saved
hints and are labelled `verification: not_checked`.

Verification is `verified`, `offline`, `rejected` (401/403), or `unverified`
(transport, decoding or other server failures). Offline mode makes no request
and leaves `user: null`. Online verification failure or missing effective
credentials exits 1; `--json` still prints the status record before the error on
stderr. Offline inventory exits 0 when readable, even without credentials.
Status never includes tokens. This changes the previous offline-by-default
behavior and the old assumption that effective_actor.user was a saved username.

## Scheme and instance matching

Saved host keys and default-host values are normalized HTTP(S) base URLs.
The URL supplies the scheme. HTTP and HTTPS, custom ports and deployment paths
identify distinct instances. CLI inputs normalize host case, default ports,
trailing slashes and the optional `/api/v3` suffix.

For a bare runtime target, `GB_PROTOCOL` selects the scheme when set. Otherwise,
the CLI checks for a unique match in the selected credential scope, then the
global scope when a profile is active. A bare target that matches both schemes
in the same scope requires a full URL. An environment token with no saved
match defaults to HTTPS.

`config set default-host` and `config set host --default` save the selected
credential's canonical URL. `config set profile --default-host` also accepts a
URL without a matching credential. A bare input to that command uses a unique
saved instance or HTTPS if no credential matches. `config get host --field
protocol` displays the scheme derived from the URL.

```text
$ gb config set default-host http://bucket.example/gitbucket --profile work
✓ Set default host to http://bucket.example/gitbucket
```

For configurations with bare host keys or defaults, set `GB_CONFIG_DIR` to a
fresh directory, run `gb auth login` with the full deployment URL, and recreate
any profile defaults you use.
