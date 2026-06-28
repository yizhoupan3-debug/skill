//! Prose QC — 术语一致性、风格合规、AI slop 检测。
//!
//! 与 prose-chain-contract.md 和 research-language-norms.md 对齐。

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 检查文本中的术语使用是否与术语表一致。
/// 返回不一致的术语列表（找到的非标准用法）。
pub fn check_terminology_consistency(
    text: &str,
    glossary: &HashMap<String, String>,
) -> Result<Vec<String>> {
    let mut violations = Vec::new();
    let lower = text.to_ascii_lowercase();

    for (variant, canonical) in glossary {
        let variant_lower = variant.to_ascii_lowercase();
        let canonical_lower = canonical.to_ascii_lowercase();

        // 如果在文中找到了非标准变体（且不是标准用法本身）
        if variant_lower != canonical_lower && lower.contains(&variant_lower) {
            violations.push(format!(
                "术语不一致：使用了 '{}' 而非标准 '{}'",
                variant, canonical
            ));
        }
    }

    Ok(violations)
}

/// Check if a substring occurrence is at a word boundary in the text.
fn is_whole_word(haystack: &str, start: usize, needle_len: usize) -> bool {
    let before_ok = start == 0
        || !haystack[..start]
            .chars()
            .last()
            .map_or(false, |c| c.is_alphanumeric());
    let after_pos = start + needle_len;
    let after_ok = after_pos >= haystack.len()
        || !haystack[after_pos..]
            .chars()
            .next()
            .map_or(false, |c| c.is_alphanumeric());
    before_ok && after_ok
}

/// 检测英文 AI slop 词汇（AI 高频非必要词汇）。
/// 返回找到的 slop 词汇及其位置。
pub fn detect_en_slop(text: &str) -> Vec<SlopHit> {
    let slop_words = [
        ("Moreover", "Additionally / 直接删除"),
        ("Furthermore", "What's more / Beyond this"),
        ("Delve", "explore / investigate"),
        ("Tapestry", "删除，替换为具体描述"),
        ("Landscape", "field / area"),
        ("It is worth noting that", "直接陈述"),
        ("It should be noted that", "直接陈述"),
        ("In the realm of", "in / within"),
        ("A testament to", "demonstrates / shows"),
        ("Pivotal", "important / key / critical"),
        ("Robust", "strong / effective / reliable"),
        ("Groundbreaking", "novel / first / new"),
        ("Cutting-edge", "state-of-the-art / recent"),
        ("Leverage", "use / apply"),
        ("Harness", "use / employ / utilize"),
        ("Paradigm", "approach / framework / method"),
    ];

    let mut hits = Vec::new();
    for (slop, replacement) in &slop_words {
        let lower = text.to_ascii_lowercase();
        let slop_lower = slop.to_ascii_lowercase();
        let mut start = 0;
        while let Some(pos) = lower[start..].find(&slop_lower) {
            let actual_pos = start + pos;
            if is_whole_word(&lower, actual_pos, slop_lower.len()) {
                hits.push(SlopHit {
                    word: slop.to_string(),
                    replacement: replacement.to_string(),
                    position: actual_pos,
                });
            }
            start = actual_pos + slop_lower.len();
        }
    }

    hits
}

/// 检测中文套话。
pub fn detect_zh_slop(text: &str) -> Vec<SlopHit> {
    let slop_phrases = [
        ("值得注意的是", "直接陈述"),
        ("众所周知", "删除或给出具体引用"),
        ("不言而喻", "删除"),
        ("毋庸置疑", "删除或给出证据"),
        ("具有重要意义", "说明具体意义"),
        ("取得了显著成效", "给出具体数据"),
        ("在一定程度上", "明确程度或删除"),
        ("相关研究表明", "给出具体引用"),
    ];

    let mut hits = Vec::new();
    for (slop, replacement) in &slop_phrases {
        let mut start = 0;
        while let Some(pos) = text[start..].find(slop) {
            let actual_pos = start + pos;
            hits.push(SlopHit {
                word: slop.to_string(),
                replacement: replacement.to_string(),
                position: actual_pos,
            });
            start = actual_pos + slop.len();
        }
    }

    hits
}

