use crate::api::client::ApiClient;
use crate::error::Result;
use crate::models::milestone::{CreateMilestone, Milestone, UpdateMilestone};

impl ApiClient {
    /// List milestones for a repository.
    pub async fn list_milestones(
        &self,
        owner: &str,
        repo: &str,
        state: &str,
    ) -> Result<Vec<Milestone>> {
        self.get(&format!("/repos/{owner}/{repo}/milestones?state={state}"))
            .await
    }

    /// Get a single milestone.
    pub async fn get_milestone(&self, owner: &str, repo: &str, number: u64) -> Result<Milestone> {
        self.get(&format!("/repos/{owner}/{repo}/milestones/{number}"))
            .await
    }

    /// Create a milestone.
    pub async fn create_milestone(
        &self,
        owner: &str,
        repo: &str,
        body: &CreateMilestone,
    ) -> Result<Milestone> {
        self.post(&format!("/repos/{owner}/{repo}/milestones"), body)
            .await
    }

    /// Update a milestone.
    pub async fn update_milestone(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdateMilestone,
    ) -> Result<Milestone> {
        self.patch(&format!("/repos/{owner}/{repo}/milestones/{number}"), body)
            .await
    }

    /// Delete a milestone.
    pub async fn delete_milestone(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        self.delete(&format!("/repos/{owner}/{repo}/milestones/{number}"))
            .await
    }
}
