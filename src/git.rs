//! Git authentication scoped to one configured instance and one requested URL.
mod process;
pub(crate) use process::GitCommandExt;

use std::io::{self, Read, Write};
use std::process::{Command, Output};
use url::Url;

use crate::api::client::normalize_web_base_url;
use crate::config::auth::HostConfig;
use crate::error::{GbError, Result};
use crate::target::same_origin;

const HELPER_ARG: &str = "--internal-git-credential";
const TARGET_ENV: &str = "GB_INTERNAL_GIT_TARGET";
const USER_ENV: &str = "GB_INTERNAL_GIT_USER";
const TOKEN_ENV: &str = "GB_INTERNAL_GIT_TOKEN";

fn git_command() -> Command {
    let mut command = Command::new("git");
    command
        .env_remove(TARGET_ENV)
        .env_remove(USER_ENV)
        .env_remove(TOKEN_ENV)
        .env_remove("GB_TOKEN")
        .env_remove("GB_PASSWORD")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_TRACE_REDACT", "1");
    command
}

fn rewritten_http_url(source: &str) -> Result<Option<Url>> {
    // --get-url performs only local URL rewriting, without contacting a remote.
    // Resolve before adding credentials to the child environment.
    let output = git_command()
        .args(["ls-remote", "--get-url", "--", source])
        .bounded_output()?;
    if !output.status.success() {
        return Err(GbError::Other("Unable to resolve Git URL rewriting".into()));
    }
    let Ok(mut url) = Url::parse(String::from_utf8_lossy(&output.stdout).trim()) else {
        return Ok(None);
    };
    if !matches!(url.scheme(), "http" | "https") {
        return Ok(None);
    }
    // Passwords do not participate in Git's HTTP configuration matching.
    // Never copy a password from an insteadOf rule into command arguments.
    url.set_password(None)
        .map_err(|_| GbError::Other("Invalid rewritten Git URL".into()))?;
    Ok(Some(url))
}

// Deliberately no Debug/Serialize: this value contains authentication material.
pub(crate) struct GitAuth {
    instance: Url,
    user: String,
    token: String,
}

impl GitAuth {
    pub(crate) fn new(hostname: &str, host: &HostConfig) -> Result<Self> {
        Ok(Self {
            instance: Url::parse(&normalize_web_base_url(hostname, &host.protocol)?)?,
            user: host.user.clone(),
            token: host.token.clone(),
        })
    }

    fn permits(&self, target: &Url) -> bool {
        let base = self.instance.path().trim_end_matches('/');
        let path = target.path();
        let relative = path.strip_prefix(base).unwrap_or("");
        same_origin(&self.instance, target)
            && relative.starts_with('/')
            && !path.to_ascii_lowercase().contains("%2f")
            && !path.to_ascii_lowercase().contains("%5c")
            && target.username().is_empty()
            && target.password().is_none()
            && target.query().is_none()
            && target.fragment().is_none()
            && !self.user.is_empty()
            && !self.token.is_empty()
            && !self.user.contains(['\r', '\n', '\0'])
            && !self.token.contains(['\r', '\n', '\0'])
    }

    /// The helper is ephemeral: no URL, argv, or Git config file contains a token.
    /// Git passes the rewritten URL to the helper, which validates it again.
    pub(crate) fn command(&self, source: &str) -> Result<Command> {
        let mut command = git_command();
        if let Ok(target) = Url::parse(source) {
            if matches!(target.scheme(), "http" | "https") && self.permits(&target) {
                let rewritten = rewritten_http_url(source)?;
                let executable = std::env::current_exe()?;
                let executable = executable.to_str().ok_or_else(|| {
                    GbError::Other("Git authentication requires a UTF-8 executable path".into())
                })?;
                // Git evaluates ! helpers with its shell, including on Windows.
                let executable = if cfg!(windows) {
                    executable.replace('\\', "/")
                } else {
                    executable.to_string()
                };
                let quoted = format!("'{}'", executable.replace('\'', "'\\''"));
                command
                    .args(["-c", "credential.helper="])
                    .args(["-c", &format!("credential.helper=!{quoted} {HELPER_ARG}")])
                    .args(["-c", "credential.useHttpPath=true"])
                    .env(
                        "GIT_ALLOW_PROTOCOL",
                        std::env::var("GIT_ALLOW_PROTOCOL")
                            .map(|value| {
                                value
                                    .split(':')
                                    .filter(|protocol| matches!(*protocol, "http" | "https"))
                                    .collect::<Vec<_>>()
                                    .join(":")
                            })
                            .unwrap_or_else(|_| "http:https".into()),
                    )
                    .args(["-c", "http.followRedirects=false"])
                    .args([
                        "-c",
                        &format!("http.{}.followRedirects=false", target.as_str()),
                    ])
                    .env(TARGET_ENV, target.as_str())
                    .env(USER_ENV, &self.user)
                    .env(TOKEN_ENV, &self.token);
                if let Some(rewritten) = rewritten {
                    // A rewrite can add a username, giving a URL-specific
                    // setting precedence over the original URL's setting.
                    command.args([
                        "-c",
                        &format!("http.{}.followRedirects=false", rewritten.as_str()),
                    ]);
                }
            }
        }
        Ok(command)
    }

