//! Claim drift detection for research workspaces.
//!
//! Implements §19.7 of the research harness spec:
//! - Structure drift: text similarity between original question and current hypothesis
//! - Perimeter breach: whether recent runs stay within falsifiable prediction
//! - Question drift: whether run descriptions still answer the original question
//! - Aggregation, threshold table, terminal-box rendering, barrier integration

use serde_json::{Value, json};
use std::collections::HashSet;

use crate::*;

/// Drift severity level matching §19.7.3 threshold table.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum DriftLevel {
    Normal,
    Attention,
    Warning,
    Blocking,
}

/// Full drift detection report.
#[derive(Debug, Clone)]
pub(super) struct DriftReport {
    pub(super) score: f64,
    pub(super) level: DriftLevel,
    pub(super) structure_drift: f64,
    pub(super) perimeter_breach: f64,
    pub(super) question_drift: f64,
    pub(super) warning_count: i64,
    #[allow(dead_code)]
    pub(super) original_question: String,
    #[allow(dead_code)]
    pub(super) active_claim: String,
    pub(super) suggestion: String,
}

/// Compute Jaccard similarity between two texts using compact_words sets.
/// Returns 0.0–1.0 where 1.0 = identical content words.
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let words_a: HashSet<String> = compact_words(a, 20).into_iter().collect();
    let words_b: HashSet<String> = compact_words(b, 20).into_iter().collect();

    if words_a.is_empty() && words_b.is_empty() {
        return 1.0; // both empty → identical
    }
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0; // one empty, one not → totally different
    }

    let intersection_len = words_a.intersection(&words_b).count();
    let union_len = words_a.union(&words_b).count();

    intersection_len as f64 / union_len as f64
}

/// Determine drift level from score and warning count per §19.7.3.
fn level_from_score(drift_score: f64, warning_count: i64) -> (DriftLevel, String) {
    if drift_score >= 0.8 || warning_count >= 3 {
        (DriftLevel::Blocking, "强制".to_string())
    } else if drift_score >= 0.6 {
        (DriftLevel::Warning, "警告".to_string())
    } else if drift_score >= 0.3 {
        (DriftLevel::Attention, "注意".to_string())
    } else {
        (DriftLevel::Normal, "正常".to_string())
    }
}

/// Compute a suggestion string based on drift components.
fn compute_suggestion(
    level: &DriftLevel,
    structure_drift: f64,
    perimeter_breach: f64,
    question_drift: f64,
) -> String {
    if *level == DriftLevel::Blocking {
        return "阻断执行：请确认研究方向是否已偏离原始问题，或更新 original_question".to_string();
    }
    if structure_drift >= 0.5 {
        return "结构偏移：当前假设已不同于原始问题，建议检查研究方向是否一致".to_string();
    }
    if perimeter_breach >= 0.5 {
        return "边界违例：最近实验超出 falsifiable_prediction 的 perimeter，建议检查实验范围".to_string();
    }
    if question_drift >= 0.5 {
        return "问题漂移：近期实验描述与原始问题不一致，建议检查是否仍在回答原问题".to_string();
    }
    "检查实验是否仍然在 perimeter 内".to_string()
}

