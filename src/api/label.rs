use crate::api::client::ApiClient;
use crate::error::{GbError, Result};
use crate::models::label::{AddIssueLabels, CreateLabel, Label, UpdateLabel};
use url::Url;

impl ApiClient {
    /// List labels for a repository.
    pub async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<Label>> {
        self.get(&format!("/repos/{owner}/{repo}/labels")).await
    }

    /// Get a single label.
    pub async fn get_label(&self, owner: &str, repo: &str, name: &str) -> Result<Label> {
        self.get(&label_path(owner, repo, name)?).await
    }

    /// Create a label.
    pub async fn create_label(&self, owner: &str, repo: &str, body: &CreateLabel) -> Result<Label> {
        self.post(&format!("/repos/{owner}/{repo}/labels"), body)
            .await
    }

    /// Update a label.
    pub async fn update_label(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
        body: &UpdateLabel,
    ) -> Result<Label> {
        self.patch(&label_path(owner, repo, name)?, body).await
    }

    /// Delete a label.
    pub async fn delete_label(&self, owner: &str, repo: &str, name: &str) -> Result<()> {
        self.delete(&label_path(owner, repo, name)?).await
    }
    pub async fn list_issue_labels(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<Label>> {
        self.get(&format!("/repos/{owner}/{repo}/issues/{number}/labels"))
            .await
    }

    pub async fn add_issue_labels(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        labels: Vec<String>,
    ) -> Result<()> {
        let _: serde_json::Value = self
            .post(
                &format!("/repos/{owner}/{repo}/issues/{number}/labels"),
                &AddIssueLabels { labels },
            )
            .await?;
        Ok(())
    }

    pub async fn remove_issue_label(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        name: &str,
    ) -> Result<()> {
        validate_label_segment(name)?;
        let mut url = Url::parse("https://example.invalid").expect("static URL");
        url.path_segments_mut().expect("static URL").extend([
            "repos",
            owner,
            repo,
            "issues",
            &number.to_string(),
            "labels",
            name,
        ]);
        self.delete(url.path()).await
    }
}

pub(crate) fn validate_label_segment(name: &str) -> Result<()> {
    let refused = || {
        GbError::Other("Label API names must be nonempty and cannot contain dot segments, backslashes or control characters at any decoding level.".into())
    };
    if name.is_empty() {
        return Err(refused());
    }
    // Slashes are encoded as label-name data. Reject traversal even after
    // repeated decoding, since proxies may interpret encoded separators first.
    let mut interpreted = name.to_string();
    for _ in 0..8 {
        if interpreted.contains('\\')
            || interpreted.chars().any(char::is_control)
            || interpreted
                .split('/')
                .any(|part| matches!(part, "." | ".."))
        {
            return Err(refused());
        }
        let decoded = percent_encoding::percent_decode_str(&interpreted)
            .decode_utf8()
            .map_err(|_| refused())?
            .into_owned();
        if decoded == interpreted {
            return Ok(());
        }
        interpreted = decoded;
    }
    Err(refused())
}

fn label_path(owner: &str, repo: &str, name: &str) -> Result<String> {
    validate_label_segment(name)?;
    let mut url = Url::parse("https://example.invalid").expect("static URL must parse");
    url.path_segments_mut()
        .expect("static URL must support path segments")
        .extend(["repos", owner, repo, "labels", name]);
    Ok(url.path().to_string())
}

#[cfg(test)]
mod tests {
    use super::label_path;

    #[test]
    fn label_names_keep_encoded_separators_without_allowing_traversal() {
        assert_eq!(
            label_path("alice", "project", "scope/bug").unwrap(),
            "/repos/alice/project/labels/scope%2Fbug"
        );
        for name in [".", "..", "%2e%2e", "safe/../../..", "safe%252f%252e%252e"] {
            assert!(label_path("alice", "project", name).is_err(), "{name}");
        }
    }

    #[test]
    fn label_path_percent_encodes_spaces() {
        assert_eq!(
            label_path("alice", "project", "needs review").unwrap(),
            "/repos/alice/project/labels/needs%20review"
        );
    }
}
