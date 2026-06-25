//! Text humanizer — reduce AIGC signals through vocabulary swap, syntactic rewrite,
//! sentence variation, and clause restructuring.
//!
//! All strategies are pure-local rule-based transforms; no API calls.

use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

use crate::aigc::Language;
use crate::types::HumanizeResult;

// ── Config ──

/// Humanization configuration.
#[derive(Debug, Clone)]
pub struct HumanizeConfig {
    /// Strategies to apply (in order).
    pub strategies: Vec<HumanizeStrategy>,
    /// Whether to preserve academic tone (avoids colloquial filler).
    pub preserve_academic_tone: bool,
    /// Target AIGC score to aim for.
    pub target_score: u32,
    /// Language of the input text.
    pub language: Language,
}

impl Default for HumanizeConfig {
    fn default() -> Self {
        Self {
            strategies: vec![
                HumanizeStrategy::VocabularySwap,
                HumanizeStrategy::SyntacticRewrite,
                HumanizeStrategy::SentenceVariation,
                HumanizeStrategy::ClauseRestructure,
            ],
            preserve_academic_tone: true,
            target_score: 30,
            language: Language::English,
        }
    }
}

// ── Strategy enum ──

/// Humanization strategy enum.
#[derive(Debug, Clone)]
pub enum HumanizeStrategy {
    /// Syntactic rewrite — adjust sentence structure (active/passive, clause order).
    SyntacticRewrite,
    /// Vocabulary swap — replace AI-high-frequency words with natural alternatives.
    VocabularySwap,
    /// Sentence variation — break uniformity, inject long/short alternation.
    SentenceVariation,
    /// Clause restructure — simplify nested clauses, split compound sentences.
    ClauseRestructure,
}

// ── Public API ──

/// Apply humanization strategies to text.
///
/// Each strategy is applied sequentially; the output of one feeds into the next.
/// The `strategies` slice determines which transforms run and in what order.
pub fn humanize(text: &str, strategies: &[HumanizeStrategy]) -> Result<HumanizeResult> {
    humanize_with_config(
        text,
        &HumanizeConfig {
            strategies: strategies.to_vec(),
            ..Default::default()
        },
    )
}

/// Apply humanization with full configuration control.
pub fn humanize_with_config(text: &str, config: &HumanizeConfig) -> Result<HumanizeResult> {
    let mut current = text.to_string();
    let mut applied = Vec::new();

    for strategy in &config.strategies {
        match strategy {
            HumanizeStrategy::VocabularySwap => {
                let (new_text, count) = vocabulary_swap(&current, config.language);
                if count > 0 {
                    current = new_text;
                    applied.push(format!("vocabulary_swap({count} replacements)"));
                }
            }
            HumanizeStrategy::SyntacticRewrite => {
                let (new_text, count) = syntactic_rewrite(&current, config.language);
                if count > 0 {
                    current = new_text;
                    applied.push(format!("syntactic_rewrite({count} transforms)"));
                }
            }
            HumanizeStrategy::SentenceVariation => {
                let (new_text, count) = sentence_variation(&current, config.language, config.preserve_academic_tone);
                if count > 0 {
                    current = new_text;
                    applied.push(format!("sentence_variation({count} adjustments)"));
                }
            }
            HumanizeStrategy::ClauseRestructure => {
                let (new_text, count) = clause_restructure(&current, config.language);
                if count > 0 {
                    current = new_text;
                    applied.push(format!("clause_restructure({count} splits)"));
                }
            }
        }
    }

    let improvement = estimate_improvement(text, &current, config.language);

    Ok(HumanizeResult {
        original: text.to_string(),
        rewritten: current,
        strategies_applied: applied,
        estimated_score_improvement: improvement,
    })
}

// ── Strategy 1: Vocabulary Swap ──

/// Replace AI-high-frequency words/phrases with natural alternatives.
fn vocabulary_swap(text: &str, language: Language) -> (String, usize) {
    let replacements = match language {
        Language::English => english_replacement_table(),
        Language::Chinese => chinese_replacement_table(),
    };

    let mut result = text.to_string();
    let mut count = 0;

    for (from, to) in &replacements {
        let before = result.clone();
        match language {
            Language::English => {
                // Case-insensitive word-boundary replacement.
                let re = match Regex::new(&format!(r"(?i)\b{}\b", regex::escape(from))) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                result = re.replace_all(&result, to.as_str()).to_string();
            }
            Language::Chinese => {
                // Simple substring replacement — no word boundary in Chinese.
                result = result.replace(from.as_str(), to.as_str());
            }
        }
        if result != before {
            count += 1;
        }
    }

    // Strip "It is worth noting that" / "It should be noted that" → direct statement.
    if language == Language::English {
        let patterns = [
            (r"(?i)It is worth noting that\s+", ""),
            (r"(?i)It should be noted that\s+", ""),
            (r"(?i)It is important to note that\s+", ""),
            (r"(?i)Notably,\s*", ""),
        ];
        for (pat, repl) in &patterns {
            if let Ok(re) = Regex::new(pat) {
                let before = result.clone();
                result = re.replace_all(&result, *repl).to_string();
                if result != before {
                    count += 1;
                }
            }
        }
    }

    // Chinese direct-statement strips
    if language == Language::Chinese {
        let patterns = [
            ("值得注意的是，", ""),
            ("需要指出的是，", ""),
        ];
        for (from, to) in &patterns {
            if result.contains(from) {
                result = result.replace(from, to);
                count += 1;
            }
        }
    }

    (result, count)
}

