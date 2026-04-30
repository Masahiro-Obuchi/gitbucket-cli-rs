use url::Url;

use crate::cli::common::{
    create_client, parse_owner_repo, resolve_hostname, validate_selected_profile,
};
use crate::error::Result;
use crate::output;

pub(super) async fn clone(
    hostname: &Option<String>,
    cli_profile: &Option<String>,
    repo: &str,
    directory: Option<&str>,
) -> Result<()> {
    let clone_url = if repo.contains("://") || repo.contains('@') {
        validate_selected_profile(cli_profile)?;
        repo.to_string()
    } else {
        let hostname = resolve_hostname(hostname, cli_profile)?;
        let client = create_client(&hostname, cli_profile)?;
        let (owner, name) = parse_owner_repo(repo)?;
        let r = client.get_repo(&owner, &name).await?;
        let fallback_clone_url = client.web_url(&format!("/{}/{}.git", owner, name));
        accessible_clone_url(r.clone_url.as_deref(), &fallback_clone_url)
    };

    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone").arg(&clone_url);
    if let Some(dir) = directory {
        cmd.arg(dir);
    }

    let output = cmd.output()?;
    if output.status.success() {
        if !output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() && !output::suppress_stderr() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }
    } else {
        return Err(crate::error::GbError::Other(format!(
            "git clone failed. {}",
            command_output_summary(&output)
        )));
    }

    Ok(())
}

pub(super) fn accessible_clone_url(api_clone_url: Option<&str>, fallback_url: &str) -> String {
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

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn command_output_summary(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        return stderr.lines().take(3).collect::<Vec<_>>().join(" ");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();
    if !stdout.is_empty() {
        return stdout.lines().take(3).collect::<Vec<_>>().join(" ");
    }

    "command did not provide output details.".into()
}

#[cfg(test)]
mod tests {
    use super::{accessible_clone_url, public_repo_prefix};

    #[test]
    fn public_repo_prefix_extracts_optional_base_path() {
        assert_eq!(public_repo_prefix("/alice/demo.git"), "");
        assert_eq!(
            public_repo_prefix("/gitbucket/alice/demo.git"),
            "/gitbucket"
        );
    }

    #[test]
    fn accessible_clone_url_rewrites_internal_host_to_public_base() {
        let rewritten = accessible_clone_url(
            Some("http://gitbucket:8080/git/alice/demo.git"),
            "http://127.0.0.1:18080/gitbucket/alice/demo.git",
        );

        assert_eq!(
            rewritten,
            "http://127.0.0.1:18080/gitbucket/git/alice/demo.git"
        );
    }

    #[test]
    fn accessible_clone_url_keeps_matching_public_clone_url() {
        let clone_url = "http://127.0.0.1:18080/gitbucket/git/alice/demo.git";
        let fallback_url = "http://127.0.0.1:18080/gitbucket/alice/demo.git";

        assert_eq!(
            accessible_clone_url(Some(clone_url), fallback_url),
            clone_url
        );
    }

    #[test]
    fn accessible_clone_url_preserves_external_clone_origin() {
        let clone_url = "https://clone.gitbucket.example.com/git/alice/demo.git";
        let fallback_url = "https://gitbucket.example.com/gitbucket/alice/demo.git";

        assert_eq!(
            accessible_clone_url(Some(clone_url), fallback_url),
            clone_url
        );
    }

    #[test]
    fn accessible_clone_url_falls_back_when_api_clone_url_is_missing() {
        let fallback_url = "http://127.0.0.1:18080/gitbucket/alice/demo.git";

        assert_eq!(accessible_clone_url(None, fallback_url), fallback_url);
    }
}
