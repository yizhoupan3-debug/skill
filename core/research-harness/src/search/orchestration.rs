// Migrated from tools/autoresearch-rs/src/research.rs

//! Multi-source literature search orchestration.
//!
//! Coordinates searches across Semantic Scholar, arXiv, and paperplain MCP,
//! deduplicates results, and ranks by relevance.

use anyhow::{bail, Result};
use serde_json::{Value, json};

use crate::search::helpers::*;
use crate::types::Paper;

/// Orchestrate a multi-source literature search using raw JSON results.
///
/// Searches across the requested sources, deduplicates by (source, title),
/// and returns the merged result list.
pub fn search_raw(
    query: &str,
    limit: usize,
    source: &ExternalSourceArg,
    timeout_secs: u64,
) -> Result<Value> {
    let client = http_client(timeout_secs)?;
    let mut results = Vec::new();
    let mut errors = Vec::new();

    if matches!(
        source,
        ExternalSourceArg::All | ExternalSourceArg::SemanticScholar
    ) {
        match crate::search::semantic_scholar::search(&client, query, limit) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("semantic-scholar: {err}")),
        }
    }
    if matches!(source, ExternalSourceArg::All | ExternalSourceArg::Arxiv) {
        match crate::search::arxiv::search(&client, query, limit) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("arxiv: {err}")),
        }
    }

    if results.is_empty() && !errors.is_empty() {
        bail!("External research failed: {}", errors.join("; "));
    }

    Ok(json!({
        "query": query,
        "source": source.as_str(),
        "results": dedupe_research_results(results),
        "errors": errors,
    }))
}

/// Orchestrate a multi-source literature search, returning typed Paper structs.
///
/// Searches across the requested sources and deduplicates results.
pub fn search_all(
    query: &str,
    limit: usize,
    source: &ExternalSourceArg,
    timeout_secs: u64,
) -> Result<Vec<Paper>> {
    let client = http_client(timeout_secs)?;
    let mut papers = Vec::new();

    if matches!(
        source,
        ExternalSourceArg::All | ExternalSourceArg::SemanticScholar
    ) {
        match crate::search::semantic_scholar::search_papers(&client, query, limit) {
            Ok(items) => papers.extend(items),
            Err(err) => {
                tracing::warn!("Semantic Scholar search failed: {err}");
            }
        }
    }
    if matches!(source, ExternalSourceArg::All | ExternalSourceArg::Arxiv) {
        match crate::search::arxiv::search_papers(&client, query, limit) {
            Ok(items) => papers.extend(items),
            Err(err) => {
                tracing::warn!("arXiv search failed: {err}");
            }
        }
    }

    // Deduplicate by title (case-insensitive)
    deduplicate_papers(&mut papers);
    papers.truncate(limit);
    Ok(papers)
}

/// Simple convenience wrapper that searches all sources with defaults.
pub fn search(query: &str, limit: usize) -> Result<Vec<Paper>> {
    search_all(query, limit, &ExternalSourceArg::All, 20)
}

/// Deduplicate papers in-place by (source, title) key (case-insensitive).
fn deduplicate_papers(papers: &mut Vec<Paper>) {
    let mut seen = std::collections::HashSet::new();
    papers.retain(|p| {
        let key = format!(
            "{}::{}",
            format!("{:?}", p.source).to_lowercase(),
            p.title.to_lowercase()
        );
        seen.insert(key)
    });
}
