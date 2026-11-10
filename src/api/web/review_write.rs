//! Pinned 4.44 AJAX write and permission-gated raw-body read contracts.
use super::GitBucketWebSession;
use crate::domain::review_write::*;
use crate::error::{GbError, Result};
use scraper::{Html, Selector};

impl GitBucketWebSession {
    pub(crate) async fn review_edit_body(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<String> {
        let page = self
            .review_read(&format!(
                "/{owner}/{repo}/commit_comments/_data/{id}?dataType=html"
            ))
            .await?;
        edit_body(&page, id)
    }

    async fn review_read(&self, path: &str) -> Result<String> {
        let url = url::Url::parse(&format!("{}{path}", self.base_url))?;
        let (response, context) = crate::api::transport::send(
            self.client
                .get(url.clone())
                .header("X-Requested-With", "XMLHttpRequest"),
            false,
            &self.secrets,
        )
        .await?;
        let actual = response.url().clone();
        let status = response.status();
        let body = crate::api::transport::read(response, &context, &self.secrets).await?;
        if actual != url || actual.path().ends_with("/signin") {
            return Err(GbError::Auth(
                "Review read did not return the exact authenticated resource.".into(),
            ));
        }
        if !status.is_success() {
            return Err(crate::api::transport::status_error(
                &context,
                &body,
                &self.secrets,
            ));
        }
        String::from_utf8(body).map_err(|_| invalid("Review resource is not UTF-8."))
    }

    pub(crate) async fn review_is_deleted(&self, owner: &str, repo: &str, id: u64) -> Result<bool> {
        match self.review_edit_body(owner, repo, id).await {
            Err(GbError::Api { status: 404, .. }) => Ok(true),
            Ok(_) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn review_submit(&self, path: &str, fields: Vec<(&str, String)>) -> Result<String> {
        let result = self
            .mutate(|client| {
                client
                    .post(format!("{}{path}", self.base_url))
                    .header("X-Requested-With", "XMLHttpRequest")
                    .form(&fields)
            })
            .await?;
        let status = result.response.status();
        let signin = result.response.url().path().ends_with("/signin");
        let body =
            crate::api::transport::read(result.response, &result.context, &self.secrets).await?;
        if !status.is_success() || signin {
            let source = if signin || matches!(status.as_u16(), 401 | 403) {
                GbError::Auth("Web session rejected the review write.".into())
            } else {
                crate::api::transport::status_error(&result.context, &body, &self.secrets)
            };
            return Err(if result.redirected {
                GbError::Unverified {
                    source: Box::new(source),
                }
            } else {
                source
            });
        }
        String::from_utf8(body).map_err(|_| {
            invalid("Review write returned non-UTF-8 data; inspect the target before retrying.")
        })
    }

    pub(crate) async fn create_inline(
        &self,
        owner: &str,
        repo: &str,
        submission: InlineSubmission<'_>,
    ) -> Result<u64> {
        let InlineSubmission {
            number,
            head,
            position,
            body,
            diff,
        } = submission;
        let mut fields = vec![
            ("issueId", number.to_string()),
            ("fileName", position.path.clone()),
            ("content", body.into()),
            ("diff", diff.into()),
        ];
        if let Some(line) = position.old_line {
            fields.push(("oldLineNumber", line.to_string()));
        }
        if let Some(line) = position.new_line {
            fields.push(("newLineNumber", line.to_string()));
        }
        let page = self
            .review_submit(
                &format!("/{owner}/{repo}/commit/{}/comment/_data/new", head.as_str()),
                fields,
            )
            .await?;
        submission_id(&page)
    }

    pub(crate) async fn edit_inline(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
        body: &str,
    ) -> Result<()> {
        self.review_submit(
            &format!("/{owner}/{repo}/commit_comments/edit/{id}"),
            vec![("content", body.into())],
        )
        .await?;
        Ok(())
    }
    pub(crate) async fn delete_inline(&self, owner: &str, repo: &str, id: u64) -> Result<()> {
        self.review_submit(
            &format!("/{owner}/{repo}/commit_comments/delete/{id}"),
            Vec::new(),
        )
        .await?;
        Ok(())
    }
}

fn edit_body(page: &str, id: u64) -> Result<String> {
    let document = Html::parse_document(page);
    let textarea = Selector::parse("textarea[name=content]").expect("static selector");
    let fields: Vec<_> = document.select(&textarea).collect();
    let button = Selector::parse(&format!(
        "input.update-comment-{id}[value='Update comment']"
    ))
    .expect("numeric selector");
    if fields.len() != 1 || document.select(&button).count() != 1 {
        return Err(invalid("Unrecognized permission-gated review edit form."));
    }
    Ok(fields[0].text().collect())
}

fn submission_id(page: &str) -> Result<u64> {
    let document = Html::parse_document(page);
    let selector =
        Selector::parse("body > .commit-comment-box[id^=discussion_r]").expect("static selector");
    let roots: Vec<_> = document.select(&selector).collect();
    if roots.len() != 1 {
        return Err(invalid("Review submission did not return a unique CommitComment identity; inspect before retrying."));
    }
    roots[0]
        .value()
        .attr("id")
        .and_then(|id| id.strip_prefix("discussion_r"))
        .and_then(|id| id.parse().ok())
        .filter(|id| *id > 0)
        .ok_or_else(|| invalid("Invalid submitted review identity."))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_escaped_original_markdown_only_from_the_identified_edit_form() {
        let form = "<textarea name=content>&lt;raw&gt; **本文**\nnext</textarea><input class=update-comment-7 value='Update comment'>";
        assert_eq!(edit_body(form, 7).unwrap(), "<raw> **本文**\nnext");
        assert!(edit_body(form, 8).is_err());
        assert!(edit_body("<form>login</form>", 7).is_err());
        let fragment =
            "<div class=commit-comment-box id=discussion_r7><div id=discussion_r8>body</div></div>";
        assert_eq!(submission_id(fragment).unwrap(), 7);
        assert!(submission_id(
            "<div><div class=commit-comment-box id=discussion_r7>body</div></div>"
        )
        .is_err());
    }
}
