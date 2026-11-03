use reqwest::StatusCode;
use scraper::{Html, Selector};
use url::Url;

use super::GitBucketWebSession;
use crate::error::{GbError, Result};

impl GitBucketWebSession {
    /// The pinned Web controller updates the numeric label ID in place.
    pub async fn update_label_color(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
        old_color: &str,
        color: &str,
    ) -> Result<()> {
        let path = format!("/{owner}/{repo}/issues/labels");
        let base = Url::parse(&format!("{}/", self.base_url))?;
        let page = self.label_html(&path).await?;
        let id = find_label_id(&page, &base, owner, repo, name)?;
        let edit_path = format!("{path}/{id}/edit");
        let form = self.label_html(&edit_path).await?;
        verify_edit_form(&form, id, name, old_color)?;
        let response = self
            .client
            .post(format!("{}{edit_path}", self.base_url))
            .header("X-Requested-With", "XMLHttpRequest")
            .form(&[
                ("labelName", name.to_string()),
                ("labelColor", format!("#{color}")),
            ])
            .send()
            .await?;
        self.ensure_success(response, "update label color in place")
            .await?;
        let after = self.label_html(&path).await.map_err(|err| GbError::Other(format!(
            "Label Web update was submitted, but label identity verification failed: {err}. Inspect labels before retrying."
        )))?;
        let after_id = find_label_id(&after, &base, owner, repo, name).map_err(|err| GbError::Other(format!(
            "Label Web update was submitted, but its identity cannot be verified: {err}. Inspect labels before retrying."
        )))?;
        if after_id != id {
            return Err(GbError::Other("Label Web update was submitted, but its numeric ID changed. Inspect labels and Issue associations before retrying.".into()));
        }
        Ok(())
    }

    async fn label_html(&self, path: &str) -> Result<String> {
        let expected = Url::parse(&format!("{}{path}", self.base_url))?;
        let response = self
            .client
            .get(expected.clone())
            .header("X-Requested-With", "XMLHttpRequest")
            .send()
            .await?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) || response.url().path().ends_with("/signin")
        {
            return Err(GbError::Auth(
                "Web label access requires a valid, permitted session.".into(),
            ));
        }
        if response.url().origin() != expected.origin() || response.url().path() != expected.path()
        {
            return Err(GbError::Other("Web label metadata redirected outside the expected route. No Web mutation was attempted.".into()));
        }
        if !response.status().is_success() {
            return Err(GbError::Other(format!(
                "Cannot read Web label metadata: HTTP {}",
                response.status().as_u16()
            )));
        }
        Ok(response.text().await?)
    }
}

fn find_label_id(page: &str, base: &Url, owner: &str, repo: &str, name: &str) -> Result<u64> {
    let document = Html::parse_document(page);
    let rows = Selector::parse("tr[id^='label-row-']").expect("static selector");
    let anchors = Selector::parse("a[id^='label-row-content-']").expect("static selector");
    let mut target = base.clone();
    target
        .path_segments_mut()
        .map_err(|_| GbError::Other("Invalid Web label base URL".into()))?
        .pop_if_empty()
        .extend([owner, repo, "issues"]);
    let mut found = Vec::new();
    for row in document.select(&rows) {
        let Some(id) = row
            .value()
            .attr("id")
            .and_then(|id| id.strip_prefix("label-row-"))
            .and_then(|id| id.parse::<u64>().ok())
            .filter(|id| *id > 0)
        else {
            continue;
        };
        for anchor in row.select(&anchors) {
            if anchor.value().attr("id") != Some(format!("label-row-content-{id}").as_str()) {
                continue;
            }
            let Some(url) = anchor
                .value()
                .attr("href")
                .and_then(|href| {
                    crate::api::url::normalize_origin_path_or_url(
                        href,
                        base.as_str().trim_end_matches('/'),
                    )
                    .ok()
                })
                .and_then(|url| Url::parse(&url).ok())
            else {
                continue;
            };
            if url.scheme() != target.scheme()
                || url.host_str() != target.host_str()
                || url.port_or_known_default() != target.port_or_known_default()
                || url.path() != target.path()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                continue;
            }
            let labels: Vec<_> = url
                .query_pairs()
                .filter(|(key, _)| key == "labels")
                .collect();
            if labels.len() == 1 && labels[0].1 == name {
                found.push(id);
            }
        }
    }
    if found.len() != 1 {
        return Err(GbError::Other("Could not uniquely identify this label's numeric ID in the verified Web page. No label replacement or deletion was attempted.".into()));
    }
    Ok(found[0])
}

