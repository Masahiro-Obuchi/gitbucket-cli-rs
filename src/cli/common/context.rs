use crate::api::client::ApiClient;
use crate::config::auth::AuthConfig;
use crate::error::Result;
use crate::git::GitAuth;

use super::auth::hostname_from_config;
use super::repo_resolution::resolve_target;

pub struct HostContext {
    pub hostname: String,
    pub client: ApiClient,
}

impl HostContext {
    pub fn resolve(cli_hostname: &Option<String>, cli_profile: &Option<String>) -> Result<Self> {
        let config = AuthConfig::load()?;
        let hostname = hostname_from_config(&config, cli_hostname, cli_profile)?;
        let credential = config.credential_for_profile(&hostname, cli_profile.as_deref())?;
        let client = ApiClient::new(
            &credential.instance,
            &credential.host.token,
            credential.protocol(),
        )?;
        Ok(Self {
            hostname: credential.instance,
            client,
        })
    }
}

pub struct RepoContext {
    pub hostname: String,
    pub git_auth: GitAuth,
    pub owner: String,
    pub repo: String,
    pub client: ApiClient,
}

impl RepoContext {
    pub fn resolve(
        cli_hostname: &Option<String>,
        cli_repo: &Option<String>,
        cli_profile: &Option<String>,
    ) -> Result<Self> {
        let config = AuthConfig::load()?;
        let hostname = hostname_from_config(&config, cli_hostname, cli_profile)?;
        let credential = config.credential_for_profile(&hostname, cli_profile.as_deref())?;
        let target = resolve_target(&config, &credential.instance, cli_repo, cli_profile)?;
        let (owner, repo) = (target.name.owner, target.name.repo);
        let client = ApiClient::new(
            &credential.instance,
            &credential.host.token,
            credential.protocol(),
        )?;
        let git_auth = GitAuth::new(target.instance, &credential.host);
        Ok(Self {
            hostname: credential.instance,
            git_auth,
            owner,
            repo,
            client,
        })
    }
}
