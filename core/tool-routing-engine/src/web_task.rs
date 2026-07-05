//! Web task classification — encoded decision tree from skills/web-tools/SKILL.md.
//!
//! This module classifies user queries into web task categories using signal
//! detection (vote counting), enabling the tool routing engine to apply
//! domain-specific adjustments when the user needs web fetch, browser
//! interaction, academic search, or other web-related tasks.
//!
//! ## Architecture
//!
//! - Signal detection functions (`needs_browser`, `is_academic`, etc.) scan the
//!   query for CN/EN keyword signals.
//! - `classify_web_task()` collects signal votes and picks the best category.
//! - `adjust_or_validate_decision()` is called post-pick in `route_tool_from_records()`
//!   to check classification consistency. It does NOT modify `score_tool()`.
//!
//! ## Design decisions
//!
//! - **Vote counting, not hardcoded priority chain.** The category with the
//!   most signal votes wins. This handles mixed queries like "打开arXiv搜论文"
//!   correctly (academic > browser when signals tilt that way).
//! - **Small adjustment weights (5-10).** This avoids overriding the base
//!   scoring pipeline — web_task is a tie-breaker, not a dominant signal.
//! - **De-amplifiers for academic signals.** "保存论文" or "写论文" include
//!   academic keywords but aren't research searches — de-amplifiers reduce
//!   false positives.
//! - **No changes to `score_tool()`.** All adjustments happen post-pick in
//!   `route_tool_from_records()`, so `search_tools()` is unaffected.

use crate::types::McpToolDecision;

// ---------------------------------------------------------------------------
// Web task categories
// ---------------------------------------------------------------------------

/// Web task category — each maps to one or more tool slugs used by the
/// routing engine's post-pick adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WebTaskCategory {
    /// Simple HTTP fetch, no JS needed → web_fetch
    SimpleFetch,
    /// Interactive browser actions (open, click, fill) → browser_open
    BrowserInteractive,
    /// Take a screenshot → browser_screenshot
    BrowserScreenshot,
    /// Single paper lookup → search_research / fetch_paper
    AcademicSearch,
    /// Batch literature survey → research_literature_search
    LiteratureBatch,
    /// General web search (informational) → WebSearch / Exa
    WebSearch,
    /// Network request inspection → browser_get_network
    NetworkMonitor,
    /// Not a web task at all
    NotWebTask,
}

impl WebTaskCategory {
    /// Human-readable label for logging.
    pub fn label(&self) -> &'static str {
        match self {
            WebTaskCategory::SimpleFetch => "simple-fetch",
            WebTaskCategory::BrowserInteractive => "browser-interactive",
            WebTaskCategory::BrowserScreenshot => "browser-screenshot",
            WebTaskCategory::AcademicSearch => "academic-search",
            WebTaskCategory::LiteratureBatch => "literature-batch",
            WebTaskCategory::WebSearch => "web-search",
            WebTaskCategory::NetworkMonitor => "network-monitor",
            WebTaskCategory::NotWebTask => "not-web-task",
        }
    }
}

// ---------------------------------------------------------------------------
// Classification types
// ---------------------------------------------------------------------------

/// Signal counts for each dimension — used for vote-based classification.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WebTaskSignalCounts {
    pub browser_signals: usize,
    pub screenshot_signals: usize,
    pub academic_signals: usize,
    pub informational_signals: usize,
    pub network_signals: usize,
    pub url_signals: usize,
    /// Signals that weaken academic classification (e.g., "保存论文").
    pub academic_deamplifiers: usize,
}

/// Full classification result for a user query.
#[derive(Debug, Clone)]
pub struct WebTaskClassification {
    pub category: WebTaskCategory,
    pub signal_counts: WebTaskSignalCounts,
    /// Confidence in [0.0, 1.0] — based on signal_count / max_possible.
    pub confidence: f64,
    /// Human-readable reason for the classification.
    pub reason: String,
}

/// Minimum academic signals required to classify as AcademicSearch/LiteratureBatch.
/// Must match `WebTaskBoostConfig::academic_min_signals` in `scoring_config.rs`.
const ACADEMIC_MIN_SIGNALS: usize = 2;

// ---------------------------------------------------------------------------
// Signal constants — both simplified Chinese and English
// ---------------------------------------------------------------------------

