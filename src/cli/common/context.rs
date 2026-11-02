use crate::api::client::ApiClient;
use crate::error::Result;
use crate::git::GitAuth;

use super::{create_client, resolve_host_config, resolve_hostname, resolve_repo};

pub struct HostContext {
    pub hostname: String,
    pub client: ApiClient,
}

impl HostContext {
    pub fn resolve(cli_hostname: &Option<String>, cli_profile: &Option<String>) -> Result<Self> {
        let hostname = resolve_hostname(cli_hostname, cli_profile)?;
        let client = create_client(&hostname, cli_profile)?;
        Ok(Self { hostname, client })
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
        let hostname = resolve_hostname(cli_hostname, cli_profile)?;
        let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
        let host = resolve_host_config(&hostname, cli_profile)?;
        let client = ApiClient::new(&hostname, &host.token, &host.protocol)?;
        let git_auth = GitAuth::new(&hostname, &host)?;
        Ok(Self {
            hostname,
            git_auth,
            owner,
            repo,
            client,
        })
    }
}
