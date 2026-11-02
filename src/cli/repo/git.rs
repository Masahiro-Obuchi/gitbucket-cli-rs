use crate::cli::common::{
    parse_owner_repo, resolve_host_config, resolve_hostname, validate_selected_profile,
};
use crate::error::Result;
use crate::output;

pub(super) async fn clone(
    hostname: &Option<String>,
    cli_profile: &Option<String>,
    repo: &str,
    directory: Option<&str>,
) -> Result<()> {
    let mut git_auth = None;
    let clone_url = if repo.contains("://") || repo.contains('@') {
        validate_selected_profile(cli_profile)?;
        if url::Url::parse(repo)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.password().is_some())
        {
            return Err(crate::error::GbError::Other(
                "Passwords in HTTP clone URLs are not supported. Use a Git credential helper, or clone OWNER/REPO after `gb auth login`.".into(),
            ));
        }
        repo.to_string()
    } else {
        let hostname = resolve_hostname(hostname, cli_profile)?;
        let host = resolve_host_config(&hostname, cli_profile)?;
        git_auth = Some(crate::git::GitAuth::new(&hostname, &host)?);
        let client = crate::api::client::ApiClient::new(&hostname, &host.token, &host.protocol)?;
        let (owner, name) = parse_owner_repo(repo)?;
        let r = client.get_repo(&owner, &name).await?;
        let fallback_clone_url = client.web_url(&format!("/{}/{}.git", owner, name));
        accessible_clone_url(r.clone_url.as_deref(), &fallback_clone_url)
    };

    let clone_url = if git_auth.is_some() {
        crate::git::clean_source(&clone_url)?
    } else {
        clone_url
    };
    let mut cmd = match &git_auth {
        Some(auth) => auth.command(&clone_url)?,
        None => {
            let mut command = std::process::Command::new("git");
            command.env_remove("GB_TOKEN").env_remove("GB_PASSWORD");
            command
        }
    };
    cmd.arg("clone").arg("--").arg(&clone_url);
    if let Some(dir) = directory {
        cmd.arg(dir);
    }

    let mut output = cmd.output()?;
    if let Some(auth) = &git_auth {
        auth.redact_output(&mut output);
    }
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

pub(super) use crate::git::accessible_clone_url;

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
    use super::accessible_clone_url;

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
