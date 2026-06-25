// Migrated from tools/autoresearch-rs/src/research.rs + helpers.rs + text.rs

//! Internal shared helpers for the search module.
//!
//! Contains utility functions extracted from autoresearch-rs that are used
//! across multiple search submodules: HTTP client building, XML parsing,
//! text compaction, and JSON field access.

use anyhow::Result;
use reqwest::blocking::Client;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use serde_json::Value;
use std::sync::OnceLock;

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

// ── arXiv XML helpers (migrated from autoresearch-rs/src/text.rs) ──

use regex::Regex;

/// Cache of compiled XML tag regexes — avoids recompilation per call.
static XML_TAG_RE_CACHE: LazyLock<Mutex<HashMap<String, Regex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn xml_text_between(raw: &str, tag: &str) -> Option<String> {
    let re = match XML_TAG_RE_CACHE.lock().ok().and_then(|c| c.get(tag).cloned()) {
        Some(r) => r,
        None => {
            let pattern =
                Regex::new(&format!(r"(?s)<{tag}(?:\s[^>]*)?>(.*?)</{tag}>")).ok()?;
            if let Ok(mut cache) = XML_TAG_RE_CACHE.lock() {
                cache.insert(tag.to_string(), pattern.clone());
            }
            pattern
        }
    };
    let captures = re.captures(raw)?;
    Some(decode_xml_entities(captures.get(1)?.as_str().trim()))
}

pub(super) fn decode_xml_entities(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ── arXiv regex patterns (migrated from main.rs constants) ──

pub(super) static ARXIV_ENTRY_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(?s)<entry>(.*?)</entry>").expect("arxiv entry regex")
});
pub(super) static ARXIV_AUTHOR_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(?s)<author>.*?<name>(.*?)</name>.*?</author>").expect("arxiv author regex")
});

// ── Text helpers (migrated from autoresearch-rs/src/text.rs) ──

/// Extract meaningful words from text, supporting both ASCII and CJK characters.
pub(super) fn compact_words(text: &str, limit: usize) -> Vec<String> {
    static WORD_RE: OnceLock<Regex> = OnceLock::new();
    let re = WORD_RE.get_or_init(|| {
        #[allow(clippy::expect_used)]
        Regex::new(r"[A-Za-z0-9][A-Za-z0-9_-]*|[\p{Han}]{2,}").expect("invalid compact_words regex")
    });
    let stops = stopwords();
    let mut filtered = Vec::new();
    for cap in re.find_iter(&text.to_lowercase()) {
        let word = cap.as_str();
        if !word.chars().any(|c| c >= '\u{4e00}') && word.len() <= 2 {
            continue;
        }
        if stops.contains(word) {
            continue;
        }
        if !filtered.iter().any(|item| item == word) {
            filtered.push(word.to_string());
        }
        if filtered.len() >= limit {
            break;
        }
    }
    filtered
}

static STOPWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "in", "into", "is",
        "it", "of", "on", "or", "reduce", "research", "that", "the", "this", "to", "use", "using",
        "with",
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上",
        "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好", "自己", "这",
        "他", "她", "它", "们", "那", "被", "从", "把", "对", "但", "而", "与", "或", "中",
        "等", "能", "可以", "什么", "怎么", "如何", "为什么", "是否", "通过", "使用", "基于",
        "以及", "然后", "因此", "所以", "如果", "虽然", "但是", "对于", "关于", "已经", "正在",
    ]
    .into_iter()
    .collect()
});

fn stopwords() -> &'static HashSet<&'static str> {
    &STOPWORDS
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