/// English AI-word replacement table.
fn english_replacement_table() -> Vec<(String, String)> {
    vec![
        ("moreover".into(), "additionally".into()),
        ("furthermore".into(), "what's more".into()),
        ("delve into".into(), "explore".into()),
        ("delve".into(), "investigate".into()),
        ("tapestry".into(), "framework".into()),
        ("rich tapestry".into(), "framework".into()),
        ("landscape".into(), "field".into()),
        ("multifaceted".into(), "complex".into()),
        ("underscore".into(), "highlight".into()),
        ("underscores".into(), "highlights".into()),
        ("pivotal".into(), "key".into()),
        ("pivotal role".into(), "key role".into()),
        ("nuanced understanding".into(), "detailed understanding".into()),
        ("comprehensive overview".into(), "broad review".into()),
        ("in conclusion".into(), "to summarize".into()),
        ("in summary".into(), "taken together".into()),
    ]
}

/// Chinese AI-word replacement table (ordered by length descending to prevent
/// substring overlap — e.g. "此外" must not match inside "与此同时").
fn chinese_replacement_table() -> Vec<(String, String)> {
    vec![
        ("发挥着重要作用".into(), "起关键作用".into()),
        ("具有重要意义".into(), "至关重要".into()),
        ("综上所述".into(), "综合来看".into()),
        ("总而言之".into(), "概括而言".into()),
        ("与此同时".into(), "另外".into()),
        ("不可否认".into(), "可以看到".into()),
        ("毋庸置疑".into(), "显而易见".into()),
        ("不可或缺".into(), "必要".into()),
        ("日益凸显".into(), "逐渐突出".into()),
        ("此外".into(), "同时".into()),
    ]
}

// ── Strategy 2: Syntactic Rewrite ──

// Pre-compiled regex patterns for syntactic rewriting.
static RE_IT_IS_ADJ_THAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)It is (\w+) that ([^.]+)\.").expect("invalid IT_IS_ADJ_THAT regex")
});
static RE_PASSIVE_BY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)The (\w+) was (\w+) by the (\w+)\.").expect("invalid PASSIVE_BY regex")
});
static RE_BEI_SOU_ZH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"被(\w+)所(\w+)").expect("invalid BEI_SOU regex")
});
static RE_WHICH_CLAUSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r",\s+which ([^,]+),\s+").expect("invalid WHICH_CLAUSE regex")
});
static RE_SEMICOLON_CONNECTORS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r";\s+(moreover|furthermore|however|nevertheless),\s+").expect("invalid SEMICOLON regex")
});
static RE_AND_CONNECTOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.{40,}?), and (.+)$").expect("invalid AND_CONNECTOR regex")
});
static RE_BINGQIE_ZH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(.{20,}?)，并且(.+?)(。|$)").expect("invalid BINGQIE regex")
});

/// Apply syntactic transforms: passive→active reordering, clause reordering.
fn syntactic_rewrite(text: &str, language: Language) -> (String, usize) {
    if language == Language::Chinese {
        return syntactic_rewrite_zh(text);
    }

    let mut result = text.to_string();
    let mut count = 0;

    // Transform: "It is [adj] that [clause]" → "[Clause] is [adj]."
    if RE_IT_IS_ADJ_THAT.is_match(&result) {
        result = RE_IT_IS_ADJ_THAT
            .replace_all(&result, |caps: &regex::Captures| {
                let adj = &caps[1];
                let clause = &caps[2];
                // Capitalize first letter of clause.
                let mut chars = clause.chars();
                let first = chars.next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
                let rest: String = chars.collect();
                format!("{first}{rest} is {adj}.")
            })
            .to_string();
        count += 1;
    }

    // Transform: "The [noun] was [past participle] by [agent]" → "[Agent] [verb]s the [noun]."
    // This is a simplified heuristic.
    if RE_PASSIVE_BY.is_match(&result) {
        result = RE_PASSIVE_BY
            .replace_all(&result, |caps: &regex::Captures| {
                let noun = &caps[1];
                let verb = &caps[2];
                let agent = &caps[3];
                // Simple conjugation: add "s" for third person.
                let v = format!("{verb}s");
                let mut a_chars = agent.chars();
                let a_first = a_chars.next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
                let a_rest: String = a_chars.collect();
                format!("{a_first}{a_rest} {v} the {noun}.")
            })
            .to_string();
        count += 1;
    }

    (result, count)
}

