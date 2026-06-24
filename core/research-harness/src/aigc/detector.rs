//! AIGC detection engine — n-gram anomaly, burstiness, syntactic pattern analysis.

use std::collections::HashMap;

use anyhow::Result;

use crate::aigc::Language;
use crate::types::{AigcDetectionResult, AigcSignal, AigcSignalType};

// ── Config ──

/// AIGC detection configuration parameters.
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// n-gram 重复率阈值。
    pub threshold_ngram: f64,
    /// burstiness（突发度）阈值。
    pub threshold_burstiness: f64,
    /// 句法模式异常阈值。
    pub threshold_pattern: f64,
    /// Detection language.
    pub language: Language,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            threshold_ngram: 0.7,
            threshold_burstiness: 0.3,
            threshold_pattern: 0.5,
            language: Language::English,
        }
    }
}

// ── Public API ──

/// Execute multi-dimensional AIGC detection on text, returning per-paragraph results.
///
/// Text is split into paragraphs (double newline or blank line). Each paragraph is
/// independently scored through three signal channels:
/// 1. N-gram frequency concentration
/// 2. Sentence-length burstiness (coefficient of variation)
/// 3. AI-style syntactic patterns
pub fn detect(text: &str, config: &DetectionConfig) -> Result<Vec<AigcDetectionResult>> {
    let paragraphs = split_paragraphs(text);
    if paragraphs.is_empty() {
        return Ok(vec![]);
    }

    let mut results = Vec::with_capacity(paragraphs.len());

    for (idx, para) in paragraphs.iter().enumerate() {
        let segment_id = format!("seg-{idx}");
        let para_trimmed = para.trim();
        if para_trimmed.is_empty() {
            continue;
        }

        let mut signals = Vec::new();

        // 1. n-gram anomaly
        let ngram_score = detect_ngram_anomaly(para_trimmed, config);
        signals.push(AigcSignal {
            signal_type: AigcSignalType::NGramAnomaly,
            value: ngram_score,
            detail: format!("n-gram concentration score: {ngram_score:.3}"),
        });

        // 2. burstiness (sentence-length coefficient of variation)
        let burst_score = detect_burstiness(para_trimmed, config);
        signals.push(AigcSignal {
            signal_type: AigcSignalType::LowBurstiness,
            value: burst_score,
            detail: format!("burstiness (low-CV → AI): {burst_score:.3}"),
        });

        // 3. syntactic patterns
        let (pattern_score, matched) = detect_syntactic_patterns(para_trimmed, config);
        signals.push(AigcSignal {
            signal_type: AigcSignalType::SyntacticPattern,
            value: pattern_score,
            detail: format!("syntactic patterns matched: {}", matched.join(", ")),
        });

        // Vocabulary repetition (bonus signal)
        let vocab_rep = detect_vocabulary_repetition(para_trimmed);
        signals.push(AigcSignal {
            signal_type: AigcSignalType::VocabularyRepetition,
            value: vocab_rep,
            detail: format!("vocabulary repetition score: {vocab_rep:.3}"),
        });

        // Weighted ai_probability
        let ai_probability =
            ngram_score * 0.3 + burst_score * 0.4 + pattern_score * 0.3;
        let ai_probability = ai_probability.clamp(0.0, 1.0);

        results.push(AigcDetectionResult {
            segment_id,
            ai_probability,
            score: (ai_probability * 100.0).round() as u32,
            signals,
        });
    }

    Ok(results)
}

// ── Signal 1: N-gram anomaly ──

