mod label;
mod repository;
pub(crate) use repository::verify_parent_html;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use reqwest::cookie::Jar;
use reqwest::redirect::Policy;
use reqwest::{Client, RequestBuilder, Response, StatusCode};

use crate::error::{GbError, Result};

use super::client::normalize_web_base_url;
use super::transport::{self, Context, Secrets};

#[derive(Clone)]
pub struct GitBucketWebSession {
    client: Client,
    cookies: Arc<Jar>,
    base_url: String,
    principal: String,
    secrets: Secrets,
}

struct MutationResponse {
    response: Response,
    redirected: bool,
    context: Context,
}

fn web_client(
    base_url: &str,
    cookies: Arc<Jar>,
    redirected: Option<Arc<AtomicBool>>,
) -> Result<Client> {
    let base_url = base_url.to_owned();
    Ok(crate::limits::http_builder()?
        .cookie_provider(cookies)
        .redirect(Policy::custom(move |attempt| {
            if let Some(redirected) = &redirected {
                redirected.store(true, Ordering::Relaxed);
            }
            // Fragments do not participate in the HTTP route. Apply the same
            // recursively decoded deployment boundary checks as API requests.
            let mut target = attempt.url().clone();
            target.set_fragment(None);
            if attempt.previous().len() >= 10
                || crate::api::url::normalize_origin_path_or_url(target.as_str(), &base_url)
                    .is_err()
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}

impl GitBucketWebSession {
    pub async fn sign_in(
        hostname: &str,
        username: &str,
        password: &str,
        protocol: &str,
    ) -> Result<Self> {
        let base_url = normalize_web_base_url(hostname, protocol)?;
        let cookies = Arc::new(Jar::default());
        let client = web_client(&base_url, cookies.clone(), None)?;

        let secrets = Secrets::new(&[password]);
        let (response, context) = transport::send(
            client.post(format!("{base_url}/signin")).form(&[
                ("userName", username),
                ("password", password),
                ("hash", ""),
            ]),
            false,
            &secrets,
        )
        .await?;
        let status = response.status();
        transport::read(response, &context, &secrets).await?;

        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(GbError::Auth(format!(
                "GitBucket web sign-in failed for '{}'. Check your username/password.",
                username
            )));
        }

        if !status.is_success() {
            return Err(GbError::Auth(format!(
                "GitBucket web sign-in failed: HTTP {}. Web fallback requires password authentication; interactive SSO/OIDC redirects are unsupported.",
                status.as_u16()
            )));
        }

        // No token or Basic header is attached: this proves the cookie session,
        // rather than accidentally verifying the REST credentials a second time.
        let (response, context) = transport::send(
            client.get(format!("{base_url}/api/v3/user")),
            false,
            &secrets,
        )
        .await?;
        let status = response.status();
        let final_url = response.url().clone();
        let body = transport::read(response, &context, &secrets).await?;
        if !status.is_success()
            || final_url.path()
                != format!(
                    "{}/api/v3/user",
                    url::Url::parse(&base_url)?.path().trim_end_matches('/')
                )
        {
            return Err(GbError::Auth("GitBucket web sign-in failed: the cookie session could not be verified. Check the password; interactive SSO/OIDC is unsupported.".into()));
        }
        let user: crate::models::user::User = serde_json::from_slice(&body).map_err(|_| GbError::Auth("GitBucket web sign-in failed: expected a cookie-authenticated user, but received an unrecognized response (possibly a login/SSO form).".into()))?;
        if user.login.is_empty() || !user.login.eq_ignore_ascii_case(username) {
            return Err(GbError::Auth(format!("GitBucket web sign-in returned user {:?}, expected {:?}. No Web changes were attempted.", user.login, username)));
        }
        Ok(Self {
            client,
            cookies,
            base_url,
            principal: user.login.clone(),
            secrets,
        })
    }

    pub fn principal(&self) -> &str {
        &self.principal
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
            .mutate(|client| {
                client.get(format!(
                    "{}/{owner}/{repo}/issues/milestones/{number}/{action}",
                    self.base_url
                ))
            })
            .await?;
        self.ensure_success(response, "change the milestone state")
            .await
    }

    async fn post_form(&self, path: &str, fields: Vec<(&str, String)>, action: &str) -> Result<()> {
        let response = self
            .mutate(|client| {
                client
                    .post(format!("{}{}", self.base_url, path))
                    .form(&fields)
            })
            .await?;
        self.ensure_success(response, action).await
    }

    async fn mutate(
        &self,
        build: impl FnOnce(&Client) -> RequestBuilder,
    ) -> Result<MutationResponse> {
        // The trace belongs to this request, including when concurrent writes or
        // reads share the session. Cookies remain shared with the verified login.
        let redirected = Arc::new(AtomicBool::new(false));
        let client = web_client(
            &self.base_url,
            self.cookies.clone(),
            Some(redirected.clone()),
        )?;
        let (response, context) = transport::send(build(&client), true, &self.secrets).await?;
        Ok(MutationResponse {
            response,
            redirected: redirected.load(Ordering::Relaxed),
            context,
        })
    }

    async fn ensure_success(&self, result: MutationResponse, action: &str) -> Result<()> {
        let MutationResponse {
            response,
            redirected,
            context,
        } = result;
        let status = response.status();
        let final_path = response.url().path().to_string();
        transport::read(response, &context, &self.secrets).await?;

        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
            || final_path.ends_with("/signin")
        {
            let source = GbError::Auth(format!(
                "GitBucket web session failed while trying to {}. Re-run the command and enter your password again.",
                action
            ));
            return Err(if redirected {
                GbError::Unverified {
                    source: Box::new(source),
                }
            } else {
                source
            });
        }

        if status.is_success() {
            return Ok(());
        }

        let source = GbError::Api { status: status.as_u16(), message: format!("Web request could not {action}. Unexpected redirect or rejected form; verify the target before retrying.") };
        Err(if redirected {
            GbError::Unverified {
                source: Box::new(source),
            }
        } else {
            source
        })
    }
}
