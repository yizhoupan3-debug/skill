//! Text processing utilities: slugification, XML parsing, Markdown helpers.
//!
//! Migrated from `tools/autoresearch-rs/src/text.rs`.

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, OnceLock};

// ── Pre-compiled regexes for common arXiv XML tags ──
// These avoid Mutex contention on the dynamic cache for the hot-path tags
// used during arXiv Atom XML parsing.

static ARXIV_TAG_TITLE: OnceLock<Regex> = OnceLock::new();
static ARXIV_TAG_SUMMARY: OnceLock<Regex> = OnceLock::new();
static ARXIV_TAG_ID: OnceLock<Regex> = OnceLock::new();
static ARXIV_TAG_PUBLISHED: OnceLock<Regex> = OnceLock::new();

fn get_static_xml_tag_re(tag: &str) -> Option<&'static Regex> {
    match tag {
        "title" => Some(ARXIV_TAG_TITLE.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r"(?s)<title(?:\s[^>]*)?>(.*?)</title>").expect("title regex")
        })),
        "summary" => Some(ARXIV_TAG_SUMMARY.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r"(?s)<summary(?:\s[^>]*)?>(.*?)</summary>").expect("summary regex")
        })),
        "id" => Some(ARXIV_TAG_ID.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r"(?s)<id(?:\s[^>]*)?>(.*?)</id>").expect("id regex")
        })),
        "published" => Some(ARXIV_TAG_PUBLISHED.get_or_init(|| {
            #[allow(clippy::expect_used)]
            Regex::new(r"(?s)<published(?:\s[^>]*)?>(.*?)</published>").expect("published regex")
        })),
        _ => None,
    }
}

// ── Slug ──
pub fn slugify(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    static NON_ALNUM: OnceLock<Regex> = OnceLock::new();
    let cleaned = NON_ALNUM
        .get_or_init(|| {
            #[allow(clippy::unwrap_used, clippy::expect_used)]
            Regex::new(r"[^a-z0-9]+").unwrap()
        })
        .replace_all(&lowered, "-")
        .to_string();
    static MULTI_DASH: OnceLock<Regex> = OnceLock::new();
    let collapsed = MULTI_DASH
        .get_or_init(|| {
            #[allow(clippy::unwrap_used, clippy::expect_used)]
            Regex::new(r"-+").unwrap()
        })
        .replace_all(&cleaned, "-")
        .trim_matches('-')
        .to_string();
    if collapsed.is_empty() {
        "hypothesis".to_string()
    } else {
        collapsed
    }
}

/// Render an optional URL as a Markdown link, or `"-"` if absent/empty.
pub fn markdown_link(value: Option<&str>) -> String {
    value
        .filter(|item| !item.trim().is_empty())
        .map(|item| format!("[link]({})", item.trim()))
        .unwrap_or_else(|| "-".into())
}

/// Decode XML entities (`&lt;`, `&gt;`, `&amp;`, `&quot;`, `&apos;`) and normalize whitespace.
pub fn decode_xml_entities(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract and decode the text content between XML tags, with regex caching.
///
/// Performance: common arXiv tags ("title", "summary", "id", "published") use
/// pre-compiled static regexes and bypass the Mutex cache entirely.
/// Dynamic cache is bounded at MAX_XML_TAG_CACHE_ENTRIES for custom tags.
pub fn xml_text_between(raw: &str, tag: &str) -> Option<String> {
    // Fast path: pre-compiled regex for common arXiv XML tags — no Mutex needed.
    if let Some(re) = get_static_xml_tag_re(tag) {
        let captures = re.captures(raw)?;
        return Some(decode_xml_entities(captures.get(1)?.as_str().trim()));
    }

    // Fallback: dynamic Mutex-guarded cache for rare/custom tags.
    const MAX_XML_TAG_CACHE_ENTRIES: usize = 64;
    static XML_TAG_RE_CACHE: LazyLock<Mutex<HashMap<String, Regex>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let re = match XML_TAG_RE_CACHE
        .lock()
        .ok()
        .and_then(|c| c.get(tag).cloned())
    {
        Some(r) => r,
        None => {
            let pattern = Regex::new(&format!(r"(?s)<{tag}(?:\s[^>]*)?>(.*?)</{tag}>")).ok()?;
            if let Ok(mut cache) = XML_TAG_RE_CACHE.lock() {
                // Evict entries when over capacity: drain oldest half to
                // preserve recent entries while preventing unbounded growth.
                if cache.len() >= MAX_XML_TAG_CACHE_ENTRIES {
                    let target = MAX_XML_TAG_CACHE_ENTRIES / 2;
                    let excess = cache.len() - target;
                    let mut count = 0;
                    cache.retain(|_, _| {
                        count += 1;
                        count > excess // keep entries past the first `excess`
                    });
                }
                cache.insert(tag.to_string(), pattern.clone());
            }
            pattern
        }
    };
    let captures = re.captures(raw)?;
    Some(decode_xml_entities(captures.get(1)?.as_str().trim()))
}

/// English + CJK stopword set (cached via LazyLock).
pub fn stopwords() -> &'static HashSet<&'static str> {
    static STOPWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        [
            "a",
            "an",
            "and",
            "are",
            "as",
            "at",
            "be",
            "by",
            "can",
            "for",
            "from",
            "in",
            "into",
            "is",
            "it",
            "of",
            "on",
            "or",
            "reduce",
            "research",
            "that",
            "the",
            "this",
            "to",
            "use",
            "using",
            "with",
            "的",
            "了",
            "在",
            "是",
            "我",
            "有",
            "和",
            "就",
            "不",
            "人",
            "都",
            "一",
            "一个",
            "上",
            "也",
            "很",
            "到",
            "说",
            "要",
            "去",
            "你",
            "会",
            "着",
            "没有",
            "看",
            "好",
            "自己",
            "这",
            "他",
            "她",
            "它",
            "们",
            "那",
            "被",
            "从",
            "把",
            "对",
            "但",
            "而",
            "与",
            "或",
            "中",
            "等",
            "能",
            "可以",
            "什么",
            "怎么",
            "如何",
            "为什么",
            "是否",
            "通过",
            "使用",
            "基于",
            "以及",
            "然后",
            "因此",
            "所以",
            "如果",
            "虽然",
            "但是",
            "对于",
            "关于",
            "已经",
            "正在",
        ]
        .into_iter()
        .collect()
    });
    &STOPWORDS
}