const BROWSER_SIGNALS_CN: &[&str] = &[
    "打开", "点击", "浏览器", "填写", "输入", "表单", "交互", "滚动",
    "导航", "按键", "等待", "跳转", "标签", "页面跳转",
    "点一下", "点开", "填一下", "按一下",
];
const BROWSER_SIGNALS_EN: &[&str] = &[
    "open", "click", "browser", "fill", "interact", "scroll",
    "navigate", "press", "wait", "launch", "visit", "go to",
];

const SCREENSHOT_SIGNALS_CN: &[&str] = &[
    "截图", "截屏", "屏幕捕获", "页面快照", "屏幕截图", "页面截图",
];
const SCREENSHOT_SIGNALS_EN: &[&str] = &[
    "screenshot", "capture", "snapshot",
];

const ACADEMIC_SIGNALS_CN: &[&str] = &[
    "论文", "文献", "学术", "研究", "期刊", "会议", "投稿", "审稿",
    "论文搜索", "论文查找", "学术搜索",
];
const ACADEMIC_SIGNALS_EN: &[&str] = &[
    "paper", "publication", "academic", "journal", "conference",
    "proceedings", "thesis", "dissertation", "study", "literature",
    "research", "scholar", "doi", "arxiv",
];

/// Words that weaken academic intent when co-occurring with academic signals.
/// E.g. "保存这篇论文" → save intent, not research intent.
const ACADEMIC_DEAMPLIFIERS_CN: &[&str] = &[
    "保存", "写", "格式", "管理", "处理", "下载", "上传", "打印",
];
const ACADEMIC_DEAMPLIFIERS_EN: &[&str] = &[
    "save", "write", "format", "edit", "download", "upload", "print",
];

const INFORMATIONAL_SIGNALS_CN: &[&str] = &[
    "查资料", "查一下", "搜一搜", "调查", "了解一下", "搜索", "搜搜",
    "查查", "百度", "搜索引擎",
];
const INFORMATIONAL_SIGNALS_EN: &[&str] = &[
    "search", "find out", "look up", "investigate", "what is",
    "how to", "find",
];

const NETWORK_SIGNALS_CN: &[&str] = &[
    "网络请求", "网络监听", "请求监控", "网络日志", "抓包",
];
const NETWORK_SIGNALS_EN: &[&str] = &[
    "network", "request monitor", "network log",
];

// ---------------------------------------------------------------------------
// Signal detection helpers
// ---------------------------------------------------------------------------

/// Count how many of `signals` are found in `query_lower`.
fn count_signals(query_lower: &str, signals: &[&str]) -> usize {
    signals
        .iter()
        .filter(|s| query_lower.contains(*s))
        .count()
}

/// Check if query contains a URL pattern (http/https or bare domain).
fn has_url_reference(query_lower: &str) -> bool {
    // Explicit protocol
    if query_lower.contains("http://") || query_lower.contains("https://") {
        return true;
    }
    // Bare domain detection: find dots surrounded by ASCII alphanumeric chars.
    // Only examine ASCII regions to avoid CJK false positives (e.g. "。" or "，").
    // Also reject all-numeric TLDs to avoid version numbers like "v1.2" or "3.14".
    let bytes = query_lower.as_bytes();
    for i in 1..bytes.len().saturating_sub(2) {
        if bytes[i] == b'.'
            && bytes[i - 1].is_ascii_alphanumeric()
            && bytes[i + 1].is_ascii_alphanumeric()
        {
            // Peek ahead: check that the TLD chars are not ALL digits
            // (version numbers like "v1.2", decimals like "3.14")
            let mut tld_pos = i + 1;
            let mut all_digits = true;
            while tld_pos < bytes.len() && bytes[tld_pos].is_ascii_alphanumeric() {
                if !bytes[tld_pos].is_ascii_digit() {
                    all_digits = false;
                }
                tld_pos += 1;
            }
            if !all_digits && tld_pos > i + 1 {
                return true;
            }
        }
    }
    false
}

