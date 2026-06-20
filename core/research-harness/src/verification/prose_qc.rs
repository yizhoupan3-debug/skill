//! Prose QC — 术语一致性、风格合规、AI slop 检测。
//!
//! 与 prose-chain-contract.md 和 research-language-norms.md 对齐。

use std::collections::HashMap;
use anyhow::Result;

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
#[derive(Debug, Clone)]
pub struct SlopHit {
    pub word: String,
    pub replacement: String,
    pub position: usize,
}

/// 检查防御性语气（过多 hedging）。
/// 返回 hedging 词汇计数。
pub fn count_hedging_words(text: &str) -> usize {
    let hedging_words = [
        "may", "might", "could", "possibly", "potentially",
        "it seems", "appears to", "somewhat", "rather", "quite",
        "to some extent", "arguably",
    ];
    let lower = text.to_ascii_lowercase();
    hedging_words.iter().map(|w| lower.matches(w).count()).sum()
}

#[cfg(test)]
mod tests {
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
        let violations = check_terminology_consistency(
            "We use a neural network for classification.",
            &glossary,
        ).unwrap();
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_hedging_count() {
        let text = "It may possibly be the case that we could potentially achieve results.";
        let count = count_hedging_words(text);
        assert!(count >= 3);
    }
}