/// Compute n-gram frequency concentration (0.0 = natural, 1.0 = highly AI-like).
///
/// Splits text into unigrams, bigrams, and trigrams; measures how concentrated the
/// distribution is. High concentration (top-5 n-grams account for a large share)
/// indicates repetitive / AI-generated prose.
fn detect_ngram_anomaly(text: &str, config: &DetectionConfig) -> f64 {
    let tokens = tokenize(text, config.language);
    if tokens.len() < 3 {
        return 0.0;
    }

    let mut all_ngrams: HashMap<String, u32> = HashMap::new();
    let mut total = 0u32;

    // Unigrams
    for t in &tokens {
        *all_ngrams.entry(t.clone()).or_insert(0) += 1;
        total += 1;
    }
    // Bigrams
    for w in tokens.windows(2) {
        let key = format!("{} {}", w[0], w[1]);
        *all_ngrams.entry(key).or_insert(0) += 1;
        total += 1;
    }
    // Trigrams
    for w in tokens.windows(3) {
        let key = format!("{} {} {}", w[0], w[1], w[2]);
        *all_ngrams.entry(key).or_insert(0) += 1;
        total += 1;
    }

    if total == 0 {
        return 0.0;
    }

    // Sort by frequency descending
    let mut freqs: Vec<u32> = all_ngrams.values().copied().collect();
    freqs.sort_unstable_by(|a, b| b.cmp(a));

    // Top-5 n-grams concentration ratio
    let top_sum: u32 = freqs.iter().take(5).sum();
    let concentration = top_sum as f64 / total as f64;

    // Normalize: concentration in [0.0, 1.0]. Typical natural text: ~0.05-0.20.
    // AI text tends to have higher concentration.
    // Map [threshold_ngram, 1.0] → [0.0, 1.0]
    let threshold = config.threshold_ngram;
    if concentration <= 0.0 {
        0.0
    } else if concentration >= threshold {
        1.0
    } else {
        concentration / threshold
    }
}

// ── Signal 2: Burstiness ──

/// Compute burstiness score (0.0 = uniform/AI, 1.0 = varied/human).
///
/// Returns the AI-probability: low coefficient of variation → high AI probability.
fn detect_burstiness(text: &str, config: &DetectionConfig) -> f64 {
    let sentences = split_sentences(text, config.language);
    if sentences.len() < 2 {
        return 0.5; // insufficient data, neutral
    }

    let lengths: Vec<f64> = sentences
        .iter()
        .map(|s| s.split_whitespace().count() as f64)
        .collect();

    let n = lengths.len() as f64;
    let mean = lengths.iter().sum::<f64>() / n;

    if mean < 1.0 {
        return 0.5;
    }

    let variance = lengths.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();
    let cv = std_dev / mean; // coefficient of variation

    // Low CV = AI-like (uniform sentence lengths).
    // CV typically: human 0.4-0.8, AI 0.1-0.3.
    // Map CV below threshold to high AI probability.
    let threshold = config.threshold_burstiness;
    if cv >= threshold * 2.0 {
        0.0 // clearly human-level variation
    } else if cv <= threshold * 0.3 {
        1.0 // extremely uniform
    } else {
        // Linear interpolation: cv in [threshold*0.3, threshold*2] → [1.0, 0.0]
        1.0 - (cv - threshold * 0.3) / (threshold * 2.0 - threshold * 0.3)
    }
}

// ── Signal 3: Syntactic patterns ──

/// Detect AI-common syntactic patterns.
///
/// Returns (score, list of matched pattern descriptions).
fn detect_syntactic_patterns(text: &str, config: &DetectionConfig) -> (f64, Vec<String>) {
    let mut matched = Vec::new();
    let text_lower = text.to_lowercase();

    let patterns = match config.language {
        Language::English => english_ai_patterns(),
        Language::Chinese => chinese_ai_patterns(),
    };

    let sentence_count = split_sentences(text, config.language).len().max(1) as f64;

    for (pattern, description) in &patterns {
        if text_lower.contains(pattern) {
            matched.push(description.clone());
        }
    }

    // Passive voice heuristic (English only)
    if config.language == Language::English {
        let passive_count = count_passive_voice(&text_lower);
        if passive_count as f64 / sentence_count > 0.4 {
            matched.push("excessive passive voice".to_string());
        }
    }

    // Paragraph uniformity
    let paragraphs: Vec<&str> = text.split("\n\n").filter(|p| !p.trim().is_empty()).collect();
    if paragraphs.len() >= 3 {
        let lens: Vec<f64> = paragraphs
            .iter()
            .map(|p| p.split_whitespace().count() as f64)
            .collect();
        let mean = lens.iter().sum::<f64>() / lens.len() as f64;
        if mean > 0.0 {
            let variance = lens.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / lens.len() as f64;
            let cv = variance.sqrt() / mean;
            if cv < 0.15 {
                matched.push("uniform paragraph lengths".to_string());
            }
        }
    }

    let pattern_count = matched.len() as f64;
    let score = (pattern_count / 5.0).clamp(0.0, 1.0);
    (score, matched)
}