    pub(crate) fn clone_url(&self, repo: &crate::models::repository::Repository) -> String {
        let fallback = format!(
            "{}/{}.git",
            self.instance.as_str().trim_end_matches('/'),
            repo.full_name
        );
        accessible_clone_url(repo.clone_url.as_deref(), &fallback)
    }

    pub(crate) fn redact_output(&self, output: &mut Output) {
        if !self.token.is_empty() {
            output.stdout = String::from_utf8_lossy(&output.stdout)
                .replace(&self.token, "[REDACTED]")
                .into_bytes();
            output.stderr = String::from_utf8_lossy(&output.stderr)
                .replace(&self.token, "[REDACTED]")
                .into_bytes();
        }
    }
}

pub(crate) fn accessible_clone_url(api_clone_url: Option<&str>, fallback_url: &str) -> String {
    let Some(api_clone_url) = api_clone_url else {
        return fallback_url.to_string();
    };

    let Ok(public_url) = Url::parse(fallback_url) else {
        return api_clone_url.to_string();
    };
    let Ok(api_url) = Url::parse(api_clone_url) else {
        return api_clone_url.to_string();
    };

    if !is_internal_gitbucket_clone_host(&api_url) && !same_origin(&api_url, &public_url) {
        return api_clone_url.to_string();
    }

    let public_prefix = public_repo_prefix(public_url.path());
    let mut clone_url = if is_internal_gitbucket_clone_host(&api_url) {
        public_url
    } else {
        api_url.clone()
    };
    let api_path = api_url.path();
    let normalized_api_path = if api_path.starts_with('/') {
        api_path.to_string()
    } else {
        format!("/{api_path}")
    };
    let combined_path = if public_prefix.is_empty()
        || normalized_api_path.starts_with(&format!("{public_prefix}/"))
    {
        normalized_api_path
    } else {
        format!("{public_prefix}{normalized_api_path}")
    };

    clone_url.set_path(&combined_path);
    clone_url.set_query(api_url.query());
    clone_url.set_fragment(api_url.fragment());
    clone_url.to_string()
}

fn public_repo_prefix(path: &str) -> String {
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() <= 2 {
        return String::new();
    }

    format!("/{}", segments[..segments.len() - 2].join("/"))
}

fn is_internal_gitbucket_clone_host(url: &Url) -> bool {
    matches!(url.host_str(), Some("gitbucket"))
}

