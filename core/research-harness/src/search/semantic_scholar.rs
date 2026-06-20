// Migrated from tools/autoresearch-rs/src/research.rs

//! Semantic Scholar API client.
//!
//! Wraps the S2 graph API for paper search with structured result parsing.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde_json::{Value, json};

use crate::search::helpers::*;

/// Search papers on Semantic Scholar by query string.
///
/// Returns a list of JSON objects with fields: source, title, authors, year,
/// venue, url, abstract, citation_count, external_ids.
pub fn search(client: &Client, query: &str, limit: usize) -> Result<Vec<Value>> {
    let response: Value = client
        .get(SEMANTIC_SCHOLAR_BASE_URL)
        .header(USER_AGENT, "research-harness/0.1")
        .header(ACCEPT, "application/json")
        .query(&[
            ("query", query),
            (
                "fields",
                "title,authors,year,venue,url,abstract,citationCount,externalIds",
            ),
            ("limit", &normalize_limit(limit).to_string()),
        ])
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

/// Search with a default client (20-second timeout).
pub fn search_default(query: &str, limit: usize) -> Result<Vec<Value>> {
    let client = http_client(20)?;
    search(&client, query, limit)
}

// ── Convenience wrapper returning crate::types::Paper ──

/// Search and convert to typed Paper structs.
pub fn search_papers(client: &Client, query: &str, limit: usize) -> Result<Vec<crate::types::Paper>> {
    let raw = search(client, query, limit)?;
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