/// A slop word/phrase hit with its position and suggested replacement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlopHit {
    pub word: String,
    pub replacement: String,
    pub position: usize,
}

/// 检查防御性语气（过多 hedging）。
/// 返回 hedging 词汇计数。
pub fn count_hedging_words(text: &str) -> usize {
    let hedging_words = [
        "may",
        "might",
        "could",
        "possibly",
        "potentially",
        "it seems",
        "appears to",
        "somewhat",
        "rather",
        "quite",
        "to some extent",
        "arguably",
    ];
    let lower = text.to_ascii_lowercase();
    hedging_words
        .iter()
        .map(|w| {
            let mut count = 0;
            let mut start = 0;
            while let Some(pos) = lower[start..].find(w) {
                let abs_pos = start + pos;
                if is_whole_word(&lower, abs_pos, w.len()) {
                    count += 1;
                }
                start = abs_pos + w.len();
            }
            count
        })
        .sum()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_en_slop_detection() {
        let text = "Moreover, we delve into the robust tapestry of cutting-edge methods.";
        let hits = detect_en_slop(text);
        assert!(hits.len() >= 3); // Moreover, delve, robust, tapestry, cutting-edge
    }

    #[test]
    fn test_zh_slop_detection() {
        let text = "值得注意的是，本方法具有重要意义。众所周知，该领域已取得显著成效。";
        let hits = detect_zh_slop(text);
        assert!(hits.len() >= 3);
    }

    #[test]
    fn test_terminology_consistency() {
        let mut glossary = HashMap::new();
        glossary.insert("neural network".to_string(), "neural net".to_string());
        let violations =
            check_terminology_consistency("We use a neural network for classification.", &glossary)
                .unwrap();
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_hedging_count() {
        let text = "It may possibly be the case that we could potentially achieve results.";
        let count = count_hedging_words(text);
        assert!(count >= 3);
    }

    #[test]
    fn word_boundary_prevents_false_positive() {
        // "may" should NOT match inside "mayhem"
        let may_contributions = count_hedging_words("may");
        let mayhem_count = count_hedging_words("mayhem");
        assert!(
            mayhem_count < may_contributions,
            "mayhem should not count as hedging"
        );
        // "quite" should NOT match inside "quiteinteresting" (nonsense word but demonstrates)
        assert_eq!(count_hedging_words("quiteinteresting"), 0);
    }

    #[test]
    fn word_boundary_still_detects_standalone() {
        assert_eq!(count_hedging_words("It may be true."), 1);
        assert_eq!(count_hedging_words("This could work."), 1);
        assert_eq!(count_hedging_words("It seems plausible."), 1);
    }

    #[test]
    fn en_slop_word_boundary() {
        // "harness" in "research-harness" — hyphen is a boundary, so it still matches
        let hits = detect_en_slop("the research-harness system");
        let harness_hits: Vec<_> = hits
            .iter()
            .filter(|h| h.word.to_ascii_lowercase() == "harness")
            .collect();
        assert!(
            !harness_hits.is_empty(),
            "research-harness should still match 'harness' slop"
        );

        // "robust" should NOT match inside "robustness"
        let hits2 = detect_en_slop("the robustness of the method");
        let robust_hits: Vec<_> = hits2
            .iter()
            .filter(|h| h.word.to_ascii_lowercase() == "robust")
            .collect();
        assert!(
            robust_hits.is_empty(),
            "robustness should not match 'robust' slop"
        );
    }

    #[test]
    fn is_whole_word_utility() {
        assert!(is_whole_word("hello world", 0, 5)); // "hello" at start
        assert!(is_whole_word("hello world", 6, 5)); // "world" at end
        assert!(is_whole_word("say hello world", 4, 5)); // "hello" in middle
        assert!(!is_whole_word("mayhem", 0, 3)); // "may" inside "mayhem"
        assert!(!is_whole_word("boot", 1, 2)); // "oo" is not whole
    }
}