/// Check if query contains an arXiv ID pattern (e.g., "2301.07041", "2301.07041v2").
///
/// Only scans ASCII byte regions to avoid CJK multi-byte interference.
/// Defensive: if fewer than 10 ASCII bytes are present, immediately returns false
/// since arXiv IDs are pure ASCII.
fn has_arxiv_id(query_lower: &str) -> bool {
    let bytes = query_lower.as_bytes();
    if bytes.len() < 10 {
        return false;
    }
    // Early exit: count ASCII digit-like bytes. arXiv IDs need at least 9 ASCII digits + dot.
    // If the ASCII byte count is under 10, it physically cannot contain an arXiv ID.
    let ascii_count = bytes.iter().filter(|b| b.is_ascii()).count();
    if ascii_count < 10 {
        return false;
    }
    // Look for: 4 digits, dot, 4-5 digits, optional "v"+digits
    // All ASCII — no CJK concerns in these patterns.
    for i in 0..bytes.len().saturating_sub(9) {
        // Check 4 preceding digits
        let mut found = true;
        for j in 0..4 {
            if i + j >= bytes.len() || !bytes[i + j].is_ascii_digit() {
                found = false;
                break;
            }
        }
        if !found || bytes[i + 4] != b'.' {
            continue;
        }
        // Count digits after dot
        let mut digit_count = 0usize;
        let mut pos = i + 5;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            digit_count += 1;
            pos += 1;
        }
        if digit_count < 4 || digit_count > 5 {
            continue;
        }
        // Optional "v" + more digits (version suffix)
        if pos < bytes.len() && bytes[pos] == b'v' {
            pos += 1;
            // Need at least one digit after v
            if pos < bytes.len() && bytes[pos].is_ascii_digit() {
                return true;
            }
            continue;
        }
        // Next char must be non-alphanumeric or end of string
        if pos >= bytes.len() || !bytes[pos].is_ascii_alphanumeric() {
            return true;
        }
    }
    false
}

/// Check if query has explicit URL signals without browser/academic context.
fn has_pure_url_context(query_lower: &str, browser_signals: usize, academic_signals: usize) -> bool {
    has_url_reference(query_lower) && browser_signals == 0 && academic_signals == 0
}

// ---------------------------------------------------------------------------
// Signal counting
// ---------------------------------------------------------------------------

/// Check if "form" appears as a standalone word (not as substring of
/// "transform", "platform", "information", etc.).
fn has_standalone_form(query_lower: &str) -> bool {
    let form = "form";
    // Fast path: if "form" isn't in the string at all, skip
    if !query_lower.contains(form) {
        return false;
    }
    // Check each occurrence with word boundaries
    let bytes = query_lower.as_bytes();
    let mut pos = 0;
    while let Some(offset) = query_lower[pos..].find(form) {
        let idx = pos + offset;
        // Check char before: must be start of string or non-alphanumeric
        let boundary_before = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
        // Check char after: must be end of string or non-alphanumeric
        let after_idx = idx + 4;
        let boundary_after = after_idx >= bytes.len() || !bytes[after_idx].is_ascii_alphanumeric();
        if boundary_before && boundary_after {
            return true;
        }
        pos = idx + 1;
    }
    false
}

/// Count all signal types in the query. Returns a `WebTaskSignalCounts` struct
/// with per-dimension hit counts.
pub fn count_all_signals(query_lower: &str) -> WebTaskSignalCounts {
    let browser_signals = count_signals(query_lower, BROWSER_SIGNALS_CN)
        + count_signals(query_lower, BROWSER_SIGNALS_EN);
    let screenshot_signals = count_signals(query_lower, SCREENSHOT_SIGNALS_CN)
        + count_signals(query_lower, SCREENSHOT_SIGNALS_EN);
    let academic_signals = count_signals(query_lower, ACADEMIC_SIGNALS_CN)
        + count_signals(query_lower, ACADEMIC_SIGNALS_EN);
    let informational_signals = count_signals(query_lower, INFORMATIONAL_SIGNALS_CN)
        + count_signals(query_lower, INFORMATIONAL_SIGNALS_EN);
    let network_signals = count_signals(query_lower, NETWORK_SIGNALS_CN)
        + count_signals(query_lower, NETWORK_SIGNALS_EN);
    let academic_deamplifiers = count_signals(query_lower, ACADEMIC_DEAMPLIFIERS_CN)
        + count_signals(query_lower, ACADEMIC_DEAMPLIFIERS_EN);

    // arXiv ID pattern counts as an academic signal
    let arxiv_signal = if has_arxiv_id(query_lower) { 1 } else { 0 };

    let mut url_signals = if has_url_reference(query_lower) { 1 } else { 0 };
    // arXiv ID counts as academic, not URL — prevent double-counting
    if has_arxiv_id(query_lower) {
        url_signals = 0;
    }

    // "form" with word boundaries (avoid false positives like "transform")
    let form_signal = if has_standalone_form(query_lower) { 1 } else { 0 };

    WebTaskSignalCounts {
        browser_signals: browser_signals + form_signal,
        screenshot_signals,
        academic_signals: academic_signals + arxiv_signal,
        informational_signals,
        network_signals,
        url_signals,
        academic_deamplifiers,
    }
}

