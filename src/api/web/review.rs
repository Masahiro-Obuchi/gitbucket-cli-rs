//! Verified GitBucket 4.44 conversation markup, not GitHub review REST semantics.
use super::GitBucketWebSession;
use crate::domain::{pull_request::CommitId, review_comment::*};
use crate::error::{GbError, Result};
use scraper::{ElementRef, Html, Selector};
use std::collections::BTreeSet;
use url::Url;

pub(crate) fn unavailable(cause: &'static str, message: &str) -> GbError {
    GbError::ReviewUnavailable {
        cause,
        message: message.into(),
    }
}
fn markup() -> GbError {
    unavailable("unsupported_markup", "Expected the verified GitBucket 4.44 PR conversation layout. No partial review list was returned.")
}
fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("internal CSS selector")
}
fn one<'a>(root: ElementRef<'a>, query: &str) -> Result<ElementRef<'a>> {
    let found: Vec<_> = root.select(&selector(query)).collect();
    match found.as_slice() {
        [element] => Ok(*element),
        _ => Err(markup()),
    }
}
fn text(element: ElementRef<'_>) -> String {
    element
        .text()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn positive(value: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(markup)
}
fn optional_line(element: ElementRef<'_>, name: &str) -> Result<Option<u64>> {
    element.value().attr(name).map(positive).transpose()
}

/// Accept proxy root-relative links only when either interpretation matches the
/// exact selected resource. Foreign/absolute origins are never rebased.
fn href_matches(href: &str, base: &str, expected: &str) -> Result<bool> {
    let expected = Url::parse(expected)?;
    if let Ok(actual) = crate::api::url::normalize_origin_path_or_url(href, base) {
        if Url::parse(&actual)? == expected {
            return Ok(true);
        }
    }
    if href.starts_with('/') && !href.starts_with("//") {
        let candidate = format!("{}{href}", base.trim_end_matches('/'));
        if let Ok(actual) = crate::api::url::normalize_origin_path_or_url(&candidate, base) {
            return Ok(Url::parse(&actual)? == expected);
        }
    }
    Ok(false)
}

impl crate::api::client::ApiClient {
    pub(crate) async fn token_review_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head: &CommitId,
    ) -> Result<Vec<ReviewComment>> {
        let base = self.web_url("");
        let page = self
            .review_html(&format!("{base}/{owner}/{repo}/pull/{number}"))
            .await?;
        parse_conversation(&page, &base, owner, repo, number, head)
    }
}

impl GitBucketWebSession {
    pub(crate) async fn review_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        head: &CommitId,
    ) -> Result<Vec<ReviewComment>> {
        let expected = Url::parse(&format!("{}/{owner}/{repo}/pull/{number}", self.base_url))?;
        let (response, context) =
            crate::api::transport::send(self.client.get(expected.clone()), false, &self.secrets)
                .await?;
        let status = response.status();
        let final_url = response.url().clone();
        let body = crate::api::transport::read(response, &context, &self.secrets).await?;
        if matches!(
            status,
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
        ) || final_url.path().ends_with("/signin")
        {
            return Err(GbError::Auth(
                "Reading review comments requires a valid permitted Web session.".into(),
            ));
        }
        if !status.is_success() {
            return Err(crate::api::transport::status_error(
                &context,
                &body,
                &self.secrets,
            ));
        }
        if final_url != expected {
            return Err(unavailable(
                "web_response",
                "The review page was not returned on the exact selected PR route.",
            ));
        }
        let body = String::from_utf8(body).map_err(|_| {
            unavailable(
                "unsupported_encoding",
                "The review page was not valid UTF-8.",
            )
        })?;
        parse_conversation(&body, &self.base_url, owner, repo, number, head)
    }
}

