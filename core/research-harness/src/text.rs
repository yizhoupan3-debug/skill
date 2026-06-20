//! Text processing utilities: slugification, XML parsing, Markdown helpers.
//!
//! Migrated from `tools/autoresearch-rs/src/text.rs`.

use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

/// Convert text to a filesystem-safe slug (lowercase, alphanumeric + hyphens).
pub fn slugify(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    static NON_ALNUM: OnceLock<Regex> = OnceLock::new();
    let cleaned = NON_ALNUM
        .get_or_init(|| Regex::new(r"[^a-z0-9]+").unwrap())
        .replace_all(&lowered, "-")
        .to_string();
    static MULTI_DASH: OnceLock<Regex> = OnceLock::new();
    let collapsed = MULTI_DASH
        .get_or_init(|| Regex::new(r"-+").unwrap())
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

/// Extract and decode the text content between XML tags.
pub fn xml_text_between(raw: &str, tag: &str) -> Option<String> {
    let pattern = Regex::new(&format!(r"(?s)<{tag}(?:\s[^>]*)?>(.*?)</{tag}>")).ok()?;
    let captures = pattern.captures(raw)?;
    Some(decode_xml_entities(captures.get(1)?.as_str().trim()))
}

/// English + CJK stopword set for content word extraction.
pub fn stopwords() -> HashSet<&'static str> {
    [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "in", "into", "is",
        "it", "of", "on", "or", "reduce", "research", "that", "the", "this", "to", "use", "using",
        "with",
    ]
    .into_iter()
    .collect()
}

/// Extract meaningful content words (deduped, stopword-filtered, length ≥ 3 for ASCII).
pub fn compact_words(text: &str, limit: usize) -> Vec<String> {
    static WORD_RE: OnceLock<Regex> = OnceLock::new();
    let re = WORD_RE.get_or_init(|| Regex::new(r"[A-Za-z0-9][A-Za-z0-9_-]*").unwrap());
    let stops = stopwords();
    let mut filtered = Vec::new();
    for cap in re.find_iter(&text.to_lowercase()) {
        let word = cap.as_str();
        if word.len() <= 2 || stops.contains(word) {
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
