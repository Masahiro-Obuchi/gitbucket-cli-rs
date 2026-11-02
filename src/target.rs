//! Validated repository identity and remote-to-instance matching.

use percent_encoding::percent_decode_str;
use url::Url;

use crate::error::{GbError, Result};

pub(crate) fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RepositoryName {
    pub owner: String,
    pub repo: String,
}

pub(crate) fn validate_component(value: &str) -> Result<()> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || !value.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, '_' | '-' | '.' | '+')
                || (!ch.is_ascii() && !ch.is_control() && !ch.is_whitespace())
        })
    {
        return Err(GbError::Other(
            "Invalid repository format. Expected OWNER/REPO with safe, nonempty path components (no separators, dot segments, whitespace, or URL escapes).".into(),
        ));
    }
    Ok(())
}

impl RepositoryName {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        let (owner, repo) = value.split_once('/').ok_or_else(|| {
            GbError::Other("Invalid repository format. Expected OWNER/REPO.".into())
        })?;
        validate_component(owner)?;
        validate_component(repo)?;
        Ok(Self {
            owner: owner.into(),
            repo: repo.into(),
        })
    }
}

pub(crate) struct RepositoryTarget {
    pub instance: Url,
    pub name: RepositoryName,
}

impl RepositoryTarget {
    pub(crate) fn explicit(instance: Url, name: &str) -> Result<Self> {
        Ok(Self {
            instance,
            name: RepositoryName::parse(name)?,
        })
    }

    pub(crate) fn from_remote(instance: Url, remote: &str) -> Result<Self> {
        let mismatch = || {
            GbError::Other(
            "Git origin does not unambiguously match the selected GitBucket host and base path. Specify the intended --hostname and --repo OWNER/REPO explicitly.".into(),
        )
        };
        if remote.contains('\\') || remote.chars().any(char::is_control) {
            return Err(mismatch());
        }
        let url = if remote.contains("://") {
            Url::parse(remote).map_err(|_| mismatch())?
        } else {
            // SCP-like syntax, including bracketed IPv6 addresses.
            let (authority, path) = if let Some((left, right)) = remote.split_once("]:") {
                (format!("{left}]"), right)
            } else {
                let (left, right) = remote.split_once(':').ok_or_else(mismatch)?;
                (left.to_string(), right)
            };
            if authority.contains('/') || path.starts_with('/') {
                return Err(mismatch());
            }
            Url::parse(&format!("ssh://{authority}/{path}")).map_err(|_| mismatch())?
        };
        let http = matches!(url.scheme(), "http" | "https");
        let same_server = if http {
            same_origin(&instance, &url)
        } else {
            url.scheme() == "ssh" && instance.host_str() == url.host_str()
        };
        if !same_server
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path().to_ascii_lowercase().contains("%2f")
            || url.path().to_ascii_lowercase().contains("%5c")
        {
            return Err(mismatch());
        }
        let path = percent_decode_str(url.path())
            .decode_utf8()
            .map_err(|_| mismatch())?;
        let base = percent_decode_str(instance.path())
            .decode_utf8()
            .map_err(|_| mismatch())?;
        let relative = path
            .strip_prefix(base.trim_end_matches('/'))
            .and_then(|rest| rest.strip_prefix('/'))
            .ok_or_else(mismatch)?;
        // /git/ is an HTTP clone route, not a wildcard deployment prefix.
        let relative = if http {
            relative.strip_prefix("git/").unwrap_or(relative)
        } else {
            relative
        };
        let relative = relative.strip_suffix(".git").unwrap_or(relative);
        let name = RepositoryName::parse(relative).map_err(|_| mismatch())?;
        Ok(Self { instance, name })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_names_cannot_modify_url_structure() {
        for name in ["a/r", "a/名前", "a/r.git", "a/r+1", "a-b/c_d"] {
            assert!(RepositoryName::parse(name).is_ok(), "{name}");
        }
        for name in [
            "a",
            "/r",
            "a/",
            "a/b/c",
            "../r",
            "a/..",
            "a/%2e%2e",
            "a/r?x=1",
            "a/r#frag",
            "a/r\\x",
            "a/r;param",
            "a/ r",
            "a/r\n",
            "a/r%252f",
        ] {
            assert!(RepositoryName::parse(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn remote_must_match_origin_and_deployment() {
        let instance = Url::parse("https://example.com:8443/bucket").unwrap();
        for remote in [
            "https://example.com:8443/bucket/git/a/r.git",
            "https://example.com:8443/bucket/a/r.git",
            "ssh://git@example.com:29418/bucket/a/r.git",
            "git@example.com:bucket/a/r.git",
        ] {
            let target = RepositoryTarget::from_remote(instance.clone(), remote).unwrap();
            assert_eq!(target.name, RepositoryName::parse("a/r").unwrap());
        }
        for remote in [
            "https://other.example:8443/bucket/git/a/r.git",
            "https://example.com/bucket/git/a/r.git",
            "http://example.com:8443/bucket/git/a/r.git",
            "https://example.com:8443/other/git/a/r.git",
            "https://example.com:8443/bucket-other/a/r.git",
            "git@example.com:a/r.git",
            "file:///bucket/a/r.git",
            "https://example.com:8443/bucket/a/r.git?x",
            "https://example.com:8443/bucket/a/r.git/extra",
            "https://example.com:8443/bucket/a%2fr.git",
        ] {
            assert!(
                RepositoryTarget::from_remote(instance.clone(), remote).is_err(),
                "{remote}"
            );
        }
    }

    #[test]
    fn ssh_uri_supports_ports_and_ipv6_without_treating_them_as_http_ports() {
        let instance = Url::parse("https://[::1]:8443").unwrap();
        for remote in ["ssh://git@[::1]:29418/a/r.git", "git@[::1]:a/r.git"] {
            assert!(
                RepositoryTarget::from_remote(instance.clone(), remote).is_ok(),
                "{remote}"
            );
        }
    }
}
