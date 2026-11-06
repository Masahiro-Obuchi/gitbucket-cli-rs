//! Collection contracts: fixed 25-item Issue/PR pages, or Link/all comments.
use std::collections::{BTreeMap, BTreeSet, HashSet};

use reqwest::header::{HeaderMap, LINK};
use serde::de::DeserializeOwned;
use url::Url;

use super::client::ApiClient;
use crate::error::{GbError, Result};

pub(crate) enum Collection {
    IssuesOrPulls,
    Comments,
}

pub(crate) trait Item {
    fn id(&self) -> u64;
}

impl Item for crate::models::issue::Issue {
    fn id(&self) -> u64 {
        self.number
    }
}
impl Item for crate::models::pull_request::PullRequest {
    fn id(&self) -> u64 {
        self.number
    }
}
impl Item for crate::models::comment::Comment {
    fn id(&self) -> u64 {
        self.id
    }
}

fn incomplete(cause: &'static str, message: &str) -> GbError {
    GbError::Pagination {
        cause,
        message: message.into(),
    }
}

fn page_limit() -> Result<usize> {
    match std::env::var("GB_PAGINATION_MAX_PAGES") {
        Err(std::env::VarError::NotPresent) => Ok(1000),
        value => value
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| (1..=100_000).contains(value))
            .ok_or_else(|| {
                GbError::Config(
                    "GB_PAGINATION_MAX_PAGES must be an integer from 1 through 100000.".into(),
                )
            }),
    }
}

impl ApiClient {
    pub(crate) async fn collection<T: DeserializeOwned + Item>(
        &self,
        path: &str,
        contract: Collection,
    ) -> Result<Vec<T>> {
        let limit = page_limit()?;
        let initial = Url::parse(&super::url::normalize_path_or_url(path, self.base_url())?)?;
        let mut next = initial.clone();
        let mut visited = HashSet::new();
        let mut items = BTreeMap::new();
        let mut linked = false;
        for page_number in 1..=limit {
            if !visited.insert(page_key(&next)?) {
                return Err(incomplete(
                    "pagination_cycle",
                    "A collection page was visited more than once.",
                ));
            }
            let (page, headers, response_url) =
                self.get_with_headers::<Vec<T>>(next.as_str()).await?;
            let count = page.len();
            let mut added = 0;
            for item in page {
                if item.id() == 0 {
                    return Err(incomplete(
                        "invalid_item_id",
                        "A collection item has an invalid zero ID.",
                    ));
                }
                if let std::collections::btree_map::Entry::Vacant(entry) = items.entry(item.id()) {
                    entry.insert(item);
                    added += 1;
                }
            }
            let link = next_link(&headers)?;
            linked |= headers.contains_key(LINK);
            let following = if let Some(link) = link {
                Some(resolve_next(
                    &link,
                    &response_url,
                    &initial,
                    self.base_url(),
                )?)
            } else if !linked && matches!(contract, Collection::IssuesOrPulls) {
                // 4.44 ignores per_page and provides exactly 25 rows without
                // Link headers. Comments instead return all rows: never probe
                // them merely because their count happens to be 25/100.
                if count > 25 {
                    return Err(incomplete("unknown_pagination", "Issue/PR response exceeds the verified 25-item page size without Link metadata."));
                }
                if count == 25 {
                    let mut next = initial.clone();
                    next.query_pairs_mut()
                        .append_pair("page", &(page_number + 1).to_string());
                    Some(next)
                } else {
                    None
                }
            } else {
                None
            };
            if following.is_some() && count > 0 && added == 0 {
                return Err(incomplete("pagination_no_progress", "The server repeated a page without adding any new IDs; pagination may be ignored."));
            }
            match following {
                None => {
                    let mut result: Vec<_> = items.into_values().collect();
                    if matches!(contract, Collection::IssuesOrPulls) {
                        result.reverse();
                    }
                    return Ok(result);
                }
                Some(following) => next = following,
            }
        }
        Err(incomplete("page_limit", "The collection still has another page at GB_PAGINATION_MAX_PAGES. Increase that explicit safety bound only if appropriate."))
    }
}

fn decoded_path(url: &Url) -> Result<String> {
    let mut path = url.path().to_owned();
    for _ in 0..8 {
        let decoded = percent_encoding::percent_decode_str(&path)
            .decode_utf8()
            .map_err(|_| incomplete("unsafe_next_link", "Invalid encoded collection path."))?
            .into_owned();
        if decoded == path {
            return Ok(path);
        }
        path = decoded;
    }
    Err(incomplete(
        "unsafe_next_link",
        "Excessively encoded collection path.",
    ))
}

fn page_key(url: &Url) -> Result<String> {
    let mut query: Vec<_> = url
        .query()
        .unwrap_or_default()
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let decode = |part: &str| {
                percent_encoding::percent_decode_str(&part.replace('+', " ")).collect::<Vec<_>>()
            };
            (decode(key), decode(value))
        })
        .collect();
    query.sort();
    Ok(serde_json::to_string(&(
        url.origin().ascii_serialization(),
        decoded_path(url)?,
        query,
    ))?)
}

