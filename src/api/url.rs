use url::Url;

use crate::error::{GbError, Result};

pub(crate) fn normalize_path_or_url(path_or_url: &str, base_url: &str) -> Result<String> {
    let absolute = Url::parse(path_or_url).is_ok();
    let candidate = if absolute {
        path_or_url.to_string()
    } else {
        let path = normalize_relative_api_path(path_or_url, base_url);
        let path = if path.is_empty() || path.starts_with(['/', '?', '#']) {
            path.to_string()
        } else {
            format!("/{path}")
        };
        format!("{base_url}{path}")
    };
    let url = checked_request_url(&candidate, base_url)?;
    Ok(url.to_string())
}

pub(crate) fn normalize_origin_path_or_url(path_or_url: &str, base_url: &str) -> Result<String> {
    let web_base = base_url.trim_end_matches("/api/v3");
    let candidate = if Url::parse(path_or_url).is_ok() {
        path_or_url.to_string()
    } else {
        format!(
            "{}{}",
            web_base,
            normalize_relative_origin_path(path_or_url, web_base)
        )
    };
    Ok(checked_request_url(&candidate, web_base)?.to_string())
}

fn checked_request_url(candidate: &str, allowed_base: &str) -> Result<Url> {
    let target = Url::parse(candidate)?;
    let base = Url::parse(allowed_base)?;
    let refused = || {
        GbError::Other(format!(
        "Refusing request outside configured GitBucket API base or origin {allowed_base}, or with credentials, a fragment, or an ambiguous path"
    ))
    };
    if candidate.chars().any(char::is_control)
        || candidate
            .split(['?', '#'])
            .next()
            .is_some_and(|path| path.contains('\\'))
        || !crate::target::same_origin(&target, &base)
        || !matches!(target.scheme(), "http" | "https")
        || !target.username().is_empty()
        || target.password().is_some()
        || target.fragment().is_some()
        || !path_has_base_prefix(target.path(), base.path())
    {
        return Err(refused());
    }
    // Also check paths interpreted after decoding. A path may be encoded more
    // than once by proxies; labels containing encoded separators remain valid
    // as long as no decoding level escapes the configured boundary.
    let mut path = target.path().to_string();
    let mut base_path = base.path().to_string();
    for _ in 0..8 {
        let decoded = percent_encoding::percent_decode_str(&path)
            .decode_utf8()
            .map_err(|_| refused())?;
        let decoded_base = percent_encoding::percent_decode_str(&base_path)
            .decode_utf8()
            .map_err(|_| refused())?;
        if decoded.contains('\\')
            || decoded.chars().any(char::is_control)
            || decoded_base.contains('\\')
            || decoded_base.chars().any(char::is_control)
            || decoded_base
                .split('/')
                .any(|part| matches!(part, "." | ".."))
        {
            return Err(refused());
        }
        if decoded == path && decoded_base == base_path {
            return Ok(target);
        }
        // Compare both sides at the same decoding level. The path setter
        // normalizes dot segments while keeping ?/# and spaces as path data.
        let mut interpreted = target.clone();
        interpreted.set_path(&decoded);
        let mut interpreted_base = base.clone();
        interpreted_base.set_path(&decoded_base);
        if !path_has_base_prefix(interpreted.path(), interpreted_base.path()) {
            return Err(refused());
        }
        path = decoded.into_owned();
        base_path = decoded_base.into_owned();
    }
    Err(refused())
}

pub(crate) fn normalize_base_url(hostname: &str, protocol: &str) -> Result<String> {
    let input = hostname.trim().trim_end_matches('/');
    let candidate = if input.contains("://") {
        input.to_string()
    } else {
        format!("{}://{}", protocol, input)
    };

    let parsed = Url::parse(&candidate)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || candidate.contains('\\')
    {
        return Err(GbError::Config(
            "Invalid GitBucket base URL. Use HTTP(S) without credentials, query, or fragment."
                .into(),
        ));
    }
    let host_part = match parsed.host() {
        Some(url::Host::Ipv6(addr)) => format!("[{}]", addr),
        Some(host) => host.to_string(),
        None => return Err(GbError::Config("Invalid GitBucket host or URL".into())),
    };

    let mut base = format!("{}://{}", parsed.scheme(), host_part);
    if let Some(port) = parsed.port() {
        base.push_str(&format!(":{}", port));
    }

    let path = parsed.path().trim_end_matches('/');
    if !path.is_empty() && path != "/" {
        base.push_str(path);
    }

    if base.ends_with("/api/v3") {
        Ok(base)
    } else {
        Ok(format!("{}/api/v3", base))
    }
}