/// Main drift detection entry point.
/// Returns a DriftReport with all component scores, level, and suggestion.
pub(super) fn detect_claim_drift(state: &Value) -> DriftReport {
    // 1. Extract original question from current_direction object or fallback
    let current_dir = state.get("current_direction");
    let original_question: String = current_dir
        .and_then(|cd| cd.get("original_question"))
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| str_key(state, "question"));

    let warning_count: i64 = current_dir
        .and_then(|cd| cd.get("deviation_warning_count"))
        .and_then(Value::as_i64)
        .unwrap_or(0);

    // 2. Get active hypothesis
    let active_hypothesis_id = state.get("active_hypothesis").and_then(Value::as_str);
    let active_hypothesis = active_hypothesis_id
        .and_then(|id| find_hypothesis(state, id));

    let active_claim = active_hypothesis
        .map(|h| str_field_default(h, "claim", ""))
        .unwrap_or_default();

    let falsifiable = active_hypothesis
        .and_then(|h| h.get("falsifiable_prediction"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // 3. Structure drift: similarity between original_question and active claim
    let similarity = jaccard_similarity(&original_question, &active_claim);
    let structure_drift = if similarity < 0.5 {
        round_to((1.0 - similarity) * 0.7 + 0.15, 2) // scale so <0.5 sim → >0.5 drift
    } else {
        round_to((1.0 - similarity) * 0.3, 2)
    };

    // 4. Perimeter breach: check recent runs' outcomes against falsifiable_prediction
    let runs = arr(state, "run_history");
    let recent_runs: Vec<&Value> = runs.iter().rev().take(5).collect();
    let has_perimeter = !falsifiable.is_empty();

    let breach_count = if has_perimeter {
        recent_runs.iter()
            .filter(|r| {
                let outcome = str_field_default(r, "outcome", "");
                // Non-confirmatory outcomes suggest perimeter breach
                outcome != "confirmatory" && !outcome.is_empty()
            })
            .count()
    } else {
        0
    };
    let perimeter_breach = if has_perimeter {
        round_to((breach_count as f64 / 2.0_f64).min(1.0), 2)
    } else {
        0.0
    };

    // 5. Question drift: compare run summaries/text with original_question
    let mismatch_count = recent_runs.iter()
        .filter(|r| {
            let summary = str_field_default(r, "summary", "");
            let finding = str_field_default(r, "finding", "");
            let run_text = if summary == "-" { &finding } else { &summary };
            if run_text.is_empty() || run_text == "-" {
                return false;
            }
            let sim = jaccard_similarity(&original_question, run_text);
            sim < 0.3
        })
        .count();
    let question_drift = round_to((mismatch_count as f64 / 3.0_f64).min(1.0), 2);

    // 6. Aggregate: weighted combination per spec §19.7.2
    let drift_score = round_to(
        0.35 * structure_drift + 0.35 * perimeter_breach + 0.30 * question_drift,
        2,
    );

    // 7. Determine level
    let (level, _label) = level_from_score(drift_score, warning_count);
    let suggestion = compute_suggestion(&level, structure_drift, perimeter_breach, question_drift);

    DriftReport {
        score: drift_score,
        level,
        structure_drift,
        perimeter_breach,
        question_drift,
        warning_count,
        original_question,
        active_claim,
        suggestion,
    }
}

/// Render the drift report in the spec's box-drawing format (§19.7.4).
#[allow(dead_code)]
pub(super) fn render_drift_report(report: &DriftReport) -> String {
    let level_label = match report.level {
        DriftLevel::Normal => "正常",
        DriftLevel::Attention => "注意",
        DriftLevel::Warning => "警告",
        DriftLevel::Blocking => "强制",
    };

    let structure_label = if report.structure_drift >= 0.5 { "⚠️ " } else { "" };
    let perimeter_label = if report.perimeter_breach >= 0.5 { "⚠️ " } else { "" };
    let question_label = if report.question_drift >= 0.5 { "⚠️ " } else { "" };

    let structure_desc = drift_component_desc(report.structure_drift);
    let perimeter_desc = drift_component_desc(report.perimeter_breach);
    let question_desc = drift_component_desc(report.question_drift);

    format!(
        "╔══════════════════════════════════════════╗\n\
         ║         Claim Drift 检测报告             ║\n\
         ╠══════════════════════════════════════════╣\n\
         ║ 原始问题: {:<35} ║\n\
         ║ 当前假设: {:<35} ║\n\
         ║ ────────────────────────────────────────── ║\n\
         ║ {structure_label}结构偏移: {:<4.1} ({:<8})             ║\n\
         ║ {perimeter_label}边界违例: {:<4.1} ({:<8})             ║\n\
         ║ {question_label}问题漂移: {:<4.1} ({:<8})             ║\n\
         ║ ────────────────────────────────────────── ║\n\
         ║ 综合评分: {:<4.2} ({:<8})                 ║\n\
         ║ 累计警告: {:<1} / 3                           ║\n\
         ║ ────────────────────────────────────────── ║\n\
         ║ 建议: {:<39} ║\n\
         ╚══════════════════════════════════════════╝",
        truncate(&report.original_question, 35),
        truncate(&report.active_claim, 35),
        report.structure_drift, structure_desc,
        report.perimeter_breach, perimeter_desc,
        report.question_drift, question_desc,
        report.score, level_label,
        report.warning_count,
        truncate(&report.suggestion, 39),
    )
}

#[allow(dead_code)]
fn drift_component_desc(value: f64) -> String {
    if value >= 0.5 {
        "偏离".to_string()
    } else if value >= 0.2 {
        "轻微".to_string()
    } else {
        "正常".to_string()
    }
}

#[allow(dead_code)]
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        format!("{:<width$}", text, width = max)
    } else {
        format!("{}...", text.chars().take(max - 3).collect::<String>())
    }
}