// ---------------------------------------------------------------------------
// Tool-slug matching for a category
// ---------------------------------------------------------------------------

/// Check whether a tool slug matches a web task category.
pub fn category_matches_tool(category: WebTaskCategory, slug: &str) -> bool {
    match category {
        WebTaskCategory::SimpleFetch
        | WebTaskCategory::WebSearch => slug == "web_fetch",
        WebTaskCategory::BrowserInteractive => {
            // Must be a browser interaction tool — exclude passive ones
            slug == "browser_open"
                || slug == "browser_click"
                || slug == "browser_fill"
                || slug == "browser_press"
                || slug == "browser_get_text"
                || slug == "browser_get_state"
                || slug == "browser_get_elements"
                || slug == "browser_wait_for"
                || slug == "browser_tabs"
        }
        WebTaskCategory::BrowserScreenshot => slug == "browser_screenshot",
        WebTaskCategory::AcademicSearch => {
            slug == "search_research"
                || slug == "fetch_paper"
                || slug == "find_paper_by_title"
        }
        WebTaskCategory::LiteratureBatch => slug == "research_literature_search",
        WebTaskCategory::NetworkMonitor => slug == "browser_get_network",
        WebTaskCategory::NotWebTask => true, // Always matches — no adjustment needed
    }
}

