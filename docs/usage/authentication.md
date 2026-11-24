# Authentication and configuration

Runtime credentials and settings operations have different lookup rules. Runtime authentication selects `GB_TOKEN`, then the selected profile's matching credential, then a matching global credential. Management operations (`config get/set/unset host`, default-host and logout) address only their selected scope. They never silently edit or remove a global fallback credential.

Scope follows `--profile`, `GB_PROFILE`, then saved `default_profile`. Auth and config commands accept `--global` to bypass the saved default profile. Combining it with an explicit `--profile` or `GB_PROFILE` is an error. Login can create a new profile; other scoped operations require an existing profile. Named profile operations and default-profile operate on the profile registry. `config list` always shows the full saved inventory without tokens.

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

Profile logout keeps the profile's default target, so remaining global fallback can still be selected and diagnosed. It preserves other profiles, global entries and environment variables. Logout reports the remaining possibility of fallback and the presence of `GB_TOKEN`. Removing a profile also clears default-profile when it points to that profile; it does not delete global or other profiles' credentials. Clearing profile default-host instead removes that routing choice. Removing the global default credential clears the global default. Set another default explicitly with `gb config set default-host` when needed.

## Login and configuration storage

```sh
gb auth login -H https://bucket.example/gitbucket --with-token < token.txt
gb auth login --profile work -H https://bucket.example/gitbucket
gb config set profile work --default-repo alice/project
gb config set default-profile work
gb config list --json
```

`auth login` verifies the token with `/user` and saves the selected instance as the scope's default. For a full URL, its scheme takes precedence over `--protocol`. For a bare hostname, login accepts `--protocol http` or `--protocol https` and defaults to HTTPS. See [token input](input-and-display.md#text-input).

At runtime, the explicit `--hostname/-H`, `GB_HOST`, and the selected scope's saved default host are tried in that order. `--profile`, `GB_PROFILE`, and `default_profile` select the profile in that order. `GB_TOKEN` takes precedence over saved credentials, followed by credentials in the selected profile and global credentials for the selected target.

`gb config path` prints the path to `config.toml`. The file lives under `GB_CONFIG_DIR` when set and in the platform's configuration directory otherwise, such as `~/.config/gb` on Linux. Each host entry stores `token` and `user`, and its URL key carries the scheme.

```toml
default_host = "https://bucket.example/gitbucket"
default_profile = "work"

[hosts."https://bucket.example/gitbucket"]
token = "your-global-token"
user = "alice"

[profiles.work]
default_host = "https://bucket.example/gitbucket"
default_repo = "alice/project"

[profiles.work.hosts."https://bucket.example/gitbucket"]
token = "your-profile-token"
user = "alice"
```

Writers wait up to five seconds for the OS lock, reload the current file, sync a private temporary file, and atomically replace the configuration. Keep the empty `config.toml.lock` file in place so concurrent writers lock the same file. The configuration and lock paths must be regular files. On Unix, the files use mode `0600` and newly created configuration directories use `0700`. On Windows, they inherit access permissions from the parent directory.

## Authentication status

```sh
gb auth status --profile work --json
gb auth status --offline --profile work --json
gb auth status --global --json
```

Status normally sends one `GET /api/v3/user` to the selected instance with the effective token. Use `gb config list` to inspect saved hosts and profiles. The selected profile can use global fallback, and an environment-only token and `GB_HOST` work without a saved login. A stale stored user or `GB_USER` cannot establish ownership of a token. Web operations with an environment token and no username hint use the verified REST login; explicit Web actor rules still apply. Git HTTP authentication with `GB_TOKEN` requires `GB_USER`; a saved user's name is no longer inherited as the username for an unrelated environment token.

For scripts upgrading from 0.7.0, read saved `hosts` and `profiles` from `gb config list --json` and read the verified actor from `gb auth status --json`. The inventory includes saved usernames and default repositories.

Status JSON contains `active_profile`, `offline` and `effective_actor`. The selected actor contains `host`, normalized `instance`, nullable `user`, nullable `stored_user`, `protocol`, `credential_source`, `verification` and nullable `error` (`code`, `status`, `cause`). `user` is populated only after verification; `stored_user` is an unverified saved hint and is null for an environment token.

Verification is `verified`, `offline`, `rejected` (401/403), or `unverified` (transport, decoding or other server failures). Offline mode makes no request and leaves `user: null`. Online verification failure or missing effective credentials exits 1; `--json` still prints the status record before the error on stderr. Offline status exits 0 when the configuration is readable, even without credentials. Status omits tokens.