fn round_to(value: f64, decimals: i32) -> f64 {
    let factor = 10i64.pow(decimals as u32) as f64;
    (value * factor).round() / factor
}

/// Format drift report for appending to a barrier report's attempted list.
#[allow(dead_code)]
pub(super) fn drift_to_barrier_entry(report: &DriftReport) -> Value {
    json!({
        "detection": "claim_drift",
        "drift_score": report.score,
        "drift_level": format!("{:?}", report.level),
        "structure_drift": report.structure_drift,
        "perimeter_breach": report.perimeter_breach,
        "question_drift": report.question_drift,
        "suggestion": report.suggestion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state_with_question(question: &str) -> Value {
        ensure_state_defaults(&json!({ "question": question }))
    }

    #[test]
    fn jaccard_identical_texts() {
        let sim = jaccard_similarity("Does method X improve accuracy", "Does method X improve accuracy");
        assert!((sim - 1.0).abs() < 0.01, "expected ~1.0, got {sim}");
    }

    #[test]
    fn jaccard_completely_different() {
        let sim = jaccard_similarity("Does method X improve accuracy", "How to cook pasta");
        assert!(sim < 0.3, "expected low similarity, got {sim}");
    }

    #[test]
    fn jaccard_partial_overlap() {
        let sim = jaccard_similarity("transformer attention mechanism", "attention is all you need transformer");
        assert!(sim > 0.2 && sim < 0.9, "expected moderate similarity, got {sim}");
    }

    #[test]
    fn jaccard_both_empty() {
        let sim = jaccard_similarity("", "");
        assert!((sim - 1.0).abs() < 0.01, "empty identical should be 1.0");
    }

    #[test]
    fn jaccard_one_empty() {
        let sim = jaccard_similarity("something", "");
        assert!((sim - 0.0).abs() < 0.01, "one empty should be 0.0");
    }

    #[test]
    fn level_normal_below_03() {
        assert!(matches!(level_from_score(0.29, 0).0, DriftLevel::Normal));
    }

    #[test]
    fn level_attention_at_03() {
        assert!(matches!(level_from_score(0.30, 0).0, DriftLevel::Attention));
    }

    #[test]
    fn level_attention_below_06() {
        assert!(matches!(level_from_score(0.59, 0).0, DriftLevel::Attention));
    }

    #[test]
    fn level_warning_at_06() {
        assert!(matches!(level_from_score(0.60, 0).0, DriftLevel::Warning));
    }

    #[test]
    fn level_warning_below_08() {
        assert!(matches!(level_from_score(0.79, 0).0, DriftLevel::Warning));
    }

    #[test]
    fn level_blocking_at_08() {
        assert!(matches!(level_from_score(0.80, 0).0, DriftLevel::Blocking));
    }

    #[test]
    fn level_blocking_warning_count_ge_3() {
        assert!(matches!(level_from_score(0.1, 3).0, DriftLevel::Blocking));
    }

    #[test]
    fn detect_drift_normal_state() {
        let state = state_with_question("Does method X improve accuracy on dataset Y?");
        let report = detect_claim_drift(&state);
        // No active hypothesis → structure drift defaults
        assert!(report.score >= 0.0);
        assert_eq!(report.original_question, "Does method X improve accuracy on dataset Y?");
    }

    #[test]
    fn detect_drift_with_hypothesis() {
        // State with question and matching hypothesis
        let mut state = ensure_state_defaults(&json!({
            "hypotheses": [{
                "id": "h1",
                "claim": "Using transformer improves accuracy",
                "status": "active",
                "falsifiable_prediction": "accuracy > 90% on test set",
                "created_at": "2026-01-01T00:00:00Z"
            }]
        }));
        set_key(&mut state, "active_hypothesis", json!("h1"));
        set_key(&mut state, "question", json!("Does transformer improve accuracy?"));

        let report = detect_claim_drift(&state);
        // Similar question + matching claim → low drift
        assert!(
            report.score < 0.5,
            "matching question and hypothesis should have low drift, got {}",
            report.score
        );
    }

    #[test]
    fn detect_drift_with_current_direction_object() {
        let mut state = ensure_state_defaults(&json!({
            "current_direction": {
                "original_question": "Does X improve Y?",
                "last_reaffirmed": "2026-06-01T00:00:00Z",
                "deviation_warning_count": 2
            },
            "run_history": [{
                "run_id": "run-001",
                "hypothesis_id": "h1",
                "outcome": "failed",
                "summary": "tuning hyperparameters for convergence"
            }]
        }));
        set_key(&mut state, "question", json!("Does X improve Y?"));
        let report = detect_claim_drift(&state);
        assert_eq!(report.warning_count, 2);
        assert_eq!(report.original_question, "Does X improve Y?");
    }

    #[test]
    fn drift_report_renders_box() {
        let report = DriftReport {
            score: 0.45,
            level: DriftLevel::Attention,
            structure_drift: 0.1,
            perimeter_breach: 0.7,
            question_drift: 0.2,
            warning_count: 2,
            original_question: "Does method X improve Y?".into(),
            active_claim: "Under Z condition accuracy improves 5%".into(),
            suggestion: "检查实验是否仍然在 perimeter 内".into(),
        };
        let rendered = render_drift_report(&report);
        assert!(rendered.contains("Claim Drift 检测报告"));
        assert!(rendered.contains("综合评分: 0.45"));
        assert!(rendered.contains("累计警告: 2"));
        assert!(rendered.contains("边界违例"));
        assert!(rendered.contains("检查实验"));
    }

    #[test]
    fn drift_report_blocking_level() {
        let report = DriftReport {
            score: 0.85,
            level: DriftLevel::Blocking,
            structure_drift: 0.8,
            perimeter_breach: 0.5,
            question_drift: 0.6,
            warning_count: 3,
            original_question: "original".into(),
            active_claim: "something different".into(),
            suggestion: "阻断执行".into(),
        };
        let rendered = render_drift_report(&report);
        assert!(rendered.contains("强制"));
        assert!(rendered.contains("阻断执行"));
    }

    #[test]
    fn drift_to_barrier_entry_has_expected_fields() {
        let report = DriftReport {
            score: 0.7,
            level: DriftLevel::Warning,
            structure_drift: 0.6,
            perimeter_breach: 0.3,
            question_drift: 0.4,
            warning_count: 1,
            original_question: "q".into(),
            active_claim: "c".into(),
            suggestion: "check perimeter".into(),
        };
        let entry = drift_to_barrier_entry(&report);
        assert_eq!(entry["detection"], "claim_drift");
        assert!((entry["drift_score"].as_f64().unwrap() - 0.7).abs() < 0.01);
    }
}
