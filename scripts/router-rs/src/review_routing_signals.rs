//! Single source of truth for review gate regexes and parallel-review routing markers.
//! Embedded JSON: `configs/framework/REVIEW_ROUTING_SIGNALS.json`.

use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

const EMBEDDED_JSON: &str = include_str!("../../../configs/framework/REVIEW_ROUTING_SIGNALS.json");

#[derive(Debug, Deserialize)]
struct ReviewRoutingSignalsFile {
    review_gate_regexes: Vec<String>,
    parallel_review_candidate: ParallelReviewCandidateFile,
}

#[derive(Debug, Deserialize)]
struct ParallelReviewCandidateFile {
    review_markers: Vec<String>,
    breadth_markers: Vec<String>,
    scope_markers: Vec<String>,
}

struct Loaded {
    review_gate_regexes: Vec<Regex>,
    parallel_review_candidate: ParallelReviewCandidateFile,
}

static LOADED: OnceLock<Loaded> = OnceLock::new();

/// Fallback mirrors embedded JSON / prior `hook_common::review_patterns` literals.
fn fallback_review_gate_pattern_strings() -> &'static [&'static str] {
    &[
        r"(?i)\b(code|security|architecture|architect)\s+review\b",
        r"(?i)\breview\s+this\s+(pr|pull request)\b",
        r"(?i)\breview\s+(my\s+)?(pr|pull request)\b",
        r"(?i)\b(pr|pull request)\s+review\b",
        r"(?i)\breview\s+(code|security|architecture)\b",
        r"(?im)^\s*review\b.*\bagain\b",
        r"(?i)\bfocus on finding\b.*\bproblems\b",
        r"(?i)(深度|全面|全仓|仓库级|跨模块|多模块|多维)\s*review",
        r"(?i)review.*(仓库|全仓|跨模块|多模块|严重程度|findings|severity|repo|repository|cross[- ]module|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
        r"(?i)(深度|全面|全仓|仓库级|跨模块|多模块|多维).*(审查|审核|审计|评审)",
        r"(?i)(审查|审核|审计|评审).*(仓库|全仓|跨模块|多模块|严重程度|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
        r"(?i)(代码审查|安全审查|架构审查|审查这个\s*PR|审查这段代码)",
        r"(?i)(审查|评审|审核).*(PR|pull request|合并请求)",
    ]
}

fn fallback_parallel_review_candidate() -> ParallelReviewCandidateFile {
    ParallelReviewCandidateFile {
        review_markers: vec![
            "review".to_string(),
            "code review".to_string(),
            "审查".to_string(),
            "审核".to_string(),
            "审计".to_string(),
            "评审".to_string(),
            "代码 review".to_string(),
            "代码审查".to_string(),
            "架构审查".to_string(),
            "安全审查".to_string(),
        ],
        breadth_markers: vec![
            "深度".to_string(),
            "全面".to_string(),
            "全量".to_string(),
            "全仓".to_string(),
            "仓库级".to_string(),
            "整个仓库".to_string(),
            "这个仓库".to_string(),
            "repo-wide".to_string(),
            "codebase-wide".to_string(),
            "跨模块".to_string(),
            "多模块".to_string(),
            "多维".to_string(),
            "多方向".to_string(),
            "多假设".to_string(),
            "第一性原理".to_string(),
            "多余入口".to_string(),
            "不必要抽象".to_string(),
        ],
        scope_markers: vec![
            "仓库".to_string(),
            "repo".to_string(),
            "codebase".to_string(),
            "代码库".to_string(),
            "架构".to_string(),
            "architecture".to_string(),
            "系统".to_string(),
            "路由".to_string(),
            "skill".to_string(),
            "模块".to_string(),
            "边界".to_string(),
            "实现质量".to_string(),
            "bug".to_string(),
            "风险".to_string(),
            "findings".to_string(),
        ],
    }
}