pub(crate) fn normalize_web_base_url(hostname: &str, protocol: &str) -> Result<String> {
    Ok(normalize_base_url(hostname, protocol)?
        .trim_end_matches("/api/v3")
        .to_string())
}

fn ensure_leading_slash(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn normalize_relative_origin_path(path: &str, web_base_url: &str) -> String {
    if !path.starts_with('/') {
        return ensure_leading_slash(path);
    }

    let Ok(base) = Url::parse(web_base_url) else {
        return path.to_string();
    };
    let base_path = base.path().trim_end_matches('/');
    if !base_path.is_empty() {
        if let Some(stripped) = strip_prefix_with_boundary(path, base_path) {
            return ensure_leading_slash(ensure_request_path(stripped));
        }
    }

    path.to_string()
}

fn normalize_relative_api_path<'a>(path: &'a str, base_url: &str) -> &'a str {
    if !path.starts_with('/') {
        return path;
    }

    let Ok(base) = Url::parse(base_url) else {
        return path;
    };
    let api_path = base.path().trim_end_matches('/');
    if let Some(stripped) = strip_prefix_with_boundary(path, api_path) {
        return ensure_request_path(stripped);
    }
    if let Some(stripped) = strip_prefix_with_boundary(path, "/api/v3") {
        return ensure_request_path(stripped);
    }

    path
}

fn strip_prefix_with_boundary<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let path_end = path.find(['?', '#']).unwrap_or(path.len());
    let mut segments = path[..path_end].split('/');
    let mut consumed = 0;
    for (index, expected) in prefix.split('/').enumerate() {
        let actual = segments.next()?;
        // Split before decoding so an encoded separator remains segment data.
        if !percent_encoding::percent_decode_str(actual)
            .eq(percent_encoding::percent_decode_str(expected))
        {
            return None;
        }
        consumed += actual.len() + usize::from(index > 0);
    }
    Some(&path[consumed..])
}

fn ensure_request_path(path: &str) -> &str {
    if path.is_empty() {
        ""
    } else if path.starts_with('/') || path.starts_with('?') || path.starts_with('#') {
        path
    } else {
        ""
    }
}