pub(crate) fn parse_conversation(
    page: &str,
    base: &str,
    owner: &str,
    repo: &str,
    number: u64,
    head: &CommitId,
) -> Result<Vec<ReviewComment>> {
    let document = Html::parse_document(page);
    let root = document.root_element();
    let tab = one(root, "#pullreq-tab > li.active > a")?;
    let url = format!(
        "{}/{owner}/{repo}/pull/{number}",
        base.trim_end_matches('/')
    );
    if !href_matches(tab.value().attr("href").ok_or_else(markup)?, base, &url)? {
        return Err(markup());
    }
    let list = one(root, "#comment-list")?;
    // Required even for zero reviews: a login/unknown empty page is not proof.
    one(list, ":scope > .issue-comment-box > #issueContent")?;
    // Only the pinned top-level conversation nodes are recognized. An unknown
    // review wrapper must not silently become an empty successful collection.
    for element in list.children().filter_map(ElementRef::wrap) {
        let classes: Vec<_> = element.value().classes().collect();
        let event = classes.iter().any(|c| {
            matches!(
                *c,
                "discussion-item-commit"
                    | "discussion-item-refer"
                    | "discussion-item-merge"
                    | "discussion-item-close"
                    | "discussion-item-reopen"
                    | "discussion-item-delete_branch"
                    | "discussion-item-add-label"
                    | "discussion-item-delete-label"
                    | "discussion-item-change-priority"
                    | "discussion-item-change-milestone"
                    | "discussion-item-assign"
                    | "discussion-item-pencil"
                    | "discussion-item-delete-comment"
            )
        });
        if element.value().name() != "script"
            && !(element.value().name() == "div"
                && (classes.contains(&"panel") || (classes.contains(&"discussion-item") && event)))
        {
            return Err(markup());
        }
    }
    let repository = format!("{owner}/{repo}");
    let mut comments = Vec::new();
    let mut ids = BTreeSet::new();
    for panel in list.select(&selector(":scope > div.panel")) {
        if panel.value().classes().any(|c| c == "issue-comment-box") {
            continue;
        }
        let inline: Vec<_> = panel
            .select(&selector(
                ":scope > div[class^='fold-comments-'] > .panel-body > .commit-comment-box",
            ))
            .collect();
        let whole = panel.value().classes().any(|c| c == "commit-comment-box");
        if inline.is_empty() && !whole {
            return Err(markup());
        }
        if whole && !inline.is_empty() {
            return Err(markup());
        }
        let (items, path, thread) = if whole {
            (vec![panel], None, None)
        } else {
            let heading = one(panel, ":scope > .panel-heading > span.monospace")?;
            let path = text(heading);
            if path.is_empty() {
                return Err(markup());
            }
            let group = one(panel, ":scope > div[class^='fold-comments-']")?;
            let classes: Vec<_> = group
                .value()
                .classes()
                .filter(|c| c.starts_with("fold-comments-"))
                .collect();
            let [class] = classes.as_slice() else {
                return Err(markup());
            };
            positive(class.trim_start_matches("fold-comments-"))?;
            (inline, Some(path), Some(class.to_string()))
        };
        let start = comments.len();
        for element in items {
            let (id, kind, header, body, position, html_url) = if whole {
                let classes: Vec<_> = element
                    .value()
                    .classes()
                    .filter_map(|c| c.strip_prefix("commit-comment-"))
                    .filter(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
                    .collect();
                let [id] = classes.as_slice() else {
                    return Err(markup());
                };
                let id = positive(id)?;
                let header = one(element, ":scope > .panel-heading")?;
                let link = one(header, ":scope > span.muted > a.monospace")?;
                let href = link.value().attr("href").ok_or_else(markup)?;
                let sha = CommitId::parse(href.rsplit('/').next().ok_or_else(markup)?)
                    .map_err(|_| markup())?;
                if !href_matches(
                    href,
                    base,
                    &format!(
                        "{}/{repository}/commit/{}",
                        base.trim_end_matches('/'),
                        sha.as_str()
                    ),
                )? {
                    return Err(markup());
                }
                let body = one(element, &format!(":scope > .commit-commentContent-{id}"))?;
                (
                    id,
                    CommentKind::Commit,
                    header,
                    body,
                    ReviewPosition {
                        source: "web_display",
                        validated_against_git: false,
                        path: None,
                        old_line: None,
                        new_line: None,
                        commit_id: Some(sha.as_str().into()),
                        original_commit_id: None,
                        original_old_line: None,
                        original_new_line: None,
                        outdated: None,
                    },
                    url.clone(),
                )
            } else {
                let id = positive(
                    element
                        .value()
                        .attr("id")
                        .and_then(|id| id.strip_prefix("discussion_r"))
                        .ok_or_else(markup)?,
                )?;
                if element.value().attr("filename") != path.as_deref() {
                    return Err(markup());
                }
                let old_line = optional_line(element, "oldline")?;
                let new_line = optional_line(element, "newline")?;
                let current = element.value().classes().any(|c| c == "inline-comment");
                let container = one(
                    element,
                    &format!(":scope > .commit-comment-{id} > .markdown-body"),
                )?;
                let header = one(container, ":scope > div:first-child")?;
                let body = one(container, &format!(":scope > .commit-commentContent-{id}"))?;
                (
                    id,
                    CommentKind::Inline,
                    header,
                    body,
                    ReviewPosition {
                        source: "web_display",
                        validated_against_git: false,
                        path: path.clone(),
                        old_line,
                        new_line,
                        commit_id: current.then(|| head.as_str().into()),
                        original_commit_id: None,
                        original_old_line: None,
                        original_new_line: None,
                        outdated: Some(!current),
                    },
                    format!("{url}#discussion_r{id}"),
                )
            };
            if !ids.insert(id) {
                return Err(unavailable("duplicate_review_id","The Web page repeated a CommitComment ID. No partial review list was returned."));
            }
            let author_link = one(header, ":scope > a.username")?;
            let author = text(author_link);
            crate::target::validate_component(&author).map_err(|_| markup())?;
            if !href_matches(
                author_link.value().attr("href").ok_or_else(markup)?,
                base,
                &format!("{}/{author}", base.trim_end_matches('/')),
            )? {
                return Err(markup());
            }
            let time = one(header, "span.muted span[data-toggle='tooltip'][title]")?;
            let created_at = time.value().attr("title").ok_or_else(markup)?.to_string();
            time::PrimitiveDateTime::parse(
                &created_at,
                time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
            )
            .map_err(|_| markup())?;
            comments.push(ReviewComment {
                identity: CommentIdentity {
                    instance: base.trim_end_matches('/').into(),
                    kind,
                    repository: repository.clone(),
                    id,
                },
                visible_on_pull_request: number,
                pull_request: None,
                author,
                body: RenderedBody {
                    format: "rendered_html",
                    html: body.inner_html(),
                    text: text(body),
                    markdown: None,
                },
                created_at,
                timestamp_format: "server_local_datetime",
                timezone: None,
                updated_at: None,
                html_url,
                position,
                thread: thread.as_ref().map(|id| ReviewThread {
                    id: id.clone(),
                    relation: "web_position_group",
                    in_reply_to: None,
                }),
            });
        }
        if let Some(thread) = thread {
            let anchor = positive(thread.trim_start_matches("fold-comments-"))?;
            if !comments[start..]
                .iter()
                .any(|comment| comment.identity.id == anchor)
            {
                return Err(markup());
            }
        }
    }
    comments.sort_by_key(|comment| comment.identity.id);
    Ok(comments)
}

#[cfg(test)]
mod tests {
    use super::*;
    const PAGE: &str =
        include_str!("../../../tests/fixtures/gitbucket-4.44-review-conversation.html");
    fn parse(page: &str, base: &str) -> Result<Vec<ReviewComment>> {
        parse_conversation(
            page,
            base,
            "alice",
            "project",
            1,
            &CommitId::parse(&"a".repeat(40)).unwrap(),
        )
    }
    #[test]
    fn reads_captured_inline_old_new_group_and_whole_commit_without_inventing_originals() {
        for base in [
            "https://example.test",
            "https://example.test/gitbucket",
            "https://example.test/alice",
        ] {
            let rows = parse(PAGE, base).unwrap();
            if base == "https://example.test" {
                let expected: serde_json::Value = serde_json::from_str(include_str!(
                    "../../../tests/fixtures/json/review-comment.json"
                ))
                .unwrap();
                assert_eq!(serde_json::to_value(&rows[0]).unwrap(), expected);
            }
            assert_eq!(rows.len(), 5);
            assert_eq!(
                rows[0].identity.to_string(),
                format!("inline:{base}/alice/project:1")
            );
            assert_eq!(rows[0].position.path.as_deref(), Some("new file.txt"));
            assert_eq!(rows[0].position.new_line, Some(1));
            assert_eq!(
                rows[0].position.commit_id.as_deref(),
                Some("a".repeat(40).as_str())
            );
            assert_eq!(
                rows[0].thread.as_ref().unwrap().id,
                rows[1].thread.as_ref().unwrap().id
            );
            assert!(rows[1].thread.as_ref().unwrap().in_reply_to.is_none());
            assert_eq!(rows[2].position.old_line, Some(1));
            assert!(rows[0].body.html.contains("<strong>review</strong>"));
            assert!(rows[0].body.markdown.is_none());
            assert!(
                rows[0].position.original_commit_id.is_none()
                    && rows[0].position.original_new_line.is_none()
            );
            assert_eq!(rows[4].identity.kind, CommentKind::Commit);
            assert!(rows[4].html_url.ends_with("/alice/project/pull/1"));
            assert!(rows.iter().all(|row| row.author == "alice"
                && row.timezone.is_none()
                && row.updated_at.is_none()));
        }
    }
    #[test]
    fn outdated_commit_is_unknown_and_rendered_content_cannot_spoof_another_record() {
        let page = PAGE.replacen("commit-comment-box inline-comment", "commit-comment-box", 1);
        let rows = parse(&page, "https://example.test").unwrap();
        assert_eq!(rows[0].position.outdated, Some(true));
        assert!(rows[0].position.commit_id.is_none());
        let page = PAGE.replace(
            "<p>新行",
            "<div class=\"commit-comment-box\" id=\"discussion_r999\">fake</div><p>新行",
        );
        assert_eq!(parse(&page, "https://example.test").unwrap().len(), 5);
    }
    #[test]
    fn rejects_unknown_or_ambiguous_layouts_and_invalid_positions_without_partial_records() {
        for page in [
            PAGE.replace("id=\"comment-list\"", "id=\"other-list\""),
            PAGE.replace("id=\"discussion_r2\"", "id=\"discussion_r1\""),
            PAGE.replace("newline=\"1\"", "newline=\"0\""),
            PAGE.replace("newline=\"1\"", "newline=\"invalid\""),
            PAGE.replace("filename=\"README.md\"", "filename=\"other.md\""),
            PAGE.replace("commit-commentContent-3", "missing-content-3"),
            PAGE.replace("2026-11-08 12:00:00", "unknown timestamp"),
            PAGE.replace(
                "href=\"/alice/project/pull/1\"",
                "href=\"https://foreign.invalid/alice/project/pull/1\"",
            ),
            PAGE.replace(
                "<div id=\"comment-list\">",
                "<div id=\"comment-list\"><article>unsupported review layout</article>",
            ),
            "<html><form action='/signin'>Sign in</form></html>".into(),
        ] {
            assert!(parse(&page, "https://example.test").is_err());
        }
    }
    #[test]
    fn zero_reviews_requires_the_conversation_identity_and_body() {
        let doc = Html::parse_document(PAGE);
        let tab = doc.select(&selector("#pullreq-tab")).next().unwrap().html();
        let issue = doc
            .select(&selector("#comment-list > .issue-comment-box"))
            .next()
            .unwrap()
            .html();
        let page = format!("{tab}<div id='comment-list'>{issue}</div>");
        assert!(parse(&page, "https://example.test").unwrap().is_empty());
        assert!(parse(
            &page.replace("</div>", "<article>unknown</article></div>"),
            "https://example.test"
        )
        .is_err());
    }
}

#[cfg(test)]
mod optional_position_tests {
    use super::*;
    #[test]
    fn file_level_and_two_sided_display_values_are_kept_without_inventing_validation() {
        let page = include_str!("../../../tests/fixtures/gitbucket-4.44-review-conversation.html");
        for (replacement, old, new) in [
            ("", None, None),
            ("newline=\"2\" oldline=\"3\"", Some(3), Some(2)),
        ] {
            let page = page.replacen("newline=\"1\"", replacement, 1);
            let rows = parse_conversation(
                &page,
                "https://example.test",
                "alice",
                "project",
                1,
                &CommitId::parse(&"a".repeat(40)).unwrap(),
            )
            .unwrap();
            assert_eq!(rows[0].position.old_line, old);
            assert_eq!(rows[0].position.new_line, new);
            assert!(!rows[0].position.validated_against_git);
        }
    }
}