fn syntactic_rewrite_zh(text: &str) -> (String, usize) {
    // Chinese syntactic rewrite is more conservative.
    // Convert overly formal "被" passive to active where possible.
    let mut result = text.to_string();
    let mut count = 0;

    // "被...所" → keep 被 (preserves passive voice), remove 所 (archaic)
    // e.g. "被他所救" → "被他救" (still passive, correct)
    if result.contains("被") {
        let before = result.clone();
        result = RE_BEI_SOU_ZH.replace_all(&result, "被$1$2").to_string();
        if result != before {
            count += 1;
        }
    }

    (result, count)
}

// ── Strategy 3: Sentence Variation ──

/// Break uniform sentence patterns by injecting length variation.
fn sentence_variation(
    text: &str,
    language: Language,
    preserve_academic: bool,
) -> (String, usize) {
    let sentences = split_sentences(text, language);
    if sentences.len() < 3 {
        return (text.to_string(), 0);
    }

    let lengths: Vec<usize> = sentences.iter().map(|s| s.len()).collect();
    let mean = lengths.iter().sum::<usize>() as f64 / lengths.len() as f64;

    let mut result_parts: Vec<String> = Vec::new();
    let mut count = 0;

    for (i, sentence) in sentences.iter().enumerate() {
        let mut s = sentence.clone();

        // If this sentence is very close to the mean length and there's a long one coming,
        // try to split it or add a short transitional phrase.
        let deviation = (s.len() as f64 - mean).abs();
        if deviation < mean * 0.1 && sentences.len() > 3 {
            // This sentence is suspiciously close to average.
            // Every other such sentence, prepend a short connector.
            if i % 2 == 0 && !preserve_academic {
                s = format!("So, {}", lowercase_first(&s));
                count += 1;
            } else if i > 0 && s.len() > 60 {
                // Try to split a long sentence at a comma.
                if let Some(comma_pos) = s[10..].find(',').map(|p| p + 10) {
                    let first_part = s[..comma_pos].trim().to_string();
                    let second_part = s[comma_pos + 1..].trim().to_string();
                    if !first_part.is_empty() && !second_part.is_empty() {
                        result_parts.push(ensure_ending(&first_part));
                        result_parts.push(capitalize_first(&second_part));
                        count += 1;
                        continue;
                    }
                }
            }
        }

        result_parts.push(s);
    }

    (result_parts.join(" "), count)
}

// ── Strategy 4: Clause Restructure ──

/// Simplify nested clauses and split compound sentences.
fn clause_restructure(text: &str, language: Language) -> (String, usize) {
    if language == Language::Chinese {
        return clause_restructure_zh(text);
    }

    let mut result = text.to_string();
    let mut count = 0;

    // Split "X, which Y, Z" into "X Z. This Y."
    if RE_WHICH_CLAUSE.is_match(&result) {
        let before = result.clone();
        result = RE_WHICH_CLAUSE
            .replace_all(&result, ". This $1 and ")
            .to_string();
        if result != before {
            count += 1;
        }
    }

    // Split "X; moreover, Y" or "X; furthermore, Y" into two sentences.
    if RE_SEMICOLON_CONNECTORS.is_match(&result) {
        let before = result.clone();
        result = RE_SEMICOLON_CONNECTORS
            .replace_all(&result, |caps: &regex::Captures| {
                let connector = &caps[1];
                let c = capitalize_first(connector);
                format!(". {c}, ")
            })
            .to_string();
        if result != before {
            count += 1;
        }
    }

    // Split long "X, and Y" sentences where X is > 50 chars.
    let lines: Vec<String> = result
        .split(". ")
        .map(|s| {
            let trimmed = s.trim();
            if let Some(caps) = RE_AND_CONNECTOR.captures(trimmed) {
                let x = caps[1].trim();
                let y = caps[2].trim();
                if !x.is_empty() && !y.is_empty() {
                    count += 1;
                    return format!("{}. {}", ensure_ending(x), capitalize_first(y));
                }
            }
            trimmed.to_string()
        })
        .collect();

    (lines.join(". "), count)
}

