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

/// 编译 JSON 中的 `review_gate_regexes` 字符串列表。无效条目会被跳过并打 `eprintln`，
/// 与 `embedded_review_routing_json_compiles_completely` 测试一同保证 JSON 改动不会静默退化。
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

/// 解析编译期嵌入的 `configs/framework/REVIEW_ROUTING_SIGNALS.json` 为运行时 `Loaded`。
/// JSON 由 `include_str!` 静态嵌入；解析失败或可编译 regex 为空 = 仓库自身契约破损，
/// 走 panic 让 CI / 启动期立即失败，避免历史上的「fallback 静默退化」第二真源。
fn load_from_embedded_json() -> Loaded {
    let parsed: ReviewRoutingSignalsFile = serde_json::from_str(EMBEDDED_JSON).expect(
        "embedded configs/framework/REVIEW_ROUTING_SIGNALS.json must parse; check JSON syntax",
    );
    let review_gate_regexes = compile_review_gate_regexes(&parsed.review_gate_regexes);
    assert!(
        !review_gate_regexes.is_empty(),
        "embedded REVIEW_ROUTING_SIGNALS.json compiled to zero review_gate regexes; \
         check pattern syntax (see eprintln traces from compile_review_gate_regexes)"
    );
    Loaded {
        review_gate_regexes,
        parallel_review_candidate: parsed.parallel_review_candidate,
    }
}

fn loaded() -> &'static Loaded {
    LOADED.get_or_init(load_from_embedded_json)
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
        assert_eq!(parsed.review_gate_regexes.len(), 17);
        assert!(!parsed.parallel_review_candidate.review_markers.is_empty());
        assert!(!parsed.parallel_review_candidate.breadth_markers.is_empty());
        assert!(!parsed.parallel_review_candidate.scope_markers.is_empty());
    }

    /// Guard against the kind of silent regex drop that the old `fallback_*` second source of truth
    /// used to paper over: every JSON pattern must compile, and every parallel-review marker bucket
    /// must remain populated. Failing here = JSON edit broke runtime behaviour.
    #[test]
    fn embedded_review_routing_json_compiles_completely() {
        let parsed: ReviewRoutingSignalsFile =
            serde_json::from_str(EMBEDDED_JSON).expect("embedded REVIEW_ROUTING_SIGNALS.json");
        let compiled = compile_review_gate_regexes(&parsed.review_gate_regexes);
        assert_eq!(
            compiled.len(),
            parsed.review_gate_regexes.len(),
            "one or more review_gate_regexes failed to compile; the SSOT cleanup expects 100% compile rate"
        );
        let parallel = &parsed.parallel_review_candidate;
        assert!(!parallel.review_markers.is_empty(), "review_markers empty");
        assert!(
            !parallel.breadth_markers.is_empty(),
            "breadth_markers empty"
        );
        assert!(!parallel.scope_markers.is_empty(), "scope_markers empty");
    }

    #[test]
    fn is_review_prompt_matches_standalone_review_short_prompt() {
        assert!(is_review_prompt("review"));
        assert!(is_review_prompt("  review  "));
        assert!(is_review_prompt("代码审查"));
        assert!(is_review_prompt("code review"));
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
    fn is_review_prompt_matches_standalone_compact_review() {
        assert!(is_review_prompt("全面review"));
    }

    #[test]
    fn is_review_prompt_ignores_host_hook_misfire_complaints() {
        assert!(!is_review_prompt(
            "cursor 对话频繁触发 claude 的 hook，深度review，我的设计是主 harness + 三个独立宿主"
        ));
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