fn compile_review_gate_regexes(patterns: &[String]) -> Vec<Regex> {
    let mut out = Vec::with_capacity(patterns.len());
    for (index, pattern) in patterns.iter().enumerate() {
        match Regex::new(pattern.as_str()) {
            Ok(regex) => out.push(regex),
            Err(err) => eprintln!(
                "[router-rs] REVIEW_ROUTING_SIGNALS: skipping invalid review_gate_regexes[{index}]: {err}; pattern={pattern:?}"
            ),
        }
    }
    out
}

fn compile_review_gate_from_str_slices(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|p| Regex::new(p).expect("fallback review gate regex must compile (sync with JSON)"))
        .collect()
}

fn try_load_from_embedded_json() -> Option<Loaded> {
    let parsed: ReviewRoutingSignalsFile = serde_json::from_str(EMBEDDED_JSON).ok()?;
    let review_gate_regexes = compile_review_gate_regexes(&parsed.review_gate_regexes);
    if review_gate_regexes.is_empty() {
        return None;
    }
    Some(Loaded {
        review_gate_regexes,
        parallel_review_candidate: parsed.parallel_review_candidate,
    })
}

fn load_or_fallback() -> Loaded {
    if let Some(loaded) = try_load_from_embedded_json() {
        return loaded;
    }
    Loaded {
        review_gate_regexes: compile_review_gate_from_str_slices(
            fallback_review_gate_pattern_strings(),
        ),
        parallel_review_candidate: fallback_parallel_review_candidate(),
    }
}

fn loaded() -> &'static Loaded {
    LOADED.get_or_init(load_or_fallback)
}

/// Compiled review-gate heuristics (same semantics as former `hook_common::review_patterns`).
pub(crate) fn review_gate_compiled_regexes() -> &'static [Regex] {
    &loaded().review_gate_regexes
}

pub(crate) struct ParallelReviewCandidateMarkers {
    pub review_markers: &'static [String],
    pub breadth_markers: &'static [String],
    pub scope_markers: &'static [String],
}

pub(crate) fn parallel_review_candidate_markers() -> ParallelReviewCandidateMarkers {
    let l = loaded();
    ParallelReviewCandidateMarkers {
        review_markers: &l.parallel_review_candidate.review_markers,
        breadth_markers: &l.parallel_review_candidate.breadth_markers,
        scope_markers: &l.parallel_review_candidate.scope_markers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_common::is_review_prompt;
    use crate::route::has_parallel_review_candidate_context;
    use crate::route::tokenize_query;

    #[test]
    fn embedded_review_routing_json_parses() {
        let parsed: ReviewRoutingSignalsFile =
            serde_json::from_str(EMBEDDED_JSON).expect("embedded REVIEW_ROUTING_SIGNALS.json");
        assert_eq!(parsed.review_gate_regexes.len(), 13);
        assert!(!parsed.parallel_review_candidate.review_markers.is_empty());
        assert!(!parsed.parallel_review_candidate.breadth_markers.is_empty());
        assert!(!parsed.parallel_review_candidate.scope_markers.is_empty());
    }

    #[test]
    fn is_review_prompt_matches_code_review_phrase() {
        assert!(is_review_prompt("Please do a code review of this change."));
    }

    #[test]
    fn is_review_prompt_matches_depth_review_chinese() {
        assert!(is_review_prompt("深度 review 整个路由系统"));
    }

    #[test]
    fn has_parallel_review_candidate_and_semantics_unchanged() {
        let q = "做一次深度 code review，聚焦仓库架构与模块边界风险";
        let tokens = tokenize_query(q);
        assert!(has_parallel_review_candidate_context(q, &tokens));
    }

    #[test]
    fn has_parallel_review_candidate_requires_all_three_groups() {
        let q = "review this file only";
        let tokens = tokenize_query(q);
        assert!(!has_parallel_review_candidate_context(q, &tokens));
    }
}
