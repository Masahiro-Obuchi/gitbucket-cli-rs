use reqwest::redirect::Policy;
use reqwest::{Client, Response, StatusCode};

use crate::error::{GbError, Result};

use super::client::normalize_web_base_url;

#[derive(Debug, Clone)]
pub struct GitBucketWebSession {
    client: Client,
    base_url: String,
}

impl GitBucketWebSession {
    pub async fn sign_in(
        hostname: &str,
        username: &str,
        password: &str,
        protocol: &str,
    ) -> Result<Self> {
        let base_url = normalize_web_base_url(hostname, protocol)?;
        let client = Client::builder()
            .cookie_store(true)
            .redirect(Policy::limited(10))
            .build()?;

        let response = client
            .post(format!("{base_url}/signin"))
            .form(&[("userName", username), ("password", password), ("hash", "")])
            .send()
            .await?;

        let status = response.status();
        let final_path = response.url().path().to_string();
        let body = response.text().await.unwrap_or_default();

        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
            || (final_path.ends_with("/signin") && body.contains("incorrect"))
        {
            return Err(GbError::Auth(format!(
                "GitBucket web sign-in failed for '{}'. Check your username/password.",
                username
            )));
        }

        if !status.is_success() && !status.is_redirection() {
            return Err(GbError::Other(format!(
                "GitBucket web sign-in failed: HTTP {}",
                status.as_u16()
            )));
        }

        Ok(Self { client, base_url })
    }

    pub async fn fork_repo(&self, owner: &str, repo: &str, account: &str) -> Result<()> {
        self.post_form(
            &format!("/{owner}/{repo}/fork"),
            vec![("account", account.to_string())],
            "fork the repository",
        )
        .await
    }

    pub async fn delete_repo(&self, owner: &str, repo: &str) -> Result<()> {
        self.post_form(
            &format!("/{owner}/{repo}/settings/delete"),
            Vec::new(),
            "delete the repository",
        )
        .await
    }

    pub async fn update_issue_state(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        action: &str,
    ) -> Result<()> {
        self.post_form(
            &format!("/{owner}/{repo}/issue_comments/state"),
            vec![
                ("issueId", number.to_string()),
                ("content", String::new()),
                ("action", action.to_string()),
            ],
            "change the issue state",
        )
        .await
    }

    pub async fn edit_issue_title(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        title: &str,
    ) -> Result<()> {
        self.post_form(
            &format!("/{owner}/{repo}/issues/edit_title/{number}"),
            vec![("title", title.to_string())],
            "edit the issue title",
        )
        .await
    }

    pub async fn edit_issue_content(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        title: &str,
        content: &str,
    ) -> Result<()> {
        self.post_form(
            &format!("/{owner}/{repo}/issues/edit/{number}"),
            vec![
                ("title", title.to_string()),
                ("content", content.to_string()),
            ],
            "edit the issue",
        )
        .await
    }

    pub async fn update_issue_milestone(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        milestone_id: Option<u64>,
    ) -> Result<()> {
        self.post_form(
            &format!("/{owner}/{repo}/issues/{number}/milestone"),
            vec![(
                "milestoneId",
                milestone_id
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            )],
            "update the issue milestone",
        )
        .await
    }

    pub async fn update_issue_assignee(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        action: &str,
        assignee: &str,
    ) -> Result<()> {
        let path_action = match action {
            "add" => "new",
            "remove" => "delete",
            other => {
                return Err(GbError::Other(format!(
                    "Invalid assignee action '{}'. Expected add or remove.",
                    other
                )))
            }
        };

        self.post_form(
            &format!("/{owner}/{repo}/issues/{number}/assignee/{path_action}"),
            vec![("assigneeUserName", assignee.to_string())],
            "update the issue assignee",
        )
        .await
    }

    pub async fn create_milestone(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        description: Option<&str>,
        due_date: Option<&str>,
    ) -> Result<()> {
        self.post_form(
            &format!("/{owner}/{repo}/issues/milestones/new"),
            vec![
                ("title", title.to_string()),
                ("description", description.unwrap_or_default().to_string()),
                ("dueDate", due_date.unwrap_or_default().to_string()),
            ],
            "create the milestone",
        )
        .await
    }

    pub async fn edit_milestone(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        title: &str,
        description: Option<&str>,
        due_date: Option<&str>,
    ) -> Result<()> {
        self.post_form(
            &format!("/{owner}/{repo}/issues/milestones/{number}/edit"),
            vec![
                ("title", title.to_string()),
                ("description", description.unwrap_or_default().to_string()),
                ("dueDate", due_date.unwrap_or_default().to_string()),
            ],
            "edit the milestone",
        )
        .await
    }

    pub async fn update_milestone_state(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        state: &str,
    ) -> Result<()> {
        let action = match state {
            "open" | "close" => state,
            other => {
                return Err(GbError::Other(format!(
                    "Invalid milestone state action '{}'. Expected open or close.",
                    other
                )))
            }
        };

        let response = self
            .client
            .get(format!(
                "{}/{owner}/{repo}/issues/milestones/{number}/{action}",
                self.base_url
            ))
            .send()
            .await?;
        self.ensure_success(response, "change the milestone state")
            .await
    }

    pub async fn delete_milestone(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let response = self
            .client
            .get(format!(
                "{}/{owner}/{repo}/issues/milestones/{number}/delete",
                self.base_url
            ))
            .send()
            .await?;
        self.ensure_success(response, "delete the milestone").await
    }

    async fn post_form(&self, path: &str, fields: Vec<(&str, String)>, action: &str) -> Result<()> {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .form(&fields)
            .send()
            .await?;
        self.ensure_success(response, action).await
    }

    async fn ensure_success(&self, response: Response, action: &str) -> Result<()> {
        let status = response.status();
        let final_path = response.url().path().to_string();
        let body = response.text().await.unwrap_or_default();

        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
            || final_path.ends_with("/signin")
        {
            return Err(GbError::Auth(format!(
                "GitBucket web session failed while trying to {}. Re-run the command and enter your password again.",
                action
            )));
        }

        if status.is_success() || status.is_redirection() {
            return Ok(());
        }

        let suffix = if body.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", body.trim())
        };
        Err(GbError::Other(format!(
            "Failed to {}: HTTP {}{}",
            action,
            status.as_u16(),
            suffix
        )))
    }
}