fn clause_restructure_zh(text: &str) -> (String, usize) {
    let mut result = text.to_string();
    let mut count = 0;

    // Split long "……，并且……" sentences.
    if RE_BINGQIE_ZH.is_match(&result) {
        let before = result.clone();
        result = RE_BINGQIE_ZH
            .replace_all(&result, |caps: &regex::Captures| {
                count += 1;
                format!("{}。{}", caps[1].trim(), caps[2].trim())
            })
            .to_string();
        if result == before {
            count = 0;
        }
    }

    (result, count)
}

// ── Helpers ──

/// Estimate the AIGC score improvement from text transformation.
fn estimate_improvement(original: &str, rewritten: &str, language: Language) -> f64 {
    // Count AI-pattern occurrences before and after.
    let patterns = match language {
        Language::English => vec![
            "moreover", "furthermore", "in conclusion",
            "it is worth noting", "delve", "tapestry", "multifaceted",
            "landscape", "underscore", "pivotal",
        ],
        Language::Chinese => vec![
            "值得注意的是", "此外", "综上所述",
            "总而言之", "不可否认", "毋庸置疑",
            "不可或缺", "日益凸显",
        ],
    };

    let original_lower = original.to_lowercase();
    let rewritten_lower = rewritten.to_lowercase();

    let orig_count: usize = patterns
        .iter()
        .map(|p| original_lower.matches(p).count())
        .sum();
    let new_count: usize = patterns
        .iter()
        .map(|p| rewritten_lower.matches(p).count())
        .sum();

    if orig_count == 0 {
        return 0.0;
    }

    let reduction = (orig_count as f64 - new_count as f64) / orig_count as f64;
    (reduction * 30.0).max(0.0) // max ~30 points improvement
}

fn split_sentences(text: &str, language: Language) -> Vec<String> {
    match language {
        Language::English => text
            .split(['.', '!', '?'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Language::Chinese => text
            .split(['。', '！', '？', '.', '!', '?'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_lowercase(), chars.collect::<String>()),
        None => String::new(),
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.collect::<String>()),
        None => String::new(),
    }
}

fn ensure_ending(s: &str) -> String {
    if s.ends_with('.') || s.ends_with('!') || s.ends_with('?') {
        s.to_string()
    } else {
        format!("{s}.")
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moreover_replaced() {
        let text = "Moreover, the results show significant improvement.";
        let result = humanize(text, &[HumanizeStrategy::VocabularySwap]).unwrap();
        assert!(
            !result.rewritten.to_lowercase().contains("moreover"),
            "'Moreover' should be replaced, got: {}",
            result.rewritten
        );
        assert!(
            !result.strategies_applied.is_empty(),
            "At least one strategy should be recorded"
        );
    }

    #[test]
    fn test_furthermore_replaced() {
        let text = "Furthermore, the approach is robust.";
        let result = humanize(text, &[HumanizeStrategy::VocabularySwap]).unwrap();
        assert!(
            !result.rewritten.to_lowercase().contains("furthermore"),
            "'Furthermore' should be replaced"
        );
    }

    #[test]
    fn test_chinese_replacement() {
        let text = "此外，不可否认的是，该方法具有重要意义。";
        let result = humanize_with_config(
            text,
            &HumanizeConfig {
                language: Language::Chinese,
                strategies: vec![HumanizeStrategy::VocabularySwap],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            !result.rewritten.contains("不可否认"),
            "Chinese AI pattern should be replaced"
        );
    }

    #[test]
    fn test_it_is_worth_noting_stripped() {
        let text = "It is worth noting that the model performs well.";
        let result = humanize(text, &[HumanizeStrategy::VocabularySwap]).unwrap();
        assert!(
            !result.rewritten.to_lowercase().contains("it is worth noting"),
            "Filler phrase should be stripped"
        );
    }

    #[test]
    fn test_original_preserved_in_result() {
        let text = "Furthermore, we delve into the issue.";
        let result = humanize(text, &[HumanizeStrategy::VocabularySwap]).unwrap();
        assert_eq!(result.original, text);
    }

    #[test]
    fn test_multiple_strategies_chained() {
        let text = "Moreover, the study was conducted by the team. Furthermore, it is worth noting that results are significant.";
        let result = humanize(
            text,
            &[
                HumanizeStrategy::VocabularySwap,
                HumanizeStrategy::SyntacticRewrite,
            ],
        )
        .unwrap();
        assert!(
            !result.rewritten.is_empty(),
            "Should produce non-empty output"
        );
        assert!(
            !result.strategies_applied.is_empty(),
            "At least one strategy should fire"
        );
    }
}