/// Compare transport URLs without falling back to owner/repo alone.
/// A root /git/ prefix is GitBucket's HTTP clone route, not a deployment path.
pub(crate) fn repository_urls_match(left: &str, right: &str) -> bool {
    fn parse(value: &str) -> Option<Url> {
        Url::parse(value).ok().or_else(|| {
            let (authority, path) = value.split_once(':')?;
            if !authority.contains('@') {
                return None;
            }
            Url::parse(&format!("ssh://{authority}/{path}")).ok()
        })
    }
    let (Some(left), Some(right)) = (parse(left), parse(right)) else {
        return false;
    };
    let allowed = |url: &Url| matches!(url.scheme(), "http" | "https" | "ssh");
    if !allowed(&left)
        || !allowed(&right)
        || left.query().is_some()
        || right.query().is_some()
        || left.fragment().is_some()
        || right.fragment().is_some()
    {
        return false;
    }
    let transport_matches = if left.scheme() == right.scheme() {
        same_origin(&left, &right)
    } else {
        // Only the standard SSH transport can be associated without explicit
        // server metadata. Never equate HTTP and HTTPS or custom SSH ports.
        (left.scheme() == "ssh" || right.scheme() == "ssh")
            && left.host_str() == right.host_str()
            && left.port().is_none()
            && right.port().is_none()
    };
    let path = |url: &Url| {
        let path = url
            .path()
            .strip_prefix("/git/")
            .unwrap_or(url.path().trim_start_matches('/'));
        path.strip_suffix(".git").unwrap_or(path).to_string()
    };
    transport_matches && path(&left) == path(&right)
}

/// Remove URL userinfo before spawning Git or displaying a source.
pub(crate) fn clean_source(source: &str) -> Result<String> {
    if let Ok(mut url) = Url::parse(source) {
        if matches!(url.scheme(), "http" | "https") {
            url.set_username("")
                .map_err(|_| GbError::Other("Invalid Git URL".into()))?;
            url.set_password(None)
                .map_err(|_| GbError::Other("Invalid Git URL".into()))?;
            return Ok(url.to_string());
        }
    }
    Ok(source.to_string())
}

fn credential_reply(target: &str, user: &str, token: &str, input: &str) -> Option<String> {
    let expected = Url::parse(target).ok()?;
    let mut protocol = None;
    let mut host = None;
    let mut path = None;
    for line in input.lines().take_while(|line| !line.is_empty()) {
        let (key, value) = line.split_once('=')?;
        let slot = match key {
            "protocol" => &mut protocol,
            "host" => &mut host,
            "path" => &mut path,
            _ => continue,
        };
        // Ambiguous credential contexts must never receive a secret.
        if slot.replace(value).is_some() {
            return None;
        }
    }
    if !matches!(expected.scheme(), "http" | "https")
        || user.is_empty()
        || token.is_empty()
        || user.contains(['\r', '\n', '\0'])
        || token.contains(['\r', '\n', '\0'])
    {
        return None;
    }
    let protocol = protocol?;
    let host = host?;
    if !matches!(protocol, "http" | "https") || host.contains(['/', '\\', '?', '#', '@']) {
        return None;
    }
    let actual = Url::parse(&format!("{protocol}://{host}/")).ok()?;
    // Git decodes the path and removes leading/trailing slashes before
    // calling credential helpers.
    let expected_path = percent_encoding::percent_decode_str(expected.path())
        .decode_utf8()
        .ok()?;
    if !same_origin(&expected, &actual)
        || !actual.username().is_empty()
        || actual.password().is_some()
        || actual.path() != "/"
        || actual.query().is_some()
        || actual.fragment().is_some()
        || path? != expected_path.trim_matches('/')
    {
        return None;
    }
    Some(format!("username={user}\npassword={token}\n\n"))
}

