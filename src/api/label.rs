use crate::api::client::ApiClient;
use crate::error::Result;
use crate::models::label::{CreateLabel, Label};
use url::Url;

impl ApiClient {
    /// List labels for a repository.
    pub async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<Label>> {
        self.get(&format!("/repos/{owner}/{repo}/labels")).await
    }

    /// Get a single label.
    pub async fn get_label(&self, owner: &str, repo: &str, name: &str) -> Result<Label> {
        self.get(&label_path(owner, repo, name)).await
    }

    /// Create a label.
    pub async fn create_label(&self, owner: &str, repo: &str, body: &CreateLabel) -> Result<Label> {
        self.post(&format!("/repos/{owner}/{repo}/labels"), body)
            .await
    }

    /// Delete a label.
    pub async fn delete_label(&self, owner: &str, repo: &str, name: &str) -> Result<()> {
        self.delete(&label_path(owner, repo, name)).await
    }
}

fn label_path(owner: &str, repo: &str, name: &str) -> String {
    let mut url = Url::parse("https://example.invalid").expect("static URL must parse");
    url.path_segments_mut()
        .expect("static URL must support path segments")
        .extend(["repos", owner, repo, "labels", name]);
    url.path().to_string()
}

#[cfg(test)]
mod tests {
    use super::label_path;

    #[test]
    fn label_path_percent_encodes_spaces() {
        assert_eq!(
            label_path("alice", "project", "needs review"),
            "/repos/alice/project/labels/needs%20review"
        );
    }
}