// ── Bonus signal: vocabulary repetition ──

/// Score how repetitive the vocabulary is (0.0 = varied, 1.0 = repetitive).
fn detect_vocabulary_repetition(text: &str) -> f64 {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 2)
        .collect();

    if words.is_empty() {
        return 0.0;
    }

    let unique: std::collections::HashSet<&str> =
        words.iter().map(|w| w.as_str()).collect();

    let type_token_ratio = unique.len() as f64 / words.len() as f64;
    // Low TTR = repetitive. Natural: 0.5-0.7, AI: 0.3-0.5.
    // Invert: low TTR → high score.
    (1.0 - type_token_ratio).clamp(0.0, 1.0)
}

// ── Helpers ──

/// Split text into paragraphs (double-newline separated).
fn split_paragraphs(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .chain(text.split("\n\r\n\r"))
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

/// Tokenize text into lowercase word tokens.
fn tokenize(text: &str, language: Language) -> Vec<String> {
    match language {
        Language::English => text
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .map(|w| w.to_lowercase())
            .filter(|w| !w.is_empty())
            .collect(),
        Language::Chinese => {
            // Simple character-level tokenization for Chinese.
            // Each CJK character is one token; ASCII words kept as-is.
            let mut tokens = Vec::new();
            let mut current_word = String::new();
            for ch in text.chars() {
                if ch.is_ascii_alphanumeric() || ch == '\'' {
                    current_word.push(ch);
                } else {
                    if !current_word.is_empty() {
                        tokens.push(current_word.to_lowercase());
                        current_word.clear();
                    }
                    if is_cjk(ch) {
                        tokens.push(ch.to_string());
                    }
                }
            }
            if !current_word.is_empty() {
                tokens.push(current_word.to_lowercase());
            }
            tokens
        }
    }
}

/// Check if a character is in the CJK Unified Ideographs range.
/// Uses the shared implementation from core-state-utils (covers Extension A-G + Hangul + Kana).
fn is_cjk(ch: char) -> bool {
    core_state_utils::text_utils::is_cjk(ch)
}

/// Split text into sentences.
fn split_sentences(text: &str, language: Language) -> Vec<String> {
    match language {
        Language::English => {
            // Split on sentence-ending punctuation.
            text.split(['.', '!', '?'])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
        Language::Chinese => {
            text.split(['。', '！', '？', '.', '!', '?'])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }
    }
}

/// English AI-typical phrase patterns.
fn english_ai_patterns() -> Vec<(&'static str, String)> {
    vec![
        ("moreover,", "AI phrase: Moreover".into()),
        ("furthermore,", "AI phrase: Furthermore".into()),
        ("in conclusion,", "AI phrase: In conclusion".into()),
        ("it is worth noting that", "AI phrase: It is worth noting that".into()),
        ("it is important to note", "AI phrase: It is important to note".into()),
        ("delve into", "AI phrase: delve into".into()),
        ("tapestry of", "AI phrase: tapestry of".into()),
        ("rich tapestry", "AI phrase: rich tapestry".into()),
        ("landscape of", "AI phrase: landscape of".into()),
        ("multifaceted", "AI word: multifaceted".into()),
        ("comprehensive overview", "AI phrase: comprehensive overview".into()),
        ("it should be noted", "AI phrase: it should be noted".into()),
        ("in summary,", "AI phrase: In summary".into()),
        ("this underscores", "AI phrase: this underscores".into()),
        ("pivotal role", "AI phrase: pivotal role".into()),
        ("nuanced understanding", "AI phrase: nuanced understanding".into()),
    ]
}

/// Chinese AI-typical phrase patterns.
fn chinese_ai_patterns() -> Vec<(&'static str, String)> {
    vec![
        ("值得注意的是", "AI 短语：值得注意的是".into()),
        ("此外，", "AI 短语：此外".into()),
        ("与此同时，", "AI 短语：与此同时".into()),
        ("综上所述", "AI 短语：综上所述".into()),
        ("总而言之", "AI 短语：总而言之".into()),
        ("需要指出的是", "AI 短语：需要指出的是".into()),
        ("不可否认", "AI 短语：不可否认".into()),
        ("毋庸置疑", "AI 短语：毋庸置疑".into()),
        ("发挥着重要作用", "AI 短语：发挥着重要作用".into()),
        ("具有重要意义", "AI 短语：具有重要意义".into()),
        ("不可或缺", "AI 短语：不可或缺".into()),
        ("日益凸显", "AI 短语：日益凸显".into()),
    ]
}

/// Count sentences likely using passive voice (English heuristic).
///
/// Looks for "is/are/was/were/been/being + past participle" patterns.
fn count_passive_voice(text: &str) -> usize {
    let passive_markers = [
        " is ", " are ", " was ", " were ", " been ", " being ",
    ];
    let mut count = 0;
    for marker in &passive_markers {
        if let Some(pos) = text.find(marker) {
            // Check if followed by a word ending in "ed" (rough heuristic).
            let after = &text[pos + marker.len()..];
            let next_word: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '\'')
                .collect();
            if next_word.ends_with("ed") || next_word.ends_with("en") {
                count += 1;
            }
        }
    }
    count
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_low_burstiness_detected_as_ai() {
        // Uniform sentence lengths → low CV → high AI probability.
        let text = "The cat sat on the mat. The dog sat on the log. The bird sat on the wire. The fish swam in the pond.";
        let config = DetectionConfig::default();
        let score = detect_burstiness(text, &config);
        assert!(
            score > 0.3,
            "Uniform sentences should yield high AI probability, got {score}"
        );
    }

    #[test]
    fn test_high_burstiness_detected_as_human() {
        // Varied sentence lengths → high CV → low AI probability.
        let text = "I woke up. The morning sun filtered through ancient oaks outside my window, casting long amber shadows across the wooden floor. Coffee. Then silence.";
        let config = DetectionConfig::default();
        let score = detect_burstiness(text, &config);
        assert!(
            score < 0.5,
            "Varied sentences should yield low AI probability, got {score}"
        );
    }

    #[test]
    fn test_syntactic_pattern_detection() {
        let text = "Moreover, the results are noteworthy. Furthermore, it is worth noting that the landscape of research has evolved. In conclusion, we delve into multifaceted aspects.";
        let config = DetectionConfig::default();
        let (score, matched) = detect_syntactic_patterns(text, &config);
        assert!(score > 0.0, "Should detect AI patterns");
        assert!(
            matched.iter().any(|m| m.contains("Moreover")),
            "Should detect 'Moreover'"
        );
        assert!(
            matched.iter().any(|m| m.contains("worth noting")),
            "Should detect 'worth noting'"
        );
    }

    #[test]
    fn test_chinese_patterns() {
        let text = "值得注意的是，这一发现具有重要意义。此外，不可否认的是，该方法日益凸显其优势。";
        let config = DetectionConfig {
            language: Language::Chinese,
            ..Default::default()
        };
        let (score, matched) = detect_syntactic_patterns(text, &config);
        assert!(score > 0.0, "Should detect Chinese AI patterns");
        assert!(matched.len() >= 2, "Should match multiple patterns");
    }

    #[test]
    fn test_full_detect_pipeline() {
        let text = "Moreover, the study reveals important findings. Furthermore, it is worth noting that the landscape has changed. In conclusion, we delve into multifaceted issues. The results are comprehensive. This underscores the pivotal role of analysis.";
        let config = DetectionConfig::default();
        let results = detect(text, &config).unwrap();
        assert!(!results.is_empty(), "Should produce at least one result");
        for r in &results {
            assert!(r.ai_probability >= 0.0 && r.ai_probability <= 1.0);
            assert!(r.score <= 100);
            assert!(!r.signals.is_empty());
        }
    }
}
