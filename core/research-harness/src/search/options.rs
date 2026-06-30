//! Search options: configurable parameters for literature search across
//! Semantic Scholar and arXiv.

use super::helpers::ExternalSourceArg;

/// How to sort search results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortBy {
    /// Sort by relevance to the query (default).
    Relevance,
    /// Sort by publication date (newest first).
    Date,
}

impl SortBy {
    pub fn as_str(&self) -> &'static str {
        match self {
            SortBy::Relevance => "relevance",
            SortBy::Date => "date",
        }
    }
}

/// Options for literature search.
///
/// Controls query, sources, filtering, sorting, and result size.
/// Construct with `SearchOptions::new(query)` for defaults, then customize.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Search query (plain text).
    pub query: String,
    /// Max results per source (clamped to 1..100).
    pub limit: usize,
    /// Source to search.
    pub source: ExternalSourceArg,
    /// Minimum publication year (inclusive). Passed as `year` param to S2,
    /// and as `submittedDate` range to arXiv.
    pub year_from: Option<u32>,
    /// Maximum publication year (inclusive).
    pub year_to: Option<u32>,
    /// Sort order.
    pub sort_by: SortBy,
    /// arXiv category filter, comma-separated (e.g. "cs.AI,cs.LG").
    /// Translated to `cat:cs.AI OR cat:cs.LG` in the arXiv query.
    pub categories: Option<String>,
    /// Advanced arXiv native query syntax.
    /// When set, the arXiv `search_query` uses this value directly,
    /// bypassing the `all:"keyword"` wrapping. The `query` field is still
    /// used for Semantic Scholar.
    /// Example: `au:vaswani AND ti:attention` or `cat:cs.LG AND abs:reinforcement`
    pub advanced_query: Option<String>,
    /// HTTP request timeout in seconds per source. Default 20.
    pub timeout_secs: u64,
    /// When true, enable fuzzy matching for arXiv (uses raw query without
    /// `all:` wrapper, allowing arXiv's built-in word-level AND matching).
    /// Semantic Scholar always does fuzzy matching natively.
    pub fuzzy_query: bool,
    /// When true, enable two-pass filtering: fetch extra results then
    /// rank by a composite score that prefers peer-reviewed authoritative
    /// papers (has DOI, known venue, high citations, recent).
    pub prefer_authoritative: bool,
}

impl SearchOptions {
    /// Create new search options with defaults.
    ///
    /// Defaults: limit=20, source=All, sort_by=Relevance, timeout=20s.
    pub fn new(query: impl Into<String>) -> Self {
        SearchOptions {
            query: query.into(),
            limit: 20,
            source: ExternalSourceArg::All,
            year_from: None,
            year_to: None,
            sort_by: SortBy::Relevance,
            categories: None,
            advanced_query: None,
            timeout_secs: 20,
            fuzzy_query: false,
            prefer_authoritative: false,
        }
    }
}