/// Extract meaningful words from text, supporting both ASCII and CJK characters.
pub fn compact_words(text: &str, limit: usize) -> Vec<String> {
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

/// Extract meaningful content words as a set (≥3 chars, lowercased, stopword-filtered).
///
/// Uses split-based tokenization (non-alphanumeric delimiters except `_`).
/// Contains an expanded stopword list covering English and CJK.
pub fn extract_content_words(text: &str) -> HashSet<String> {
    let stopwords: HashSet<&str> = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can",
        "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by", "from",
        "as", "into", "through", "during", "before", "after", "above", "below", "between", "out",
        "off", "over", "under", "again", "further", "then", "once", "and", "but", "or", "nor",
        "not", "so", "yet", "both", "either", "neither", "each", "every", "all", "any", "few",
        "more", "most", "other", "some", "such", "no", "only", "own", "same", "than", "too",
        "very", "just", "that", "this", "these", "those", "it", "its", "we", "our", "they",
        "their", // Chinese stopwords
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上", "也",
        "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好", "自己", "这",
    ]
    .iter()
    .copied()
    .collect();

    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|w| w.to_ascii_lowercase())
        .filter(|w| w.len() >= 3 && !stopwords.contains(w.as_str()))
        .collect()
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  Trim  Spaces  "), "trim-spaces");
    }

    #[test]
    fn slugify_empty_fallback() {
        assert_eq!(slugify(""), "hypothesis");
        assert_eq!(slugify("---"), "hypothesis");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("foo@bar!baz"), "foo-bar-baz");
    }

    #[test]
    fn markdown_link_some() {
        assert_eq!(
            markdown_link(Some("https://example.com")),
            "[link](https://example.com)"
        );
    }

    #[test]
    fn markdown_link_none() {
        assert_eq!(markdown_link(None), "-");
    }

    #[test]
    fn markdown_link_empty() {
        assert_eq!(markdown_link(Some("  ")), "-");
    }

    #[test]
    fn decode_xml_entities_basic() {
        assert_eq!(decode_xml_entities("&lt;div&gt;"), "<div>");
        assert_eq!(decode_xml_entities("a &amp; b"), "a & b");
        assert_eq!(decode_xml_entities("&quot;hello&quot;"), "\"hello\"");
    }

    #[test]
    fn decode_xml_entities_normalizes_whitespace() {
        assert_eq!(decode_xml_entities("  hello   world  "), "hello world");
    }

    #[test]
    fn xml_text_between_extracts_content() {
        let raw = "<title>My Paper</title>";
        assert_eq!(xml_text_between(raw, "title"), Some("My Paper".into()));
    }

    #[test]
    fn xml_text_between_not_found() {
        assert_eq!(xml_text_between("<foo>bar</foo>", "title"), None);
    }

    #[test]
    fn compact_words_filters_stopwords() {
        let words = compact_words("the quick brown fox jumps over the lazy dog", 10);
        assert!(!words.contains(&"the".to_string()));
        assert!(words.contains(&"quick".to_string()));
        assert!(words.contains(&"brown".to_string()));
    }

    #[test]
    fn compact_words_respects_limit() {
        let words = compact_words("alpha beta gamma delta epsilon zeta", 3);
        assert_eq!(words.len(), 3);
    }

    #[test]
    fn compact_words_deduplicates() {
        let words = compact_words("test test test foo", 10);
        assert_eq!(words.iter().filter(|w| *w == "test").count(), 1);
    }

    #[test]
    fn stopwords_nonempty() {
        let sw = stopwords();
        assert!(!sw.is_empty());
        assert!(sw.contains("the"));
        assert!(sw.contains("is"));
    }
}
