// Migrated from tools/autoresearch-rs/src/research.rs

//! Semantic Scholar API client.
//!
//! Wraps the S2 graph API for paper search with structured result parsing.
//! Supports year filtering and up to 100 results.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_json::{Value, json};

use crate::search::helpers::*;
use crate::search::options::*;

/// Search papers on Semantic Scholar by query string.
///
/// Supports: year range filter, up to 100 results.
/// S2 does not expose a sort-by-date parameter on the search endpoint;
/// results are always ordered by relevance.
pub fn search(client: &Client, opts: &SearchOptions) -> Result<Vec<Value>> {
    let mut first_non_transient: Option<anyhow::Error> = None;
    for attempt in 0..3 {
        match try_search(client, opts) {
            Ok(results) => return Ok(results),
            Err(e) => {
                let msg = e.to_string();
                let is_transient = msg.contains("503")
                    || msg.contains("502")
                    || msg.contains("429")
                    || msg.contains("timeout")
                    || msg.contains("connection");
                if is_transient && attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_millis(500 * (1 << attempt)));
                    continue;
                }
                if !is_transient && first_non_transient.is_none() {
                    first_non_transient = Some(anyhow::anyhow!("{msg}"));
                }
                if let Some(ctx) = first_non_transient {
                    return Err(ctx);
                }
                return Err(anyhow::anyhow!("S2 search failed: {msg}"));
            }
        }
    }
    Err(anyhow::anyhow!("S2 search failed after 3 retries"))
}

fn try_search(client: &Client, opts: &SearchOptions) -> Result<Vec<Value>> {
    crate::util::validate_url_for_fetch(SEMANTIC_SCHOLAR_BASE_URL)?;

    let mut query_params: Vec<(&str, String)> = vec![
        ("query", opts.query.clone()),
        (
            "fields",
            "title,authors,year,venue,url,abstract,citationCount,externalIds,publicationDate"
                .to_string(),
        ),
        ("limit", normalize_limit(opts.limit).to_string()),
    ];

    // S2 supports year as a single year or hyphen range: "2023" or "2023-2025"
    match (opts.year_from, opts.year_to) {
        (Some(from), Some(to)) => {
            query_params.push(("year", format!("{from}-{to}")));
        }
        (Some(from), None) => {
            query_params.push(("year", from.to_string()));
        }
        (None, Some(to)) => {
            query_params.push(("year", format!("0-{to}")));
        }
        (None, None) => {}
    }

    let mut request = client
        .get(SEMANTIC_SCHOLAR_BASE_URL)
        .header(USER_AGENT, "research-harness/0.1")
        .header(ACCEPT, "application/json");

    for (key, val) in &query_params {
        request = request.query(&[(key, val)]);
    }

    let response: Value = request
        .send()
        .context("Semantic Scholar request failed")?
        .error_for_status()
        .context("Semantic Scholar returned an error")?
        .json()
        .context("Semantic Scholar returned invalid JSON")?;

    let mut results = Vec::new();
    for item in response
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let authors = item
            .get("authors")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|author| author.get("name").and_then(Value::as_str))
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        results.push(json!({
            "source": "Semantic Scholar",
            "title": str_field_default(&item, "title", "_untitled_"),
            "authors": authors,
            "year": item.get("year").cloned().unwrap_or(Value::Null),
            "venue": item.get("venue").cloned().unwrap_or(Value::Null),
            "url": item.get("url").cloned().unwrap_or(Value::Null),
            "abstract": item.get("abstract").cloned().unwrap_or(Value::Null),
            "citation_count": item.get("citationCount").cloned().unwrap_or(Value::Null),
            "external_ids": item.get("externalIds").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(results)
}

// ── Convenience wrapper returning crate::types::Paper ──

/// Search and convert to typed Paper structs.
pub fn search_papers(
    client: &Client,
    opts: &SearchOptions,
) -> Result<Vec<crate::types::Paper>> {
    let raw = search(client, opts)?;
    raw.into_iter().map(json_to_paper).collect()
}

fn json_to_paper(v: Value) -> Result<crate::types::Paper> {
    let title = str_field_default(&v, "title", "_untitled_");
    let id = v
        .get("external_ids")
        .and_then(|ids| ids.get("ArXiv").or(ids.get("DOI")))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let authors: Vec<String> = v
        .get("authors")
        .and_then(Value::as_str)
        .unwrap_or("")
        .split(", ")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let year = v.get("year").and_then(Value::as_u64).map(|y| y as u32);
    Ok(crate::types::Paper {
        id,
        title,
        authors,
        abstract_text: v
            .get("abstract")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        year,
        venue: v.get("venue").and_then(Value::as_str).map(String::from),
        doi: v
            .get("external_ids")
            .and_then(|ids| ids.get("DOI"))
            .and_then(Value::as_str)
            .map(String::from),
        url: v.get("url").and_then(Value::as_str).map(String::from),
        source: crate::types::PaperSource::SemanticScholar,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn json_to_paper_with_arxiv_external_id() {
        let v = json!({
            "title": "A S2 Paper",
            "external_ids": {"ArXiv": "2301.07041"},
            "authors": "Smith, Jane",
            "year": 2023,
            "venue": "NeurIPS",
            "abstract": "A paper from S2.",
            "url": "https://semanticscholar.org/paper/abc"
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.id, "2301.07041");
        assert_eq!(paper.title, "A S2 Paper");
        assert_eq!(paper.year, Some(2023));
        assert_eq!(paper.venue, Some("NeurIPS".to_string()));
    }

    #[test]
    fn json_to_paper_doi_fallback_when_no_arxiv() {
        let v = json!({
            "title": "DOI Only",
            "external_ids": {"DOI": "10.1234/test"},
            "authors": "",
            "year": 2024,
            "abstract": ""
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.id, "10.1234/test");
        assert_eq!(paper.doi, Some("10.1234/test".to_string()));
    }

    #[test]
    fn json_to_paper_no_external_ids() {
        let v = json!({
            "title": "No IDs",
            "external_ids": {},
            "authors": "",
            "year": null,
            "abstract": ""
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.id, "");
        assert_eq!(paper.year, None);
    }

    #[test]
    fn json_to_paper_multiple_authors_from_string() {
        let v = json!({
            "title": "Multi-author",
            "external_ids": {"ArXiv": "2301.07042"},
            "authors": "Alice, Bob, Charlie",
            "year": 2024,
            "abstract": ""
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.authors, vec!["Alice", "Bob", "Charlie"]);
    }

    #[test]
    fn json_to_paper_null_abstract() {
        let v = json!({
            "title": "Null Abstract",
            "external_ids": {"ArXiv": "2301.07043"},
            "authors": "",
            "year": 2024,
            "abstract": null
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.abstract_text, "");
    }

    #[test]
    fn json_to_paper_venue_from_field() {
        let v = json!({
            "title": "Venue Test",
            "external_ids": {"ArXiv": "2301.07045"},
            "authors": "",
            "year": 2024,
            "venue": "ICML",
            "abstract": ""
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.venue, Some("ICML".to_string()));
    }

    #[test]
    fn json_to_paper_doi_from_external() {
        let v = json!({
            "title": "DOI is separate from ID",
            "external_ids": {"ArXiv": "2301.07046", "DOI": "10.5678/test-doi"},
            "authors": "",
            "year": 2024,
            "abstract": ""
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.id, "2301.07046");
        assert_eq!(paper.doi, Some("10.5678/test-doi".to_string()));
    }
}