fn resolve_next(link: &str, response: &Url, initial: &Url, base: &str) -> Result<Url> {
    if link.chars().any(char::is_control) || link.contains('\\') {
        return Err(incomplete(
            "unsafe_next_link",
            "Ambiguous characters in the next Link.",
        ));
    }
    let joined = response.join(link).map_err(|_| {
        incomplete(
            "unsafe_next_link",
            "The next Link is not a valid URI reference.",
        )
    })?;
    let checked = super::url::normalize_path_or_url(joined.as_str(), base).map_err(|_| incomplete("unsafe_next_link", "The next Link must stay within the configured GitBucket API base without credentials or fragments."))?;
    let mut next = Url::parse(&checked)?;
    if decoded_path(&next)? != decoded_path(initial)? {
        return Err(incomplete(
            "unsafe_next_link",
            "The next Link changed the collection endpoint.",
        ));
    }
    // A query-relative Link replaces the query under RFC resolution. Restore
    // omitted collection filters instead of silently changing closed/all to open.
    for (key, value) in initial
        .query_pairs()
        .filter(|(key, _)| key != "per_page" && key != "page")
    {
        let values: Vec<_> = next
            .query_pairs()
            .filter(|(name, _)| *name == key)
            .map(|(_, value)| value.into_owned())
            .collect();
        if values.is_empty() {
            next.query_pairs_mut().append_pair(&key, &value);
        } else if values.len() != 1 || values[0] != value {
            return Err(incomplete(
                "changed_filter",
                "The next Link changed a collection filter.",
            ));
        }
    }
    Ok(next)
}

// Split Link fields without splitting commas/semicolons inside URI references
// or quoted attributes. Malformed pagination metadata must not imply "last page".
fn split_field(value: &str, delimiter: char) -> Result<Vec<&str>> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut angle = false;
    let mut quote = false;
    let mut escape = false;
    for (index, ch) in value.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if quote && ch == '\\' {
            escape = true;
            continue;
        }
        if !angle && ch == '"' {
            quote = !quote;
        } else if !quote && ch == '<' {
            if angle {
                return Err(incomplete(
                    "invalid_link_header",
                    "Nested Link URI delimiters.",
                ));
            }
            angle = true;
        } else if !quote && ch == '>' {
            if !angle {
                return Err(incomplete(
                    "invalid_link_header",
                    "Unmatched Link URI delimiter.",
                ));
            }
            angle = false;
        } else if !angle && !quote && ch == delimiter {
            result.push(value[start..index].trim());
            start = index + ch.len_utf8();
        }
    }
    if angle || quote || escape {
        return Err(incomplete(
            "invalid_link_header",
            "Unterminated Link header field.",
        ));
    }
    result.push(value[start..].trim());
    Ok(result)
}

fn next_link(headers: &HeaderMap) -> Result<Option<String>> {
    let mut next = BTreeSet::new();
    for header in headers.get_all(LINK) {
        let value = header
            .to_str()
            .map_err(|_| incomplete("invalid_link_header", "Non-text Link header."))?;
        for entry in split_field(value, ',')? {
            let end = entry
                .find('>')
                .filter(|_| entry.starts_with('<'))
                .ok_or_else(|| {
                    incomplete(
                        "invalid_link_header",
                        "Link header is missing URI delimiters.",
                    )
                })?;
            let target = &entry[1..end];
            if target.is_empty() {
                return Err(incomplete("invalid_link_header", "Empty Link target."));
            }
            let suffix = entry[end + 1..].trim();
            if !suffix.is_empty() && !suffix.starts_with(';') {
                return Err(incomplete(
                    "invalid_link_header",
                    "Invalid Link attributes.",
                ));
            }
            let mut relation = None;
            for param in split_field(suffix, ';')?
                .into_iter()
                .filter(|param| !param.is_empty())
            {
                let (name, value) = param
                    .split_once('=')
                    .ok_or_else(|| incomplete("invalid_link_header", "Invalid Link attribute."))?;
                if name.trim().eq_ignore_ascii_case("rel") {
                    if relation.is_some() {
                        return Err(incomplete(
                            "invalid_link_header",
                            "Duplicate Link relation attribute.",
                        ));
                    }
                    let value = value.trim();
                    relation = Some(if value.starts_with('"') {
                        value
                            .strip_prefix('"')
                            .and_then(|v| v.strip_suffix('"'))
                            .ok_or_else(|| {
                                incomplete("invalid_link_header", "Invalid quoted Link relation.")
                            })?
                    } else {
                        value
                    });
                }
            }
            if relation.is_some_and(|value| {
                value
                    .split_ascii_whitespace()
                    .any(|rel| rel.eq_ignore_ascii_case("next"))
            }) {
                next.insert(target.to_string());
            }
        }
    }
    if next.len() > 1 {
        return Err(incomplete(
            "ambiguous_next_link",
            "Multiple different next Links were returned.",
        ));
    }
    Ok(next.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn link_parser_handles_quoted_attributes_and_multiple_fields() {
        let mut headers = HeaderMap::new();
        headers.append(LINK, HeaderValue::from_static("<?page=1>; rel=prev"));
        headers.append(LINK, HeaderValue::from_static("<?page=2&cursor=a,b;c>; title=\"escaped \\\"comma, semicolon;\"; ReL=\"prev NEXT\""));
        assert_eq!(
            next_link(&headers).unwrap().as_deref(),
            Some("?page=2&cursor=a,b;c")
        );
        let mut terminal = HeaderMap::new();
        terminal.insert(LINK, HeaderValue::from_static("<?page=1>; rel=prev"));
        assert_eq!(next_link(&terminal).unwrap(), None);
    }

    #[test]
    fn canonical_page_keys_keep_query_bytes_distinct_and_ignore_pair_order() {
        let parse = |query: &str| {
            Url::parse(&format!("https://example.com/api/v3/issues?{query}")).unwrap()
        };
        assert_eq!(
            page_key(&parse("page=2&state=all")).unwrap(),
            page_key(&parse("state=all&%70age=2")).unwrap()
        );
        assert_eq!(
            page_key(&parse("cursor=two+words")).unwrap(),
            page_key(&parse("cursor=two%20words")).unwrap()
        );
        assert_ne!(
            page_key(&parse("cursor=%FF")).unwrap(),
            page_key(&parse("cursor=%FE")).unwrap()
        );
    }
}
