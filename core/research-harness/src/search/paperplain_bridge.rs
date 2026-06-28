//! MCP paperplain tool bridge.
//!
//! Provides Rust-side helpers for the paperplain MCP tools (fetch_paper, find_paper_by_title,
//! search_research). The actual MCP protocol is handled by the host; this module provides
//! the data models and result parsing.

use crate::types::{Paper, PaperSource};
use anyhow::{Context, Result};

/// Parse a paperplain MCP search_research response into Paper structs.
///
/// Expected response shape (from paperplain's search_research tool):
/// ```json
/// {
///   "papers": [
///     {
///       "title": "...",
///       "authors": ["..."],
///       "abstract": "...",
///       "year": 2024,
///       "url": "...",
///       "doi": "...",
///       "source": "pubmed|arxiv|semantic_scholar"
///     }
///   ]
/// }
/// ```
pub fn parse_search_response(json: &serde_json::Value) -> Result<Vec<Paper>> {
    let papers = json
        .get("papers")
        .or_else(|| json.get("results"))
        .and_then(|v| v.as_array())
        .context("missing 'papers' or 'results' array in response")?;

    let mut result = Vec::new();
    for (i, paper_json) in papers.iter().enumerate() {
        let title = paper_json
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();

        let authors: Vec<String> = paper_json
            .get("authors")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|a| a.as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default();

        let source_str = paper_json
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("manual");
        let source = match source_str.to_ascii_lowercase().as_str() {
            "pubmed" | "health" => PaperSource::PubMed,
            "arxiv" | "arxive" | "cs" => PaperSource::ArXiv,
            "semantic_scholar" | "s2" => PaperSource::SemanticScholar,
            _ => PaperSource::Manual,
        };

        result.push(Paper {
            id: paper_json
                .get("id")
                .or_else(|| paper_json.get("paper_id"))
                .and_then(|v| v.as_str())
                .unwrap_or(&format!("paper-{i}"))
                .to_string(),
            title,
            authors,
            abstract_text: paper_json
                .get("abstract")
                .or_else(|| paper_json.get("abstract_text"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            year: paper_json
                .get("year")
                .and_then(|v| v.as_u64())
                .map(|y| y as u32),
            venue: paper_json
                .get("venue")
                .or_else(|| paper_json.get("journal"))
                .and_then(|v| v.as_str())
                .map(String::from),
            doi: paper_json
                .get("doi")
                .and_then(|v| v.as_str())
                .map(String::from),
            url: paper_json
                .get("url")
                .or_else(|| paper_json.get("source_url"))
                .and_then(|v| v.as_str())
                .map(String::from),
            source,
        });
    }

    Ok(result)
}

/// Parse a paperplain MCP fetch_paper response into a single Paper.
pub fn parse_fetch_response(json: &serde_json::Value) -> Result<Paper> {
    let paper_json = json
        .get("paper")
        .or(Some(json))
        .context("empty fetch_paper response")?;

    let mut papers = parse_search_response(&serde_json::json!({
        "papers": [paper_json]
    }))?;

    papers
        .pop()
        .context("failed to parse paper from fetch response")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn parse_search_response_multiple() {
        let json = serde_json::json!({
            "papers": [
                {
                    "title": "Attention Is All You Need",
                    "authors": ["Vaswani et al."],
                    "abstract": "The dominant sequence...",
                    "year": 2017,
                    "source": "arxiv",
                    "url": "https://arxiv.org/abs/1706.03762"
                },
                {
                    "title": "BERT",
                    "authors": ["Devlin et al."],
                    "year": 2019,
                    "source": "semantic_scholar"
                }
            ]
        });

        let papers = parse_search_response(&json).unwrap();
        assert_eq!(papers.len(), 2);
        assert_eq!(papers[0].title, "Attention Is All You Need");
        assert!(matches!(papers[0].source, PaperSource::ArXiv));
        assert_eq!(papers[1].year, Some(2019));
    }

    #[test]
    fn parse_fetch_single_paper() {
        let json = serde_json::json!({
            "paper": {
                "title": "A Paper",
                "authors": ["Author"],
                "abstract": "Abstract text",
                "year": 2024,
                "doi": "10.1000/test",
                "source": "pubmed"
            }
        });

        let paper = parse_fetch_response(&json).unwrap();
        assert_eq!(paper.title, "A Paper");
        assert_eq!(paper.doi, Some("10.1000/test".to_string()));
        assert!(matches!(paper.source, PaperSource::PubMed));
    }
}
