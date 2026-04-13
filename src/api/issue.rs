use crate::api::client::ApiClient;
use crate::error::Result;
use crate::models::comment::{Comment, CreateComment};
use crate::models::issue::{CreateIssue, Issue, UpdateIssue};
use reqwest::header::{HeaderMap, LINK};

impl ApiClient {
    /// List issues for a repository
    pub async fn list_issues(&self, owner: &str, repo: &str, state: &str) -> Result<Vec<Issue>> {
        self.get(&format!("/repos/{owner}/{repo}/issues?state={state}"))
            .await
    }

    /// Get a single issue
    pub async fn get_issue(&self, owner: &str, repo: &str, number: u64) -> Result<Issue> {
        self.get(&format!("/repos/{}/{}/issues/{}", owner, repo, number))
            .await
    }

    /// Create an issue
    pub async fn create_issue(&self, owner: &str, repo: &str, body: &CreateIssue) -> Result<Issue> {
        self.post(&format!("/repos/{}/{}/issues", owner, repo), body)
            .await
    }

    /// Update an issue (close, reopen, edit)
    pub async fn update_issue(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdateIssue,
    ) -> Result<Issue> {
        self.patch(
            &format!("/repos/{}/{}/issues/{}", owner, repo, number),
            body,
        )
        .await
    }

    /// List comments on an issue
    pub async fn list_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<Comment>> {
        self.get(&format!(
            "/repos/{}/{}/issues/{}/comments",
            owner, repo, number
        ))
        .await
    }

    /// Add a comment to an issue
    pub async fn create_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &CreateComment,
    ) -> Result<Comment> {
        self.post(
            &format!("/repos/{}/{}/issues/{}/comments", owner, repo, number),
            body,
        )
        .await
    }

    /// List all comments on an issue across paginated API responses
    pub async fn list_all_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<Comment>> {
        let mut path = Some(format!(
            "/repos/{}/{}/issues/{}/comments?per_page=100",
            owner, repo, number
        ));
        let mut comments = Vec::new();

        while let Some(next_path) = path {
            let (mut page, headers) = self.get_with_headers::<Vec<Comment>>(&next_path).await?;
            comments.append(&mut page);
            path = next_link(&headers);
        }

        Ok(comments)
    }

    /// Update an issue comment
    pub async fn update_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &CreateComment,
    ) -> Result<Comment> {
        self.patch(
            &format!("/repos/{}/{}/issues/comments/{}", owner, repo, comment_id),
            body,
        )
        .await
    }
}

fn next_link(headers: &HeaderMap) -> Option<String> {
    let link = headers.get(LINK)?.to_str().ok()?;
    link.split(',').find_map(|entry| {
        let (url_part, params) = entry.trim().split_once(';')?;
        if !params
            .split(';')
            .any(|param| param.trim() == r#"rel="next""#)
        {
            return None;
        }
        Some(
            url_part
                .trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use reqwest::header::HeaderValue;

    use super::*;

    #[test]
    fn next_link_extracts_next_relation() {
        let mut headers = HeaderMap::new();
        headers.insert(
            LINK,
            HeaderValue::from_static(
                r#"</api/v3/repos/alice/project/issues/7/comments?page=2>; rel="next", </api/v3/repos/alice/project/issues/7/comments?page=4>; rel="last""#,
            ),
        );

        assert_eq!(
            next_link(&headers),
            Some("/api/v3/repos/alice/project/issues/7/comments?page=2".into())
        );
    }

    #[test]
    fn next_link_returns_none_without_next_relation() {
        let mut headers = HeaderMap::new();
        headers.insert(
            LINK,
            HeaderValue::from_static(
                r#"</api/v3/repos/alice/project/issues/7/comments?page=1>; rel="prev""#,
            ),
        );

        assert_eq!(next_link(&headers), None);
    }
}
