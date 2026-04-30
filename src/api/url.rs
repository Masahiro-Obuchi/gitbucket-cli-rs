use url::Url;

use crate::error::{GbError, Result};

pub(crate) fn normalize_path_or_url(path_or_url: &str, base_url: &str) -> Result<String> {
    if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
        validate_absolute_api_url(path_or_url, base_url)?;
        Ok(path_or_url.to_string())
    } else {
        Ok(format!(
            "{}{}",
            base_url,
            normalize_relative_api_path(path_or_url, base_url)
        ))
    }
}

pub(crate) fn normalize_origin_path_or_url(path_or_url: &str, base_url: &str) -> Result<String> {
    let web_base_url = base_url.trim_end_matches("/api/v3");
    if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
        validate_absolute_origin_url(path_or_url, web_base_url)?;
        Ok(path_or_url.to_string())
    } else {
        Ok(format!(
            "{}{}",
            web_base_url,
            normalize_relative_origin_path(path_or_url, web_base_url)
        ))
    }
}

pub(crate) fn normalize_base_url(hostname: &str, protocol: &str) -> Result<String> {
    let input = hostname.trim().trim_end_matches('/');
    let candidate = if input.starts_with("http://") || input.starts_with("https://") {
        input.to_string()
    } else {
        format!("{}://{}", protocol, input)
    };

    let parsed = Url::parse(&candidate)?;
    let host = parsed
        .host_str()
        .ok_or_else(|| GbError::Config(format!("Invalid GitBucket host or URL: {}", hostname)))?;

    let mut base = format!("{}://{}", parsed.scheme(), host);
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

fn validate_absolute_api_url(url: &str, base_url: &str) -> Result<()> {
    let target = Url::parse(url)?;
    let base = Url::parse(base_url)?;

    let same_origin = target.scheme() == base.scheme()
        && target.host_str() == base.host_str()
        && target.port_or_known_default() == base.port_or_known_default();
    let same_api_base = path_has_base_prefix(target.path(), base.path());

    if same_origin && same_api_base {
        Ok(())
    } else {
        Err(GbError::Other(format!(
            "Refusing to follow pagination URL outside configured GitBucket API base {}",
            base_url
        )))
    }
}

fn validate_absolute_origin_url(url: &str, web_base_url: &str) -> Result<()> {
    let target = Url::parse(url)?;
    let base = Url::parse(web_base_url)?;

    let same_origin = target.scheme() == base.scheme()
        && target.host_str() == base.host_str()
        && target.port_or_known_default() == base.port_or_known_default();
    let same_base = path_has_base_prefix(target.path(), base.path());

    if same_origin && same_base {
        Ok(())
    } else {
        Err(GbError::Other(format!(
            "Refusing to fetch URL outside configured GitBucket origin {}",
            web_base_url
        )))
    }
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
    path.strip_prefix(prefix).filter(|rest| {
        rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#')
    })
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

    path == normalized_base
        || path
            .strip_prefix(normalized_base)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_base_url, normalize_origin_path_or_url, normalize_path_or_url,
        normalize_relative_api_path, normalize_web_base_url,
    };
    use crate::api::client::ApiClient;

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
