use scraper::{Html, Selector};

use super::GitBucketWebSession;
use crate::error::{GbError, Result};

impl GitBucketWebSession {
    pub async fn verify_fork_parent(&self, target: &str, source: &str) -> Result<()> {
        let expected = url::Url::parse(&format!("{}/{target}", self.base_url))?;
        let response = self.client.get(expected.clone()).send().await?;
        if !response.status().is_success() || response.url() != &expected {
            return Err(GbError::Other(
                "Cannot verify the fork's Web metadata on the expected repository page.".into(),
            ));
        }
        verify_parent_html(&response.text().await?, &self.base_url, target, source)
    }
}

/// 4.44 omits fork/parent/source from ApiRepository. Its menu template renders
/// parentUserName/parentRepositoryName in this header (not the network origin).
pub(crate) fn verify_parent_html(page: &str, base: &str, target: &str, source: &str) -> Result<()> {
    let document = Html::parse_document(page);
    let selector = |value| Selector::parse(value).expect("static selector");
    let headers: Vec<_> = document
        .select(&selector(".headbar > .container > .head"))
        .collect();
    let error = || {
        GbError::Other("Fork destination or immediate parent could not be verified from the repository header. Unknown layouts and mismatched parents are not accepted.".into())
    };
    let [header] = headers.as_slice() else {
        return Err(error());
    };
    let links: Vec<_> = header.select(&selector(":scope > a.strong")).collect();
    let parents: Vec<_> = header.select(&selector(":scope > .forked > a")).collect();
    let ([link], [parent]) = (links.as_slice(), parents.as_slice()) else {
        return Err(error());
    };
    for (anchor, name) in [(link, target), (parent, source)] {
        let href = anchor.value().attr("href").ok_or_else(error)?;
        let actual = crate::api::url::normalize_origin_path_or_url(href, base)?;
        let expected = url::Url::parse(&format!("{}/{name}", base.trim_end_matches('/')))?;
        if actual != expected.as_str() {
            return Err(error());
        }
    }
    if link.text().collect::<String>().trim() != target.rsplit('/').next().unwrap_or_default()
        || parent.text().collect::<String>().trim() != source
    {
        return Err(error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const PAGE: &str = include_str!("../../../tests/fixtures/gitbucket-4.44-fork-header.html");
    #[test]
    fn verifies_immediate_parent_with_root_relative_proxy_links() {
        for base in ["https://example.test", "https://example.test/gitbucket"] {
            verify_parent_html(PAGE, base, "alice/project", "bob/project").unwrap();
        }
    }
    #[test]
    fn compares_encoded_urls_for_unicode_repository_names() {
        let page = PAGE
            .replace("alice", "アリス")
            .replace("bob", "ボブ")
            .replace("project", "案件");
        verify_parent_html(
            &page,
            "https://example.test/gitbucket",
            "アリス/案件",
            "ボブ/案件",
        )
        .unwrap();
    }
    #[test]
    fn rejects_foreign_links_changed_parent_and_ambiguous_or_missing_headers() {
        for page in [
            PAGE.replace("/bob/project", "/carol/project"),
            PAGE.replace(
                "href=\"/bob/project\"",
                "href=\"https://foreign.test/bob/project\"",
            ),
            format!("{PAGE}{PAGE}"),
            PAGE.replace("class=\"forked\"", "class=\"unknown\""),
            PAGE.replace("/alice/project", "/alice/other"),
        ] {
            assert!(verify_parent_html(
                &page,
                "https://example.test/gitbucket",
                "alice/project",
                "bob/project"
            )
            .is_err());
        }
    }
}
