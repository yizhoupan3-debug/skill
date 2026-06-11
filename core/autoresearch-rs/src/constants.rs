use regex::Regex;
use std::sync::LazyLock;

pub(crate) const SCHEMA_VERSION: i64 = 4;
pub(crate) const STAGE_BOOTSTRAP: &str = "bootstrap";
pub(crate) const STAGE_INNER_LOOP: &str = "inner-loop";
pub(crate) const STAGE_OUTER_LOOP: &str = "outer-loop";
pub(crate) const STAGE_FINALIZE: &str = "finalize";
pub(crate) const STALE_STATE_DAYS: i64 = 10;
pub(crate) const RECENT_ACTIVITY_DAYS: i64 = 14;
pub(crate) const FALLBACK_ACTIVITY_LIMIT: usize = 3;
pub(crate) const TEMPLATES_RELATIVE: &str = "core/autoresearch-rs/templates";
pub(crate) const DEFAULT_RESEARCH_RESULT_LIMIT: usize = 5;
pub(crate) const DEFAULT_EXTERNAL_TIMEOUT_SECS: u64 = 20;
pub(crate) const SEMANTIC_SCHOLAR_BASE_URL: &str = "https://api.semanticscholar.org/graph/v1/paper/search";
pub(crate) const ARXIV_BASE_URL: &str = "https://export.arxiv.org/api/query";

pub(crate) const FINDINGS_BLOCK_START: &str = "<!-- autoresearch:findings:start -->";
pub(crate) const FINDINGS_BLOCK_END: &str = "<!-- autoresearch:findings:end -->";
pub(crate) const NOVELTY_BLOCK_START: &str = "<!-- autoresearch:novelty:start -->";
pub(crate) const NOVELTY_BLOCK_END: &str = "<!-- autoresearch:novelty:end -->";
pub(crate) const SEARCH_PLAN_BLOCK_START: &str = "<!-- autoresearch:search-plan:start -->";
pub(crate) const SEARCH_PLAN_BLOCK_END: &str = "<!-- autoresearch:search-plan:end -->";
pub(crate) const EXTERNAL_RESEARCH_BLOCK_START: &str = "<!-- autoresearch:external-research:start -->";
pub(crate) const EXTERNAL_RESEARCH_BLOCK_END: &str = "<!-- autoresearch:external-research:end -->";
pub(crate) const CLAIMS_BLOCK_START: &str = "<!-- autoresearch:claims:start -->";
pub(crate) const CLAIMS_BLOCK_END: &str = "<!-- autoresearch:claims:end -->";
pub(crate) const CONTEXT_BLOCK_START: &str = "<!-- autoresearch:context:start -->";
pub(crate) const CONTEXT_BLOCK_END: &str = "<!-- autoresearch:context:end -->";
pub(crate) const REUSE_INDEX_BLOCK_START: &str = "<!-- autoresearch:reuse-index:start -->";
pub(crate) const REUSE_INDEX_BLOCK_END: &str = "<!-- autoresearch:reuse-index:end -->";

pub(crate) static ARXIV_ENTRY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<entry>(.*?)</entry>").expect("arxiv entry regex"));
pub(crate) static ARXIV_AUTHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<author>.*?<name>(.*?)</name>.*?</author>").expect("arxiv author regex")
});
