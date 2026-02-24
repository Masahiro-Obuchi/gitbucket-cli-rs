use crate::api::client::ApiClient;
use crate::error::Result;
use crate::models::repository::{CreateRepository, Repository};

impl ApiClient {
    /// List repositories for a user
    pub async fn list_user_repos(&self, owner: &str) -> Result<Vec<Repository>> {
        self.get(&format!("/users/{}/repos", owner)).await
    }

    /// List repositories for the authenticated user
    pub async fn list_my_repos(&self) -> Result<Vec<Repository>> {
        self.get("/user/repos").await
    }

    /// Get a single repository
    pub async fn get_repo(&self, owner: &str, repo: &str) -> Result<Repository> {
        self.get(&format!("/repos/{}/{}", owner, repo)).await
    }

    /// Create a repository for the authenticated user
    pub async fn create_user_repo(&self, body: &CreateRepository) -> Result<Repository> {
        self.post("/user/repos", body).await
    }

    /// Create a repository under an organization
    pub async fn create_org_repo(
        &self,
        org: &str,
        body: &CreateRepository,
    ) -> Result<Repository> {
        self.post(&format!("/orgs/{}/repos", org), body).await
    }

    /// Delete a repository
    pub async fn delete_repo(&self, owner: &str, repo: &str) -> Result<()> {
        self.delete(&format!("/repos/{}/{}", owner, repo)).await
    }

    /// Fork a repository
    pub async fn fork_repo(&self, owner: &str, repo: &str) -> Result<Repository> {
        self.post::<Repository, serde_json::Value>(
            &format!("/repos/{}/{}/forks", owner, repo),
            &serde_json::json!({}),
        )
        .await
    }
}
