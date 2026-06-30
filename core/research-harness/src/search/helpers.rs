// Migrated from tools/autoresearch-rs/src/research.rs + helpers.rs + text.rs

//! Internal shared helpers for the search module.
//!
//! Contains utility functions extracted from autoresearch-rs that are used
//! across multiple search submodules: HTTP client building, text compaction,
//! and JSON field access.

use anyhow::Result;
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::HashSet;

// ── Constants ──

pub(super) const SEMANTIC_SCHOLAR_BASE_URL: &str =
    "https://api.semanticscholar.org/graph/v1/paper/search";
pub(super) const ARXIV_BASE_URL: &str = "https://export.arxiv.org/api/query";

// ── HTTP Client ──

/// Build a blocking HTTP client with a bounded timeout.
/// Delegates to the shared factory in `crate::util`.
pub(super) fn http_client(timeout_secs: u64) -> Result<Client> {
    crate::util::blocking_client(timeout_secs)
}

/// Clamp a search result limit to [1, 100].
pub(super) fn normalize_limit(limit: usize) -> usize {
    limit.clamp(1, 100)
}

// ── JSON field helpers (migrated from autoresearch-rs/src/helpers.rs) ──
// NOTE: These return `String` (not `&str`) and use `"-"` as default, unlike
// `crate::util::{str_field, str_field_default}` which return `&str` with `""`.
// The owned return type and non-empty default are intentional for search result
// construction where fields are immediately consumed as owned strings.

pub(super) fn str_field(value: &Value, key: &str) -> String {
    str_field_default(value, key, "-")
}

pub(super) fn str_field_default(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

// ── External source argument (migrated from cli.rs + arg_impls.rs) ──

/// Source selection for external research queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalSourceArg {
    All,
    SemanticScholar,
    Arxiv,
}

impl ExternalSourceArg {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExternalSourceArg::All => "all",
            ExternalSourceArg::SemanticScholar => "semantic-scholar",
            ExternalSourceArg::Arxiv => "arxiv",
        }
    }
}

// ── Deduplication ──

/// Deduplicate search results by (source, title) key (case-insensitive).
/// Preserves original order. Uses HashSet for O(1) lookup.
pub(super) fn dedupe_research_results(results: Vec<Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for result in results {
        let key = format!(
            "{}::{}",
            str_field_default(&result, "source", "-").to_lowercase(),
            str_field_default(&result, "title", "-").to_lowercase()
        );
        if seen.insert(key) {
            deduped.push(result);
        }
    }
    deduped
}