/// Find the best-scoring candidate whose slug matches the category.
/// Returns `None` when no matching candidate has a positive score.
fn find_best_matching_tool<'a>(
    category: WebTaskCategory,
    candidates: &'a [super::types::ToolCandidate<'a>],
) -> Option<&'a super::types::ToolCandidate<'a>> {
    candidates
        .iter()
        .filter(|c| c.score > 0.0 && category_matches_tool(category, &c.record.slug))
        .max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

// ---------------------------------------------------------------------------
// Main classifier
// ---------------------------------------------------------------------------

/// Classify a user query into a web task category.
///
/// Uses vote counting across signal dimensions, with de-amplifiers to reduce
/// false positives. Returns `None` when the query shows no clear web intent.
///
/// ## Classification logic
///
/// 1. **Pure URL fetch**: has URL, no browser/academic signals → `SimpleFetch`
/// 2. **Academic dominance**: academic signals > browser + informational,
///    AND ≥ 2 academic signals → `LiteratureBatch` or `AcademicSearch`
///    (de-amplifiers reduce effective count by 50%)
/// 3. **Screenshot**: any screenshot signal → `BrowserScreenshot`
/// 4. **Browser interaction**: any browser signal → `BrowserInteractive`
/// 5. **Network monitor**: any network signal → `NetworkMonitor`
/// 6. **Informational**: any informational signal → `WebSearch`
/// 7. **URL reference**: has URL, no other signals → `SimpleFetch`
pub fn classify_web_task(query: &str) -> Option<WebTaskClassification> {
    let query_lower = query.to_lowercase();
    let counts = count_all_signals(&query_lower);

    // Rule 1: Pure URL fetch without browser or academic context
    if has_pure_url_context(&query_lower, counts.browser_signals, counts.academic_signals) {
        return Some(WebTaskClassification {
            category: WebTaskCategory::SimpleFetch,
            confidence: 0.9,
            signal_counts: counts,
            reason: "pure url fetch: web_fetch".to_string(),
        });
    }

    // Rule 2: Academic dominance — accounting for de-amplifiers
    let effective_academic = if counts.academic_deamplifiers > 0 && counts.academic_signals > 0 {
        // De-amplifiers reduce effective academic count by 50%
        counts.academic_signals.saturating_sub(counts.academic_deamplifiers)
    } else {
        counts.academic_signals
    };

    // Academic need at least ACADEMIC_MIN_SIGNALS signals, and more signals than browser + informational
    if effective_academic >= ACADEMIC_MIN_SIGNALS
        && effective_academic > (counts.browser_signals + counts.informational_signals)
    {
        let category = if counts.academic_signals >= 3 || counts.informational_signals == 0 {
            WebTaskCategory::LiteratureBatch
        } else {
            WebTaskCategory::AcademicSearch
        };
        return Some(WebTaskClassification {
            category,
            confidence: (effective_academic as f64 / 5.0).min(1.0),
            signal_counts: counts,
            reason: format!("academic {} signals", effective_academic),
        });
    }

    // Rule 3: Screenshot
    if counts.screenshot_signals > 0 {
        return Some(WebTaskClassification {
            category: WebTaskCategory::BrowserScreenshot,
            confidence: (counts.screenshot_signals as f64 / 3.0).min(1.0),
            signal_counts: counts,
            reason: "screenshot intent".to_string(),
        });
    }

    // Rule 4: Browser interaction
    if counts.browser_signals > 0 {
        return Some(WebTaskClassification {
            category: WebTaskCategory::BrowserInteractive,
            confidence: (counts.browser_signals as f64 / 5.0).min(1.0),
            signal_counts: counts,
            reason: format!("browser {} signals", counts.browser_signals),
        });
    }

    // Rule 5: Network monitor
    if counts.network_signals > 0 {
        return Some(WebTaskClassification {
            category: WebTaskCategory::NetworkMonitor,
            confidence: (counts.network_signals as f64 / 3.0).min(1.0),
            signal_counts: counts,
            reason: "network monitor".to_string(),
        });
    }

    // Rule 6: Informational search
    if counts.informational_signals > 0 {
        return Some(WebTaskClassification {
            category: WebTaskCategory::WebSearch,
            confidence: (counts.informational_signals as f64 / 5.0).min(1.0),
            signal_counts: counts,
            reason: "informational search".to_string(),
        });
    }

    // Rule 7: URL reference, no other signals
    if has_url_reference(&query_lower) {
        return Some(WebTaskClassification {
            category: WebTaskCategory::SimpleFetch,
            confidence: 0.7,
            signal_counts: counts,
            reason: "url reference".to_string(),
        });
    }

    // No web intent detected
    None
}

// ---------------------------------------------------------------------------
// Post-pick validation / adjustment
// ---------------------------------------------------------------------------

/// Default web task boost weight applied when adjusting post-pick decisions.
/// This is intentionally small (5.0) — it's a tie-breaker, not a primary
/// scoring signal. Configured via `WebTaskBoostConfig::category_boost`.
const DEFAULT_CATEGORY_BOOST: f64 = 5.0;

/// Minimum score gap below which a web-task-correcting swap is attempted.
/// When the gap between the top scorer and the web-task-matching tool exceeds
/// this threshold, the web task classification does NOT override.
const MAX_SWAP_GAP: f64 = 15.0;

/// Validate the routing decision against web task classification.
///
/// Called AFTER `route_tool_from_records()` picks the best candidate.
/// When the classified web task category conflicts with the selected tool,
/// and the gap to the nearest matching tool is small enough, applies a
/// small boost to the matching tool.
///
/// Returns the (possibly adjusted) decision, or `None` when no web task
/// classification applies.
pub fn adjust_or_validate_decision<'a>(
    current: &McpToolDecision,
    web_class: &WebTaskClassification,
    candidates: &'a [super::types::ToolCandidate<'a>],
) -> Option<McpToolDecision> {
    if web_class.category == WebTaskCategory::NotWebTask {
        return None;
    }

    // Check if the current tool is already a good match
    if category_matches_tool(web_class.category, &current.selected_tool) {
        // Already correct — no adjustment needed
        return None;
    }

    // Try to find a better tool for this category
    let better = find_best_matching_tool(web_class.category, candidates)?;

    // Only swap if the gap is small enough
    if (current.score - better.score).abs() < MAX_SWAP_GAP {
        let adjusted_score = better.score + DEFAULT_CATEGORY_BOOST;
        let mut decision = McpToolDecision {
            decision_schema_version: current.decision_schema_version.clone(),
            selected_tool: better.record.slug.clone(),
            score: adjusted_score,
            reasons: vec![
                format!("web_task:{}", web_class.category.label()),
                format!("web_task_boost:+{DEFAULT_CATEGORY_BOOST}"),
            ],
            matched_token_count: better.matched_token_count,
            dispatch_domain: better.record.dispatch_domain.to_string(),
            mcp_server: better.record.mcp_server.clone(),
            fuzzy_match: false,
        };
        decision.reasons.extend(better.reasons.clone());
        return Some(decision);
    }

    // Gap too large — don't override
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::scoring_config::tool_scoring_weights;
    use crate::routing::{route_tool_from_records, score_tool};
    use mcp_tool_registry::{DispatchDomain, ToolLayer, ToolOwner};

    // ── classify_web_task tests ──

    #[test]
    fn test_simple_fetch_url() {
        let result = classify_web_task("帮我看看这个链接 https://example.com");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::SimpleFetch);
    }

    #[test]
    fn test_browser_interactive_cn() {
        let result = classify_web_task("帮我点一下登录按钮");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::BrowserInteractive);
    }

    #[test]
    fn test_browser_interactive_en() {
        let result = classify_web_task("click the submit button");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::BrowserInteractive);
    }

    #[test]
    fn test_screenshot() {
        let result = classify_web_task("帮我截图这个页面");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::BrowserScreenshot);
    }

    #[test]
    fn test_screenshot_url() {
        let result = classify_web_task("打开网页帮我截图");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::BrowserScreenshot);
    }

    #[test]
    fn test_academic_double_hit() {
        let result = classify_web_task("查论文 Attention Is All You Need DOI");
        assert!(result.is_some());
        let cat = result.unwrap().category;
        assert!(cat == WebTaskCategory::AcademicSearch || cat == WebTaskCategory::LiteratureBatch);
    }

    #[test]
    fn test_open_arxiv_search_paper() {
        // "打开arXiv搜论文" — academic (2) > browser (1) → Academic
        let result = classify_web_task("打开arXiv搜论文");
        assert!(result.is_some(), "classify should return Some for academic+browser mixed query");
        let cat = result.unwrap().category;
        assert!(
            cat == WebTaskCategory::AcademicSearch || cat == WebTaskCategory::LiteratureBatch,
            "expected academic, got {:?}",
            cat
        );
    }

    #[test]
    fn test_open_arxiv_signals() {
        let counts = count_all_signals("打开arxiv搜论文");
        assert!(counts.browser_signals >= 1, "should detect browser signal '打开': {}", counts.browser_signals);
        assert!(counts.academic_signals >= 1, "should detect academic signal: {}", counts.academic_signals);
        assert!(counts.academic_signals >= counts.browser_signals,
            "academic signals ({}) should >= browser signals ({})",
            counts.academic_signals, counts.browser_signals);
    }

    #[test]
    fn test_navigate_arxiv_signals() {
        let counts = count_all_signals("navigate to arxiv for 论文");
        assert!(counts.browser_signals >= 1, "should detect browser signal 'navigate': {}", counts.browser_signals);
        assert!(counts.academic_signals >= 1, "should detect academic signal: {}", counts.academic_signals);
    }

    #[test]
    fn test_navigate_arxiv_classify() {
        let result = classify_web_task("navigate to arXiv for 论文");
        assert!(result.is_some(), "classify should return Some for navigate+academic");
        let classification = result.unwrap();
        assert!(
            classification.category == WebTaskCategory::AcademicSearch
                || classification.category == WebTaskCategory::LiteratureBatch,
            "expected academic category for navigate+academic query, got {:?} (browser={}, academic={})",
            classification.category,
            classification.signal_counts.browser_signals,
            classification.signal_counts.academic_signals,
        );
    }

    #[test]
    fn test_non_web_query() {
        let result = classify_web_task("写一篇关于注意力的论文");
        // De-amplifier "写" neutralizes academic signal "论文" → not a web task
        assert!(
            result.is_none(),
            "non-web query should return None, got {:?}",
            result.map(|c| c.category)
        );
    }

    #[test]
    fn test_arxiv_id() {
        let result = classify_web_task("2301.07041是什么论文");
        assert!(result.is_some());
        // arxiv ID should count as academic, not URL
        let cat = result.unwrap().category;
        assert_eq!(cat, WebTaskCategory::LiteratureBatch);
    }

    #[test]
    fn test_english_fetch() {
        let result = classify_web_task("fetch this URL for me https://example.com");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::SimpleFetch);
    }

    #[test]
    fn test_mixed_en_cn_academic() {
        let result = classify_web_task("navigate to arXiv for 论文");
        assert!(result.is_some());
        let cat = result.unwrap().category;
        assert!(
            cat == WebTaskCategory::AcademicSearch || cat == WebTaskCategory::LiteratureBatch,
            "expected academic for mixed query, got {:?}",
            cat
        );
    }

    #[test]
    fn test_academic_deamplifier() {
        let result = classify_web_task("保存这篇论文到本地");
        // "保存" de-amplifies "论文" → effective academic becomes 0
        assert!(result.is_none() || result.unwrap().category != WebTaskCategory::AcademicSearch);
    }

    #[test]
    fn test_url_no_protocol() {
        let result = classify_web_task("看看 github.com 这个网站");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::SimpleFetch);
    }

    #[test]
    fn test_network_monitor() {
        let result = classify_web_task("帮我看看网络请求");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::NetworkMonitor);
    }

    #[test]
    fn test_informational_search() {
        let result = classify_web_task("查一下LLM框架对比");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::WebSearch);
    }

    #[test]
    fn test_empty_query() {
        let result = classify_web_task("");
        assert!(result.is_none());
    }

    // ── category_matches_tool tests ──

    #[test]
    fn test_category_matches_web_fetch() {
        assert!(category_matches_tool(WebTaskCategory::SimpleFetch, "web_fetch"));
        assert!(!category_matches_tool(WebTaskCategory::SimpleFetch, "browser_open"));
    }

    #[test]
    fn test_category_matches_browser() {
        assert!(category_matches_tool(WebTaskCategory::BrowserInteractive, "browser_open"));
        assert!(category_matches_tool(WebTaskCategory::BrowserInteractive, "browser_click"));
        assert!(category_matches_tool(WebTaskCategory::BrowserInteractive, "browser_fill"));
        // But NOT screenshot
        assert!(!category_matches_tool(WebTaskCategory::BrowserInteractive, "browser_screenshot"));
    }

    #[test]
    fn test_category_matches_academic() {
        assert!(category_matches_tool(WebTaskCategory::AcademicSearch, "search_research"));
        assert!(category_matches_tool(WebTaskCategory::AcademicSearch, "fetch_paper"));
        assert!(!category_matches_tool(WebTaskCategory::AcademicSearch, "web_fetch"));
    }

    // ── signal count tests ──

    #[test]
    fn test_signal_counts_browser() {
        let counts = count_all_signals("打开这个页面然后点击提交按钮");
        assert!(counts.browser_signals >= 2, "browser signals: {}", counts.browser_signals);
    }

    #[test]
    fn test_signal_counts_screenshot() {
        let counts = count_all_signals("截图保存这个页面");
        assert!(counts.screenshot_signals >= 1);
    }

    #[test]
    fn test_signal_counts_academic() {
        let counts = count_all_signals("查论文 attention is all you need doi");
        assert!(counts.academic_signals >= 2, "should detect 2+ academic signals, got {}", counts.academic_signals);
    }

    #[test]
    fn test_signal_counts_deamplifier() {
        let counts = count_all_signals("保存这篇论文");
        assert!(counts.academic_signals >= 1);
        assert!(counts.academic_deamplifiers >= 1);
    }

    // ── Integration tests with route_tool_from_records ──

    #[test]
    fn test_route_web_fetch() {
        // Verify web_task classification handles URL fetch
        let result = classify_web_task("帮我看看这个链接 https://example.com");
        assert!(result.is_some(), "should classify URL fetch");
        assert_eq!(result.unwrap().category, WebTaskCategory::SimpleFetch);
    }

    #[test]
    fn test_route_browser_click() {
        // Verify web_task classification handles browser interaction
        let result = classify_web_task("帮我点一下登录按钮");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::BrowserInteractive);
    }

    #[test]
    fn test_route_screenshot() {
        let result = classify_web_task("帮我截图这个页面");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::BrowserScreenshot);
    }

    #[test]
    fn test_route_academic() {
        let result = classify_web_task("帮我查论文 Attention Is All You Need DOI");
        assert!(result.is_some(), "academic query (论文+DOI) should classify");
        let cat = result.unwrap().category;
        assert!(
            cat == WebTaskCategory::AcademicSearch || cat == WebTaskCategory::LiteratureBatch,
            "academic query should classify as academic, got {:?}",
            cat
        );
    }

    #[test]
    fn test_route_network_monitor() {
        let result = classify_web_task("帮我看看网络请求");
        assert!(result.is_some());
        assert_eq!(result.unwrap().category, WebTaskCategory::NetworkMonitor);
    }

    #[test]
    fn test_route_non_web_query() {
        let result = classify_web_task("写一篇关于注意力的论文");
        // De-amplifier on "写" prevents academic classification
        if let Some(c) = result {
            assert!(
                c.category != WebTaskCategory::AcademicSearch,
                "non-web query should not classify as academic, got {:?}",
                c.category
            );
        }
    }
}
