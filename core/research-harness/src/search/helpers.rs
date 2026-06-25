// Migrated from tools/autoresearch-rs/src/research.rs + helpers.rs + text.rs

//! Internal shared helpers for the search module.
//!
//! Contains utility functions extracted from autoresearch-rs that are used
//! across multiple search submodules: HTTP client building, XML parsing,
//! text compaction, and JSON field access.

use anyhow::Result;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;

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

/// Clamp a search result limit to [1, 20].
pub(super) fn normalize_limit(limit: usize) -> usize {
    limit.clamp(1, 20)
}

// ── arXiv XML helpers (delegated to crate::text) ──

// NOTE: xml_text_between, decode_xml_entities, compact_words, and stopwords
// are implemented in text.rs and re-exported here for search-internal callers.

pub(super) use crate::text::{xml_text_between, decode_xml_entities, compact_words};

// ── arXiv regex patterns (migrated from main.rs constants) ──

pub(super) static ARXIV_ENTRY_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(?s)<entry>(.*?)</entry>").expect("arxiv entry regex")
});
pub(super) static ARXIV_AUTHOR_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(?s)<author>.*?<name>(.*?)</name>.*?</author>").expect("arxiv author regex")
});

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
/// Preserves original order (uses Vec position check instead of HashSet).
pub(super) fn dedupe_research_results(results: Vec<Value>) -> Vec<Value> {
    let mut seen = Vec::new();
    let mut deduped = Vec::new();
    for result in results {
        let key = format!(
            "{}::{}",
            str_field_default(&result, "source", "-").to_lowercase(),
            str_field_default(&result, "title", "-").to_lowercase()
        );
        if !seen.contains(&key) {
            seen.push(key);
            deduped.push(result);
        }
    }
    deduped
}