## Scheme and instance matching

Saved host keys and default-host values are normalized HTTP(S) base URLs. The URL supplies the scheme. HTTP and HTTPS, custom ports and deployment paths identify distinct instances. CLI inputs normalize host case, default ports, trailing slashes and the optional `/api/v3` suffix.

For a bare runtime target, `GB_PROTOCOL` selects the scheme when set. Otherwise, the CLI checks for a unique match in the selected credential scope, then the global scope when a profile is active. A bare target that matches both schemes in the same scope requires a full URL. An environment token with no saved match defaults to HTTPS.

`config set default-host` and `config set host --default` save the selected credential's canonical URL. `config set profile --default-host` also accepts a URL without a matching credential. A bare input to that command uses a unique saved instance or HTTPS if no credential matches. `config get host --field protocol` displays the scheme derived from the URL.

```text
$ gb config set default-host http://bucket.example/gitbucket --profile work
✓ Set default host to http://bucket.example/gitbucket
```

For configurations with bare host keys or defaults, set `GB_CONFIG_DIR` to a fresh directory, run `gb auth login` with the full deployment URL, and recreate any profile defaults you use.


When switching `GB_CONFIG_DIR`, pause inline PR comment writes and copy the private [retry records](../reference/review-writes.md#retry-records) to the new directory while preserving its existing records. If you cannot carry the records over, inspect unresolved submissions before retrying them.

## Web passwords

Some operations need a GitBucket Web session in addition to the REST token. Use `gb auth web-login` to verify the Web account and store its password in the OS credential store. The selected REST token determines the default account. Set `GB_WEB_USER` explicitly to use a different Web account.

```sh
gb auth web-login --hostname https://bucket.example/gitbucket
gb auth web-status --hostname https://bucket.example/gitbucket
gb auth web-status --hostname https://bucket.example/gitbucket --user alice --json
gb auth web-logout --hostname https://bucket.example/gitbucket --user alice
```

`web-login` reads the password from a terminal or from `GB_PASSWORD` if supplied. It saves the password only after verifying the signed-in account. `web-logout` removes the local entry for the full instance URL and Web username and can run without a REST token. Web passwords and REST tokens have independent lifecycles. Profiles that resolve to the same instance and Web username share an entry.

`web-status` checks the OS credential store using the same account selection as `web-login`. Supply `--user` and a full instance URL via `--hostname` or `GB_HOST` to check the store directly, independently of the REST configuration. The output reports the selected account and whether a password is saved, without exposing the password. JSON contains `instance`, `user`, and a Boolean `saved`. A missing entry gives `saved: false` and exit 0. A failed read, such as an unavailable store or locked entry, gives a `credential_store_error` with `--json-errors`. The check ignores `GB_PASSWORD` and does not attempt Web sign-in.

The store is Keychain on macOS, Credential Manager on Windows, and the default Secret Service collection on Linux. Open and unlock the store before registering a password. Linux requires a Secret Service provider in the current D-Bus session. A headless shell can supply `GB_PASSWORD` without a credential store.

When a command needs Web sign-in, it tries `GB_PASSWORD` first, then a stored password, then terminal input if permitted. Automatic reads allow up to three seconds and avoid graphical unlock prompts. Cancelling the read stops Web sign-in. If the store is locked or unavailable, unlock it outside the command or supply `GB_PASSWORD`. With no usable password and no prompt, the command reports `web_password_required`.

## Git authentication

`gb repo clone OWNER/REPO`, `gb pr checkout`, and `gb pr diff` reuse the selected GitBucket user/token for HTTP(S) Git URLs on the same scheme, host, port, and configured base path. With `GB_TOKEN` alone, also set `GB_USER` for private Git access. SSH and other clone origins use Git's own authentication.

Credentials are supplied by a process-local Git credential helper, not placed in Git command arguments, clone URLs, or Git configuration files. The helper checks the exact URL Git requests, including after `url.*.insteadOf` rewriting, and does not save credentials through existing helpers. Token-bearing commands allow only HTTP(S) transports, even if a local URL rewrite enables another transport. Token-bearing HTTP transfers do not follow redirects, including after URL rewriting; configure the direct clone URL if a proxy redirects it. The token is passed to the helper in the Git subprocess environment, so this does not protect against a local process with permission to inspect that environment.

Full-URL `gb repo clone` uses Git's own credentials. HTTP URLs containing passwords are rejected; use a credential helper or `OWNER/REPO` with `gb auth login` instead. Existing HTTP remote userinfo is removed from the command URL used by PR fetches.
