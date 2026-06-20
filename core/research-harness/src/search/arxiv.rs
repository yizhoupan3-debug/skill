// Migrated from tools/autoresearch-rs/src/research.rs

//! arXiv API client.
//!
//! Searches arXiv via the Atom feed API and parses results into JSON or Paper structs.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde_json::{Value, json};

use crate::search::helpers::*;

/// Search arXiv by query string.
///
/// Returns a list of JSON objects with fields: source, title, authors, year,
/// venue, url, abstract, citation_count, external_ids.
///
/// Retries up to 3 times with exponential backoff on transient errors (503, timeout).
pub fn search(client: &Client, query: &str, limit: usize) -> Result<Vec<Value>> {
    let mut last_err = None;
    for attempt in 0..3 {
        match try_search(client, query, limit) {
            Ok(results) => return Ok(results),
            Err(e) => {
                let msg = e.to_string();
                // Only retry on transient errors
                let is_transient = msg.contains("503") || msg.contains("502")
                    || msg.contains("timeout") || msg.contains("connection");
                if is_transient && attempt < 2 {
                    last_err = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(500 * (1 << attempt)));
                    continue;
                }
                return Err(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("arXiv search failed after 3 attempts")))
}

fn try_search(client: &Client, query: &str, limit: usize) -> Result<Vec<Value>> {
    let raw = client
        .get(ARXIV_BASE_URL)
        .header(USER_AGENT, "research-harness/0.1")
        .query(&[
            ("search_query", format!("all:{query}")),
            ("start", "0".to_string()),
            ("max_results", normalize_limit(limit).to_string()),
            ("sortBy", "relevance".to_string()),
            ("sortOrder", "descending".to_string()),
        ])
        .send()
        .context("arXiv request failed")?
        .error_for_status()
        .context("arXiv returned an error")?
        .text()
        .context("arXiv returned invalid text")?;
    let entry_re = &*ARXIV_ENTRY_RE;
    let author_re = &*ARXIV_AUTHOR_RE;
    let mut results = Vec::new();
    for entry in entry_re.captures_iter(&raw) {
        let entry_raw = entry.get(1).map(|item| item.as_str()).unwrap_or("");
        let authors = author_re
            .captures_iter(entry_raw)
            .filter_map(|cap| cap.get(1).map(|item| decode_xml_entities(item.as_str())))
            .take(4)
            .collect::<Vec<_>>()
            .join(", ");
        results.push(json!({
            "source": "arXiv",
            "title": xml_text_between(entry_raw, "title").unwrap_or_else(|| "_untitled_".into()),
            "authors": authors,
            "year": xml_text_between(entry_raw, "published").map(|date| date.chars().take(4).collect::<String>()).unwrap_or_default(),
            "venue": "arXiv",
            "url": xml_text_between(entry_raw, "id").unwrap_or_default(),
            "abstract": xml_text_between(entry_raw, "summary").unwrap_or_default(),
            "citation_count": Value::Null,
            "external_ids": Value::Null,
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
    let url = v.get("url").and_then(Value::as_str).unwrap_or("");
    // arXiv URL contains the ID, e.g. http://arxiv.org/abs/2301.07041
    let id = url
        .rsplit('/')
        .next()
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
    let year = v.get("year").and_then(Value::as_str).and_then(|y| y.parse::<u32>().ok());
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
        venue: Some("arXiv".to_string()),
        doi: None,
        url: Some(url.to_string()),
        source: crate::types::PaperSource::ArXiv,
    })
}
