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

Instance keys normalize scheme, host case, default port, trailing slash and the
optional `/api/v3` suffix. Ports, deployment paths and HTTP versus HTTPS remain
distinct. Legacy bare keys use their stored protocol; an explicit URL's scheme
defines its actual instance even if its legacy protocol field disagrees.

A bare runtime target uses an explicit URL scheme or `GB_PROTOCOL` when supplied;
otherwise it must identify one saved instance in the selected scope/fallback.
Without stored credentials an environment token defaults to HTTPS. Multiple
HTTP/HTTPS candidates require an explicit scheme. Multiple aliases for one
instance are rejected for credential use until explicitly repaired/migrated,
even when one alias exactly matches the input. No HTTPS credential is selected
for an explicit HTTP request.

New login entries use normalized full URL keys and actual protocols. Re-login
updates an existing unique legacy alias without silently migrating the file.
`config get host` exposes the stored protocol; runtime status reports the actual
instance protocol. `config set host --protocol` can update a legacy bare key;
changing the scheme of an explicit URL requires login to the new URL.

## Explicit migration and guarded restore

```sh
gb config migrate
gb config migrate --apply
gb config migrate --restore /path/from/the/backup/result.json
```

Migration covers the whole file and cannot be combined with --profile,
GB_PROFILE or --global. Preview changes no configuration bytes and creates no
backup. Apply recalculates the complete plan under the ordinary configuration
OS lock, normalizes global/profile keys and default-host references, and retains
unknown TOML fields. It may reformat TOML and remove comments in the migrated
file; the backup retains exact original bytes. Different credentials or differing
unknown fields that would collide at one key stop the entire migration before
any backup/config write. Only identical normalized records are merged. A bare
default without matching saved credentials, or an ambiguous default, is refused;
set an explicit HTTP(S) URL first instead of guessing its scheme.

Apply writes and syncs a unique `config.toml.migration-*.json` backup before
atomically replacing config.toml. The backup contains the original secrets and
a digest of the expected migrated file: protect it like config.toml. On Unix it
uses 0600; on Windows it inherits the parent directory ACL. Config, lock and
backup reads reject symlinks/reparse points and non-regular files. The parent
directory must be trusted. The command reports the backup path and key changes,
without credential values. An already normalized file is a no-op.

Restore acquires the same lock and checks that the current bytes still match
the migrated digest. A later edit refuses automatic restore instead of erasing
new credentials/settings. A retry after an already completed restore is a no-op.
The original bytes are restored atomically; backup files remain available. An
external editor or older client that ignores the lock can still race; filesystem
durability and locking have the same limits as ordinary configuration writes.