/// Runs before clap, only for Git's private credential-helper subprocess.
/// Store/erase deliberately do nothing; credentials are never persisted.
pub(crate) fn run_credential_helper() -> bool {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(HELPER_ARG)) {
        return false;
    }
    if args.next().as_deref() != Some(std::ffi::OsStr::new("get")) || args.next().is_some() {
        return true;
    }
    let reply = (|| {
        let target = std::env::var(TARGET_ENV).ok()?;
        let user = std::env::var(USER_ENV).ok()?;
        let token = std::env::var(TOKEN_ENV).ok()?;
        let mut input = String::new();
        io::stdin().take(65537).read_to_string(&mut input).ok()?;
        if input.len() > 65536 {
            return None;
        }
        credential_reply(&target, &user, &token, &input)
    })();
    let _ = io::stdout().write_all(reply.as_deref().unwrap_or("quit=true\n\n").as_bytes());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_repo_prefix_extracts_optional_base_path() {
        assert_eq!(public_repo_prefix("/alice/demo.git"), "");
        assert_eq!(
            public_repo_prefix("/gitbucket/alice/demo.git"),
            "/gitbucket"
        );
    }

    #[test]
    fn credentials_are_scoped_to_origin_and_deployment_path() {
        let auth = GitAuth::new(
            "https://example.com/gitbucket",
            &HostConfig {
                user: "alice".into(),
                token: "secret".into(),
                protocol: "https".into(),
            },
        )
        .unwrap();
        assert!(auth.permits(&Url::parse("https://example.com/gitbucket/git/a/r.git").unwrap()));
        for target in [
            "https://other.example/gitbucket/git/a/r.git",
            "http://example.com/gitbucket/git/a/r.git",
            "https://example.com:8443/gitbucket/git/a/r.git",
            "https://example.com/gitbucket-other/a/r.git",
            "https://example.com/other/a/r.git",
            "https://example.com/gitbucket/../other/a/r.git",
            "https://example.com/gitbucket/a%2fb.git",
        ] {
            assert!(!auth.permits(&Url::parse(target).unwrap()), "{target}");
        }
    }

    #[test]
    fn command_never_contains_token_or_persists_helper_configuration() {
        let auth = GitAuth::new(
            "https://example.com/gitbucket",
            &HostConfig {
                user: "alice".into(),
                token: "secret-value".into(),
                protocol: "https".into(),
            },
        )
        .unwrap();
        let command = auth
            .command("https://example.com/gitbucket/git/a/r.git")
            .unwrap();
        let args: Vec<_> = command.get_args().map(|s| s.to_string_lossy()).collect();
        assert!(args.iter().all(|arg| !arg.contains("secret-value")));
        assert!(args.iter().any(|arg| arg == "http.followRedirects=false"));
        assert!(command
            .get_envs()
            .any(|(key, value)| key == TOKEN_ENV
                && value == Some(std::ffi::OsStr::new("secret-value"))));
        let foreign = auth
            .command("https://other.example/gitbucket/git/a/r.git")
            .unwrap();
        assert!(!foreign
            .get_envs()
            .any(|(key, value)| key == TOKEN_ENV && value.is_some()));
    }

    #[test]
    fn remote_identity_includes_host_and_deployment() {
        assert!(repository_urls_match(
            "git@example.com:a/r.git",
            "https://example.com/git/a/r.git"
        ));
        for other in [
            "https://other.example/a/r.git",
            "http://example.com/a/r.git",
            "https://example.com:8443/a/r.git",
            "https://example.com/gitbucket/a/r.git",
        ] {
            assert!(!repository_urls_match("https://example.com/a/r.git", other));
        }
    }

    #[test]
    fn helper_checks_rewritten_origin_and_exact_path() {
        let target = "https://example.com:443/gitbucket/git/a/r.git";
        let valid = "protocol=https\nhost=example.com\npath=gitbucket/git/a/r.git\n\n";
        assert_eq!(
            credential_reply(target, "alice", "secret", valid).as_deref(),
            Some("username=alice\npassword=secret\n\n")
        );
        for input in [
            valid.replace("example.com", "other.example"),
            valid.replace("https", "http"),
            valid.replace("example.com", "example.com:8443"),
            valid.replace("gitbucket/git", "other/git"),
            valid.replace("r.git", "r.git/extra"),
            valid.replace("path=", "ignored="),
            format!("protocol=http\n{valid}"),
        ] {
            assert!(credential_reply(target, "alice", "secret", &input).is_none());
        }
        assert!(credential_reply(target, "alice", "secret\nmalicious=value", valid).is_none());
    }

    #[test]
    fn helper_matches_git_normalized_paths_without_double_decoding() {
        let target = "https://example.com//bucket%20site/git/a/r%2520.git///";
        let valid = "protocol=https\nhost=example.com\npath=bucket site/git/a/r%20.git\n\n";
        assert_eq!(
            credential_reply(target, "alice", "secret", valid).as_deref(),
            Some("username=alice\npassword=secret\n\n")
        );
        for input in [
            valid.replace("r%20.git", "r .git"),
            valid.replace("bucket site", "bucket%20site"),
            valid.replace("r%20.git", "r%20.git/extra"),
        ] {
            assert!(credential_reply(target, "alice", "secret", &input).is_none());
        }
    }
}
