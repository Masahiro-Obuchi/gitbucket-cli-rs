use crate::api::client::ApiClient;
use crate::error::{GbError, Result};
use crate::models::repository::{CreateRepository, Repository};

impl ApiClient {
    /// List repositories for a user
    pub async fn list_user_repos(&self, owner: &str) -> Result<Vec<Repository>> {
        crate::target::validate_component(owner)?;
        self.get(&format!("/users/{}/repos", owner)).await
    }

    /// List repositories for an organization
    pub async fn list_org_repos(&self, org: &str) -> Result<Vec<Repository>> {
        crate::target::validate_component(org)?;
        self.get(&format!("/orgs/{}/repos", org)).await
    }

    /// List repositories for an owner that may be a user or organization
    pub async fn list_owner_repos(&self, owner: &str) -> Result<Vec<Repository>> {
        match self.list_org_repos(owner).await {
            Ok(repos) => Ok(repos),
            Err(GbError::Api { status: 404, .. }) => self.list_user_repos(owner).await,
            Err(err) => Err(err),
        }
    }

    /// List repositories for the authenticated user
    pub async fn list_my_repos(&self) -> Result<Vec<Repository>> {
        self.get("/user/repos").await
    }

    /// Get a single repository
    pub async fn get_repo(&self, owner: &str, repo: &str) -> Result<Repository> {
        crate::target::RepositoryName::parse(&format!("{owner}/{repo}"))?;
        self.get(&format!("/repos/{}/{}", owner, repo)).await
    }

    /// Create a repository for the authenticated user
    pub async fn create_user_repo(&self, body: &CreateRepository) -> Result<Repository> {
        self.post("/user/repos", body).await
    }

    /// Create a repository under an organization
    pub async fn create_org_repo(&self, org: &str, body: &CreateRepository) -> Result<Repository> {
        crate::target::validate_component(org)?;
        self.post(&format!("/orgs/{}/repos", org), body).await
    }

    /// Delete a repository
    pub async fn delete_repo(&self, owner: &str, repo: &str) -> Result<()> {
        self.delete(&format!("/repos/{}/{}", owner, repo)).await
    }

    /// Fork a repository
    pub async fn fork_repo(
        &self,
        owner: &str,
        repo: &str,
        group: Option<&str>,
    ) -> Result<Repository> {
        let body = group
            .map(|group| serde_json::json!({"organization":group}))
            .unwrap_or_else(|| serde_json::json!({}));
        self.post::<Repository, serde_json::Value>(
            &format!("/repos/{}/{}/forks", owner, repo),
            &body,
        )
        .await
    }
}