fn verify_edit_form(page: &str, id: u64, name: &str, color: &str) -> Result<()> {
    let document = Html::parse_fragment(page);
    let value = |key: &str| -> Option<String> {
        let selector = Selector::parse(&format!("input[id='{key}-{id}']")).ok()?;
        let mut matches = document.select(&selector);
        let value = matches.next()?.value().attr("value")?.to_string();
        if matches.next().is_some() {
            return None;
        }
        Some(value)
    };
    if value("labelName").as_deref() != Some(name)
        || value("labelColor")
            .as_deref()
            .map(|value| value.trim_start_matches('#').to_ascii_lowercase())
            .as_deref()
            != Some(color)
    {
        return Err(GbError::Other("Label Web form does not match the API snapshot (or its layout is unsupported). No Web mutation was attempted; refresh and retry.".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sanitized_pinned_server_fragments_with_root_relative_proxy_links() {
        let row = include_str!("../../../tests/fixtures/gitbucket-4.44-label-row.html");
        let form = include_str!("../../../tests/fixtures/gitbucket-4.44-label-edit.html");
        let base = Url::parse("https://example.test/gitbucket/").unwrap();
        assert_eq!(
            find_label_id(
                &format!("<table>{row}</table>"),
                &base,
                "alice",
                "project",
                "needs & review"
            )
            .unwrap(),
            42
        );
        verify_edit_form(form, 42, "needs & review", "123abc").unwrap();
    }

    #[test]
    fn identifies_decoded_name_with_entities_and_exact_deployment() {
        let base = Url::parse("https://example.test/gitbucket/").unwrap();
        let page = r#"<table><tr id="label-row-42"><td><a id="label-row-content-42" href="/gitbucket/a/r/issues?labels=a%26b&amp;state=open">a&amp;b</a></td></tr></table>"#;
        assert_eq!(find_label_id(page, &base, "a", "r", "a&b").unwrap(), 42);
        assert!(find_label_id(
            &page.replace("/gitbucket/a", "/other/a"),
            &base,
            "a",
            "r",
            "a&b"
        )
        .is_err());
        assert!(find_label_id(
            &page.replace("/gitbucket/a", "https://evil.test/gitbucket/a"),
            &base,
            "a",
            "r",
            "a&b"
        )
        .is_err());
    }

    #[test]
    fn rejects_ambiguous_names_or_inconsistent_row_ids() {
        let base = Url::parse("https://example.test/").unwrap();
        let page = r#"<table><tr id="label-row-42"><td><a id="label-row-content-42" href="/a/r/issues?labels=bug">bug</a></td></tr></table>"#;
        assert!(find_label_id(&format!("{page}{page}"), &base, "a", "r", "bug").is_err());
        assert!(find_label_id(
            &page.replace("label-row-content-42", "label-row-content-43"),
            &base,
            "a",
            "r",
            "bug"
        )
        .is_err());
    }

    #[test]
    fn verifies_form_identity_entities_and_single_input_values() {
        let page = r##"<input id="labelName-42" value="a&amp;b&nbsp;"><input id="labelColor-42" value="#ABCDEF">"##;
        verify_edit_form(page, 42, "a&b\u{a0}", "abcdef").unwrap();
        assert!(verify_edit_form(page, 42, "other", "abcdef").is_err());
        assert!(verify_edit_form(&format!("{page}{page}"), 42, "a&b\u{a0}", "abcdef").is_err());
    }
}
