# Review Notes

Last updated: 2026-03-20

## Findings

1. High - Auth config stores tokens in plain text with default file permissions, which can expose PATs to other local users on Unix-like systems.
   Status: fixed
2. High - Default host selection is nondeterministic when multiple hosts are configured because it depends on `HashMap` iteration order.
   Status: fixed
3. Medium - UTF-8 truncation can panic when repository names, issue titles, or descriptions contain multibyte characters.
   Status: fixed
4. Medium - `gb pr checkout` and `gb pr diff` do not handle fork-based pull requests correctly because they assume the head branch exists on `origin`.
   Status: fixed
5. Medium - `gb issue list --state` and `gb pr list --state` accept a filter that is never applied.
   Status: fixed
6. Medium - `GB_TOKEN` forces `https` for plain hostnames, which breaks HTTP-only local GitBucket instances.
   Status: partially fixed via protocol inference from URL schemes and `GB_PROTOCOL` support for plain hosts

## Verification

- `cargo check`
- `cargo test`
- `cargo fmt --all`