fn path_has_base_prefix(path: &str, base: &str) -> bool {
    let normalized_base = base.trim_end_matches('/');
    if normalized_base.is_empty() {
        return true;
    }

    strip_prefix_with_boundary(path, normalized_base).is_some()
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_base_url, normalize_origin_path_or_url, normalize_path_or_url,
        normalize_relative_api_path, normalize_web_base_url,
    };
    use crate::api::client::ApiClient;

    #[test]
    fn encoded_deployment_paths_keep_their_request_boundary() {
        for deployment in [
            "/git%62ucket",
            "/%e6%97%a5%e6%9c%ac",
            "/bucket%25site",
            "/bucket%2520site",
        ] {
            let base = format!("https://example.com{deployment}/api/v3");
            assert_eq!(
                normalize_path_or_url("/user", &base).unwrap(),
                format!("{base}/user")
            );
            assert_eq!(
                normalize_origin_path_or_url("/a/r/pull/1.diff", &base).unwrap(),
                format!("https://example.com{deployment}/a/r/pull/1.diff")
            );
            for escape in [
                "/../../../outside",
                "/%252e%252e/%252e%252e/%252e%252e/outside",
            ] {
                assert!(normalize_path_or_url(escape, &base).is_err());
            }
        }
        let base = "https://example.com/git%62ucket/api/v3";
        assert_eq!(
            normalize_path_or_url("/gitbucket/api/v3/user?page=2", base).unwrap(),
            "https://example.com/git%62ucket/api/v3/user?page=2"
        );
        assert_eq!(
            normalize_origin_path_or_url("/gitbucket/a/r/pull/1.diff", base).unwrap(),
            "https://example.com/git%62ucket/a/r/pull/1.diff"
        );
        assert!(
            normalize_path_or_url("https://example.com/gitbucket/api/v3/user?page=2", base).is_ok()
        );
        assert!(
            normalize_path_or_url("https://example.com/gitbucket-other/api/v3/user", base).is_err()
        );
        for target in [
            "https://example.com/gitbucket%2fapi/v3/user",
            "https://example.com/gitbucket/api%2fv3/user",
            "https://example.com/gitbucket/api/v30/user",
        ] {
            assert!(normalize_path_or_url(target, base).is_err(), "{target}");
        }
        let ambiguous_base = "https://example.com/a/%252e%252e/bucket/api/v3";
        assert!(normalize_path_or_url("/user", ambiguous_base).is_err());
    }

    #[test]
    fn checks_the_final_normalized_request_path() {
        let base = "https://example.com/gitbucket/api/v3";
        for path in [
            "../../outside",
            "/../../outside",
            "/%2e%2e/%2e%2e/outside",
            "/%252e%252e%252f%252e%252e/outside",
            "/%3f/%252e%252e/%252e%252e/%252e%252e/outside",
            "/%23/%252e%252e/%252e%252e/%252e%252e/outside",
            "https://example.com/gitbucket/api/v3/../../../outside",
            "https://alice:secret@example.com/gitbucket/api/v3/user",
            "/user#fragment",
        ] {
            assert!(normalize_path_or_url(path, base).is_err(), "{path}");
        }
        assert_eq!(
            normalize_path_or_url("/repos/a/r/../other", base).unwrap(),
            "https://example.com/gitbucket/api/v3/repos/a/other"
        );
        assert_eq!(
            normalize_path_or_url("/labels/bug%2furgent", base).unwrap(),
            "https://example.com/gitbucket/api/v3/labels/bug%2furgent"
        );
        assert_eq!(
            normalize_path_or_url("/user?q=https://example.org", base).unwrap(),
            "https://example.com/gitbucket/api/v3/user?q=https://example.org"
        );
    }

    #[test]
    fn origin_fetches_cannot_escape_the_deployment() {
        for path in [
            "/../outside",
            "/%252e%252e/outside",
            "https://other.example/gitbucket/file",
        ] {
            assert!(
                normalize_origin_path_or_url(path, "https://example.com/gitbucket/api/v3").is_err()
            );
        }
    }

    #[test]
    fn rejects_ambiguous_base_urls_without_echoing_credentials() {
        for base in [
            "https://alice:secret@example.com",
            "https://example.com?key=secret",
            "https://example.com#fragment",
            "ssh://example.com",
        ] {
            let error = normalize_base_url(base, "https").unwrap_err();
            assert!(!error.to_string().contains("secret"));
        }
    }

    #[test]
    fn normalizes_plain_hostname() {
        let base = normalize_base_url("gitbucket.example.com", "https").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/api/v3");
    }

    #[test]
    fn normalizes_hostname_with_path() {
        let base = normalize_base_url("gitbucket.example.com/gitbucket", "https").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/gitbucket/api/v3");
    }

    #[test]
    fn normalizes_full_base_url() {
        let base = normalize_base_url("https://gitbucket.example.com/gitbucket", "http").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/gitbucket/api/v3");
    }

    #[test]
    fn keeps_existing_api_base_url() {
        let base =
            normalize_base_url("https://gitbucket.example.com/gitbucket/api/v3", "https").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/gitbucket/api/v3");
    }

    #[test]
    fn builds_web_url_from_subpath_base() {
        let client = ApiClient::new(
            "https://gitbucket.example.com/gitbucket",
            "dummy-token",
            "https",
        )
        .unwrap();
        assert_eq!(
            client.web_url("/alice/my-repo"),
            "https://gitbucket.example.com/gitbucket/alice/my-repo"
        );
    }

    #[test]
    fn preserves_port_and_trailing_slash_in_base_url() {
        let base =
            normalize_base_url("https://gitbucket.example.com:8443/gitbucket/", "http").unwrap();
        assert_eq!(base, "https://gitbucket.example.com:8443/gitbucket/api/v3");
    }

    #[test]
    fn normalizes_ipv6_host_with_port() {
        let base = normalize_base_url("http://[::1]:8080/gitbucket", "http").unwrap();
        assert_eq!(base, "http://[::1]:8080/gitbucket/api/v3");
    }

    #[test]
    fn normalizes_ipv6_host_without_port() {
        let base = normalize_base_url("http://[::1]/gitbucket", "http").unwrap();
        assert_eq!(base, "http://[::1]/gitbucket/api/v3");
    }

    #[test]
    fn normalizes_web_base_url() {
        let base = normalize_web_base_url("gitbucket.example.com/gitbucket", "https").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/gitbucket");
    }

    #[test]
    fn normalizes_api_root_relative_pagination_path() {
        assert_eq!(
            normalize_relative_api_path(
                "/api/v3/repos/alice/project/issues/7/comments?page=2",
                "https://gitbucket.example.com/api/v3"
            ),
            "/repos/alice/project/issues/7/comments?page=2"
        );
    }

    #[test]
    fn normalizes_subpath_api_root_relative_pagination_path() {
        assert_eq!(
            normalize_relative_api_path(
                "/gitbucket/api/v3/repos/alice/project/issues/7/comments?page=2",
                "https://gitbucket.example.com/gitbucket/api/v3"
            ),
            "/repos/alice/project/issues/7/comments?page=2"
        );
    }

    #[test]
    fn normalizes_origin_root_relative_path_with_subpath_base() {
        let normalized = normalize_origin_path_or_url(
            "/gitbucket/alice/project/pull/9.diff",
            "https://gitbucket.example.com/gitbucket/api/v3",
        )
        .unwrap();

        assert_eq!(
            normalized,
            "https://gitbucket.example.com/gitbucket/alice/project/pull/9.diff"
        );
    }

    #[test]
    fn preserves_origin_root_relative_path_without_subpath_base() {
        let normalized = normalize_origin_path_or_url(
            "/alice/project/pull/9.diff",
            "https://gitbucket.example.com/api/v3",
        )
        .unwrap();

        assert_eq!(
            normalized,
            "https://gitbucket.example.com/alice/project/pull/9.diff"
        );
    }

    #[test]
    fn leaves_plain_root_relative_paths_unchanged() {
        assert_eq!(
            normalize_relative_api_path(
                "/repos/alice/project/issues/7/comments?page=2",
                "https://gitbucket.example.com/api/v3"
            ),
            "/repos/alice/project/issues/7/comments?page=2"
        );
    }

    #[test]
    fn accepts_absolute_pagination_url_inside_api_base() {
        let normalized = normalize_path_or_url(
            "https://gitbucket.example.com/gitbucket/api/v3/repos/alice/project/issues/7/comments?page=2",
            "https://gitbucket.example.com/gitbucket/api/v3",
        )
        .unwrap();

        assert_eq!(
            normalized,
            "https://gitbucket.example.com/gitbucket/api/v3/repos/alice/project/issues/7/comments?page=2"
        );
    }

    #[test]
    fn rejects_absolute_pagination_url_on_different_host() {
        let err = normalize_path_or_url(
            "https://attacker.example/repos/alice/project/issues/7/comments?page=2",
            "https://gitbucket.example.com/api/v3",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("outside configured GitBucket API base"));
    }

    #[test]
    fn rejects_absolute_pagination_url_outside_subpath_api_base() {
        let err = normalize_path_or_url(
            "https://gitbucket.example.com/api/v3/repos/alice/project/issues/7/comments?page=2",
            "https://gitbucket.example.com/gitbucket/api/v3",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("outside configured GitBucket API base"));
    }

    #[test]
    fn rejects_absolute_pagination_url_with_api_prefix_boundary_mismatch() {
        let err = normalize_path_or_url(
            "https://gitbucket.example.com/api/v30/repos/alice/project/issues/7/comments?page=2",
            "https://gitbucket.example.com/api/v3",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("outside configured GitBucket API base"));
    }
}
