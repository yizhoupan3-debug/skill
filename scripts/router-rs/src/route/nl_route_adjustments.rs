//! Data-driven NL hot-route suppress/boost rules from embedded
//! [`NL_ROUTE_ADJUSTMENTS.json`](../../../configs/framework/NL_ROUTE_ADJUSTMENTS.json).

use super::signals::*;
use super::types::{RouteCandidate, SkillRecord};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;

const NL_EMBED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/framework/NL_ROUTE_ADJUSTMENTS.json"
));

const EXPECTED_SCHEMA: &str = "nl-route-adjustments-v1";

type NlSignalEvalFn = fn(&SkillRecord, &str, &[String], &HashSet<String>) -> bool;

#[derive(Debug, Clone, Copy)]
struct NlSignalEntry {
    name: &'static str,
    eval: NlSignalEvalFn,
}

fn nl_sig_has_beamer_slide_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_beamer_slide_context(query_text, query_token_list)
}

fn nl_sig_has_bounded_subagent_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_bounded_subagent_context(query_text, query_token_list)
}

fn nl_sig_has_ci_failure_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_ci_failure_context(query_text, query_token_list)
}

fn nl_sig_has_copywriting_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_copywriting_context(query_text, query_token_list)
}

fn nl_sig_has_design_contract_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_design_contract_context(query_text, query_token_list)
}

fn nl_sig_has_design_contract_negation_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_design_contract_negation_context(query_text, query_token_list)
}

fn nl_sig_has_design_output_audit_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_design_output_audit_context(query_text, query_token_list)
}

fn nl_sig_has_design_reference_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_design_reference_context(query_text, query_token_list)
}

fn nl_sig_has_design_workflow_protocol_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_design_workflow_protocol_context(query_text, query_token_list)
}

fn nl_sig_has_diagramming_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_diagramming_context(query_text, query_token_list)
}

fn nl_sig_has_github_pr_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_github_pr_context(query_text, query_token_list)
}

fn nl_sig_has_paper_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_paper_context(query_text, query_token_list)
}

fn nl_sig_has_paper_direct_revision_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_paper_direct_revision_context(query_text, query_token_list)
}

fn nl_sig_has_paper_figure_layout_review_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_paper_figure_layout_review_context(query_text, query_token_list)
}

fn nl_sig_has_paper_logic_evidence_review_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_paper_logic_evidence_review_context(query_text, query_token_list)
}

fn nl_sig_has_paper_ref_first_workflow_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_paper_ref_first_workflow_context(query_text, query_token_list)
}

fn nl_sig_has_paper_review_judgment_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_paper_review_judgment_context(query_text, query_token_list)
}

fn nl_sig_has_paper_workbench_frontdoor_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_paper_workbench_frontdoor_context(query_text, query_token_list)
}

fn nl_sig_has_paper_writing_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_paper_writing_context(query_text, query_token_list)
}

fn nl_sig_has_parallel_execution_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_parallel_execution_context(query_text, query_token_list)
}

fn nl_sig_has_pr_triage_summary_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_pr_triage_summary_context(query_text, query_token_list)
}

fn nl_sig_has_prose_naturalization_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_prose_naturalization_context(query_text, query_token_list)
}

fn nl_sig_has_rendered_visual_evidence_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_rendered_visual_evidence_context(query_text, query_token_list)
}

fn nl_sig_has_runtime_lightweighting_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_runtime_lightweighting_context(query_text, query_token_list)
}

fn nl_sig_has_scientific_figure_plotting_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_scientific_figure_plotting_context(query_text, query_token_list)
}

fn nl_sig_has_sentry_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_sentry_context(query_text, query_token_list)
}

fn nl_sig_has_skill_creator_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_skill_creator_context(query_text, query_token_list)
}

fn nl_sig_has_skill_framework_maintenance_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_skill_framework_maintenance_context(query_text, query_token_list)
}

fn nl_sig_has_skill_installer_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_skill_installer_context(query_text, query_token_list)
}

fn nl_sig_has_source_slide_format_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_source_slide_format_context(query_text, query_token_list)
}

fn nl_sig_has_systematic_debug_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_systematic_debug_context(query_text, query_token_list)
}

fn nl_sig_has_team_orchestration_context(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    has_team_orchestration_context(query_text, query_token_list)
}

fn nl_sig_is_meta_routing_task(
    _record: &SkillRecord,
    query_text: &str,
    _query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    is_meta_routing_task(query_text)
}

fn nl_sig_paper_skill_requires_context(
    record: &SkillRecord,
    _query_text: &str,
    _query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    paper_skill_requires_context(&record.slug)
}

fn nl_sig_should_defer_to_artifact_gate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    should_defer_to_artifact_gate(record, query_text, query_token_list)
}

fn nl_sig_should_prefer_design_contract_over_artifact(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    should_prefer_design_contract_over_artifact(record, query_text, query_token_list)
}

fn nl_sig_should_route_to_gh_fix_ci(
    _record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    should_route_to_gh_fix_ci(query_text, query_token_list)
}

fn nl_sig_should_suppress_non_target_artifact_gate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    _query_tokens: &HashSet<String>,
) -> bool {
    should_suppress_non_target_artifact_gate(record, query_text, query_token_list)
}

/// Sorted `when.signal` registry: single source for allowlist + evaluation.
const NL_SIGNAL_REGISTRY: &[NlSignalEntry] = &[
    NlSignalEntry {
        name: "has_beamer_slide_context",
        eval: nl_sig_has_beamer_slide_context,
    },
    NlSignalEntry {
        name: "has_bounded_subagent_context",
        eval: nl_sig_has_bounded_subagent_context,
    },
    NlSignalEntry {
        name: "has_ci_failure_context",
        eval: nl_sig_has_ci_failure_context,
    },
    NlSignalEntry {
        name: "has_copywriting_context",
        eval: nl_sig_has_copywriting_context,
    },
    NlSignalEntry {
        name: "has_design_contract_context",
        eval: nl_sig_has_design_contract_context,
    },
    NlSignalEntry {
        name: "has_design_contract_negation_context",
        eval: nl_sig_has_design_contract_negation_context,
    },
    NlSignalEntry {
        name: "has_design_output_audit_context",
        eval: nl_sig_has_design_output_audit_context,
    },
    NlSignalEntry {
        name: "has_design_reference_context",
        eval: nl_sig_has_design_reference_context,
    },
    NlSignalEntry {
        name: "has_design_workflow_protocol_context",
        eval: nl_sig_has_design_workflow_protocol_context,
    },
    NlSignalEntry {
        name: "has_diagramming_context",
        eval: nl_sig_has_diagramming_context,
    },
    NlSignalEntry {
        name: "has_github_pr_context",
        eval: nl_sig_has_github_pr_context,
    },
    NlSignalEntry {
        name: "has_paper_context",
        eval: nl_sig_has_paper_context,
    },
    NlSignalEntry {
        name: "has_paper_direct_revision_context",
        eval: nl_sig_has_paper_direct_revision_context,
    },
    NlSignalEntry {
        name: "has_paper_figure_layout_review_context",
        eval: nl_sig_has_paper_figure_layout_review_context,
    },
    NlSignalEntry {
        name: "has_paper_logic_evidence_review_context",
        eval: nl_sig_has_paper_logic_evidence_review_context,
    },
    NlSignalEntry {
        name: "has_paper_ref_first_workflow_context",
        eval: nl_sig_has_paper_ref_first_workflow_context,
    },
    NlSignalEntry {
        name: "has_paper_review_judgment_context",
        eval: nl_sig_has_paper_review_judgment_context,
    },
    NlSignalEntry {
        name: "has_paper_workbench_frontdoor_context",
        eval: nl_sig_has_paper_workbench_frontdoor_context,
    },
    NlSignalEntry {
        name: "has_paper_writing_context",
        eval: nl_sig_has_paper_writing_context,
    },
    NlSignalEntry {
        name: "has_parallel_execution_context",
        eval: nl_sig_has_parallel_execution_context,
    },
    NlSignalEntry {
        name: "has_pr_triage_summary_context",
        eval: nl_sig_has_pr_triage_summary_context,
    },
    NlSignalEntry {
        name: "has_prose_naturalization_context",
        eval: nl_sig_has_prose_naturalization_context,
    },
    NlSignalEntry {
        name: "has_rendered_visual_evidence_context",
        eval: nl_sig_has_rendered_visual_evidence_context,
    },
    NlSignalEntry {
        name: "has_runtime_lightweighting_context",
        eval: nl_sig_has_runtime_lightweighting_context,
    },
    NlSignalEntry {
        name: "has_scientific_figure_plotting_context",
        eval: nl_sig_has_scientific_figure_plotting_context,
    },
    NlSignalEntry {
        name: "has_sentry_context",
        eval: nl_sig_has_sentry_context,
    },
    NlSignalEntry {
        name: "has_skill_creator_context",
        eval: nl_sig_has_skill_creator_context,
    },
    NlSignalEntry {
        name: "has_skill_framework_maintenance_context",
        eval: nl_sig_has_skill_framework_maintenance_context,
    },
    NlSignalEntry {
        name: "has_skill_installer_context",
        eval: nl_sig_has_skill_installer_context,
    },
    NlSignalEntry {
        name: "has_source_slide_format_context",
        eval: nl_sig_has_source_slide_format_context,
    },
    NlSignalEntry {
        name: "has_systematic_debug_context",
        eval: nl_sig_has_systematic_debug_context,
    },
    NlSignalEntry {
        name: "has_team_orchestration_context",
        eval: nl_sig_has_team_orchestration_context,
    },
    NlSignalEntry {
        name: "is_meta_routing_task",
        eval: nl_sig_is_meta_routing_task,
    },
    NlSignalEntry {
        name: "paper_skill_requires_context",
        eval: nl_sig_paper_skill_requires_context,
    },
    NlSignalEntry {
        name: "should_defer_to_artifact_gate",
        eval: nl_sig_should_defer_to_artifact_gate,
    },
    NlSignalEntry {
        name: "should_prefer_design_contract_over_artifact",
        eval: nl_sig_should_prefer_design_contract_over_artifact,
    },
    NlSignalEntry {
        name: "should_route_to_gh_fix_ci",
        eval: nl_sig_should_route_to_gh_fix_ci,
    },
    NlSignalEntry {
        name: "should_suppress_non_target_artifact_gate",
        eval: nl_sig_should_suppress_non_target_artifact_gate,
    },
];

/// Sorted JSON array of every `NL_SIGNAL_REGISTRY[].name` for policy tests / CI (`router-rs framework nl-route-signal-registry-contract`).
pub(crate) fn nl_route_signal_registry_names_json() -> String {
    let mut names: Vec<&'static str> = NL_SIGNAL_REGISTRY.iter().map(|e| e.name).collect();
    names.sort_unstable();
    serde_json::to_string(&names).unwrap_or_else(|e| {
        panic!("serialize nl_route signal registry names as JSON failed: {e}");
    })
}

fn nl_registry_find(name: &str) -> Option<NlSignalEvalFn> {
    NL_SIGNAL_REGISTRY
        .binary_search_by(|entry| entry.name.cmp(name))
        .ok()
        .map(|idx| NL_SIGNAL_REGISTRY[idx].eval)
}

fn allowed_signal(name: &str) -> bool {
    nl_registry_find(name).is_some()
}

fn validate_signal(name: &str) -> Result<(), String> {
    if allowed_signal(name) {
        Ok(())
    } else {
        Err(format!(
            "unknown when.signal `{name}` (not in NL_SIGNAL_REGISTRY)"
        ))
    }
}

#[derive(Debug, Clone)]
enum WhenExpr {
    Literal(bool),
    All(Vec<WhenExpr>),
    Any(Vec<WhenExpr>),
    Not(Box<WhenExpr>),
    Signal(String),
    QueryContains(String),
    FirstTurn(bool),
}

#[derive(Debug, Clone, Default)]
struct RecordFilter {
    slug: Option<String>,
    slugs: Option<Vec<String>>,
    gate_lower: Option<String>,
}

#[derive(Debug, Clone)]
enum CompiledAction {
    Suppress { reason: String },
    Boost { delta: f64, reason: String },
}

#[derive(Debug, Clone)]
struct CompiledRule {
    record: RecordFilter,
    when: WhenExpr,
    action: CompiledAction,
}

struct CompiledNl {
    pre: Vec<CompiledRule>,
    post: Vec<CompiledRule>,
}

fn parse_record_filter(filter: Option<&Value>) -> Result<RecordFilter, String> {
    let Some(spec) = filter else {
        return Ok(RecordFilter::default());
    };
    if spec.is_null() {
        return Ok(RecordFilter::default());
    }
    let Some(obj) = spec.as_object() else {
        return Err("record: expected object or null".into());
    };
    const ALLOWED: &[&str] = &["slug", "slugs", "gate_lower"];
    for k in obj.keys() {
        if !ALLOWED.contains(&k.as_str()) {
            return Err(format!("record: unknown key `{k}`"));
        }
    }
    let slug = obj
        .get("slug")
        .map(|v| {
            v.as_str()
                .ok_or_else(|| "record.slug must be string".to_string())
                .map(str::to_string)
        })
        .transpose()?;
    let slugs = obj
        .get("slugs")
        .map(|v| -> Result<Vec<String>, String> {
            let arr = v
                .as_array()
                .ok_or_else(|| "record.slugs must be array".to_string())?;
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                let s = item
                    .as_str()
                    .ok_or_else(|| "record.slugs entries must be strings".to_string())?;
                out.push(s.to_string());
            }
            Ok::<Vec<String>, String>(out)
        })
        .transpose()?;
    let gate_lower = obj
        .get("gate_lower")
        .map(|v| {
            v.as_str()
                .ok_or_else(|| "record.gate_lower must be string".to_string())
                .map(str::to_string)
        })
        .transpose()?;
    Ok(RecordFilter {
        slug,
        slugs,
        gate_lower,
    })
}

fn parse_when(expr: &Value) -> Result<WhenExpr, String> {
    match expr {
        Value::Bool(b) => Ok(WhenExpr::Literal(*b)),
        Value::Object(map) => {
            if map.is_empty() {
                return Err(
                    "when: empty object is not allowed (use true or a single recognized key)"
                        .into(),
                );
            }
            for k in map.keys() {
                if !matches!(
                    k.as_str(),
                    "all" | "any" | "not" | "signal" | "query_contains" | "first_turn"
                ) {
                    return Err(format!("when: unknown key `{k}`"));
                }
            }
            if let Some(arr) = map.get("all") {
                if map.len() != 1 {
                    return Err("when: `all` must be the sole object key".into());
                }
                let arr = arr
                    .as_array()
                    .ok_or_else(|| "when.all must be array".to_string())?;
                let mut out = Vec::with_capacity(arr.len());
                for item in arr {
                    out.push(parse_when(item)?);
                }
                return Ok(WhenExpr::All(out));
            }
            if let Some(arr) = map.get("any") {
                if map.len() != 1 {
                    return Err("when: `any` must be the sole object key".into());
                }
                let arr = arr
                    .as_array()
                    .ok_or_else(|| "when.any must be array".to_string())?;
                let mut out = Vec::with_capacity(arr.len());
                for item in arr {
                    out.push(parse_when(item)?);
                }
                return Ok(WhenExpr::Any(out));
            }
            if let Some(inner) = map.get("not") {
                if map.len() != 1 {
                    return Err("when: `not` must be the sole object key".into());
                }
                return Ok(WhenExpr::Not(Box::new(parse_when(inner)?)));
            }
            if map.len() != 1 {
                return Err(format!(
                    "when: expected exactly one leaf key among signal/query_contains/first_turn, got {:?}",
                    map.keys().collect::<Vec<_>>()
                ));
            }
            if let Some(s) = map.get("signal").and_then(Value::as_str) {
                validate_signal(s)?;
                return Ok(WhenExpr::Signal(s.to_string()));
            }
            if let Some(s) = map.get("query_contains").and_then(Value::as_str) {
                return Ok(WhenExpr::QueryContains(s.to_string()));
            }
            if let Some(b) = map.get("first_turn").and_then(Value::as_bool) {
                return Ok(WhenExpr::FirstTurn(b));
            }
            Err("when: leaf object must be signal, query_contains, or first_turn".into())
        }
        other => Err(format!("when: expected bool or object, got {other}")),
    }
}

fn parse_action(action: &Value) -> Result<CompiledAction, String> {
    let Some(obj) = action.as_object() else {
        return Err("action: expected object".into());
    };
    for k in obj.keys() {
        if !matches!(k.as_str(), "type" | "reason" | "delta") {
            return Err(format!("action: unknown key `{k}`"));
        }
    }
    let ty = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "action.type must be string".to_string())?;
    match ty {
        "suppress" => {
            let reason = obj
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("Suppressed by NL_ROUTE_ADJUSTMENTS.")
                .to_string();
            Ok(CompiledAction::Suppress { reason })
        }
        "boost" => {
            let delta = obj.get("delta").and_then(Value::as_f64).unwrap_or(0.0);
            if !delta.is_finite() {
                return Err("action.boost delta must be a finite number".into());
            }
            let reason = obj
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("Boost from NL_ROUTE_ADJUSTMENTS.")
                .to_string();
            Ok(CompiledAction::Boost { delta, reason })
        }
        other => Err(format!("unknown action.type `{other}`")),
    }
}

fn compile_rule(rule: &Value) -> Result<CompiledRule, String> {
    let Some(obj) = rule.as_object() else {
        return Err("rule must be JSON object".into());
    };
    for k in obj.keys() {
        if !matches!(k.as_str(), "record" | "when" | "action") {
            return Err(format!("rule: unknown top-level key `{k}`"));
        }
    }
    let record = parse_record_filter(obj.get("record"))?;
    let when = match obj.get("when") {
        None => WhenExpr::Literal(true),
        Some(v) => parse_when(v)?,
    };
    let action = parse_action(
        obj.get("action")
            .ok_or_else(|| "rule.action is required".to_string())?,
    )?;
    Ok(CompiledRule {
        record,
        when,
        action,
    })
}

fn compile_rule_vec(rules: &[Value], label: &str) -> Result<Vec<CompiledRule>, String> {
    let mut out = Vec::with_capacity(rules.len());
    for (i, rule) in rules.iter().enumerate() {
        out.push(compile_rule(rule).map_err(|e| format!("{label}[{i}]: {e}"))?);
    }
    Ok(out)
}

/// Parse and validate embedded (or test) NL JSON. Used by [`compiled_nl`] and unit tests for bad fixtures.
fn compile_nl_route_adjustments(json: &str) -> Result<CompiledNl, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("NL JSON parse: {e}"))?;
    let Some(root_obj) = root.as_object() else {
        return Err("NL root must be object".into());
    };
    const ROOT_KEYS: &[&str] = &[
        "schema_version",
        "docs",
        "pre_framework_alias_rules",
        "post_framework_alias_rules",
    ];
    for k in root_obj.keys() {
        if !ROOT_KEYS.contains(&k.as_str()) {
            return Err(format!("NL root: unknown key `{k}`"));
        }
    }
    let sv = root_obj
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if sv != EXPECTED_SCHEMA {
        return Err(format!(
            "NL_ROUTE_ADJUSTMENTS schema_version mismatch: expected `{EXPECTED_SCHEMA}`, got `{sv}`"
        ));
    }
    let pre = root_obj
        .get("pre_framework_alias_rules")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let post = root_obj
        .get("post_framework_alias_rules")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    Ok(CompiledNl {
        pre: compile_rule_vec(pre, "pre_framework_alias_rules")?,
        post: compile_rule_vec(post, "post_framework_alias_rules")?,
    })
}

fn compiled_nl() -> &'static CompiledNl {
    static CELL: OnceLock<CompiledNl> = OnceLock::new();
    CELL.get_or_init(|| {
        compile_nl_route_adjustments(NL_EMBED).unwrap_or_else(|e| {
            panic!("NL_ROUTE_ADJUSTMENTS.json failed compile-time validation: {e}");
        })
    })
}

fn matches_record_filter(filter: &RecordFilter, record: &SkillRecord) -> bool {
    if let Some(s) = &filter.slug {
        if record.slug != *s {
            return false;
        }
    }
    if let Some(arr) = &filter.slugs {
        let ok = arr.iter().any(|s| s == record.slug.as_str());
        if !ok {
            return false;
        }
    }
    if let Some(g) = &filter.gate_lower {
        if record.gate_lower != *g {
            return false;
        }
    }
    true
}

fn eval_signal(
    name: &str,
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<String>,
) -> bool {
    nl_registry_find(name)
        .map(|eval| (eval)(record, query_text, query_token_list, query_tokens))
        .unwrap_or(false)
}

fn eval_when_expr(
    expr: &WhenExpr,
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<String>,
    first_turn: bool,
) -> bool {
    match expr {
        WhenExpr::Literal(b) => *b,
        WhenExpr::All(v) => v.iter().all(|sub| {
            eval_when_expr(
                sub,
                record,
                query_text,
                query_token_list,
                query_tokens,
                first_turn,
            )
        }),
        WhenExpr::Any(v) => v.iter().any(|sub| {
            eval_when_expr(
                sub,
                record,
                query_text,
                query_token_list,
                query_tokens,
                first_turn,
            )
        }),
        WhenExpr::Not(inner) => !eval_when_expr(
            inner,
            record,
            query_text,
            query_token_list,
            query_tokens,
            first_turn,
        ),
        WhenExpr::Signal(name) => {
            eval_signal(name, record, query_text, query_token_list, query_tokens)
        }
        WhenExpr::QueryContains(s) => query_text.contains(s.as_str()),
        WhenExpr::FirstTurn(b) => first_turn == *b,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_rule_list<'a>(
    rules: &[CompiledRule],
    record: &'a SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<String>,
    first_turn: bool,
    score: &mut f64,
    reasons: &mut Vec<String>,
) -> Option<RouteCandidate<'a>> {
    for rule in rules {
        if !matches_record_filter(&rule.record, record) {
            continue;
        }
        if !eval_when_expr(
            &rule.when,
            record,
            query_text,
            query_token_list,
            query_tokens,
            first_turn,
        ) {
            continue;
        }
        match &rule.action {
            CompiledAction::Suppress { reason } => {
                return Some(RouteCandidate {
                    record,
                    score: 0.0,
                    reasons: vec![reason.clone()],
                });
            }
            CompiledAction::Boost { delta, reason } => {
                *score += delta;
                reasons.push(reason.clone());
            }
        }
    }
    None
}

pub(crate) fn apply_nl_pre_framework_alias_rules<'a>(
    record: &'a SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<String>,
    first_turn: bool,
    score: &mut f64,
    reasons: &mut Vec<String>,
) -> Option<RouteCandidate<'a>> {
    apply_rule_list(
        &compiled_nl().pre,
        record,
        query_text,
        query_token_list,
        query_tokens,
        first_turn,
        score,
        reasons,
    )
}

pub(crate) fn apply_nl_post_framework_alias_rules<'a>(
    record: &'a SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<String>,
    first_turn: bool,
    score: &mut f64,
    reasons: &mut Vec<String>,
) -> Option<RouteCandidate<'a>> {
    apply_rule_list(
        &compiled_nl().post,
        record,
        query_text,
        query_token_list,
        query_tokens,
        first_turn,
        score,
        reasons,
    )
}

#[cfg(test)]
mod tests {
    use super::super::text::tokenize_route_text;
    use super::*;

    #[test]
    fn nl_signal_registry_sorted_unique() {
        assert!(
            NL_SIGNAL_REGISTRY
                .windows(2)
                .all(|pair| pair[0].name < pair[1].name),
            "NL_SIGNAL_REGISTRY must be strictly sorted by name (no duplicates)"
        );
    }

    #[test]
    fn nl_embed_compiles() {
        let _ = compiled_nl();
    }

    #[test]
    fn compile_rejects_empty_when_object() {
        let j = r#"{
            "schema_version": "nl-route-adjustments-v1",
            "pre_framework_alias_rules": [
                {"when": {}, "action": {"type": "suppress", "reason": "x"}, "record": {"slug": "a"}}
            ],
            "post_framework_alias_rules": []
        }"#;
        assert!(compile_nl_route_adjustments(j).is_err());
    }

    #[test]
    fn compile_rejects_unknown_when_key() {
        let j = r#"{
            "schema_version": "nl-route-adjustments-v1",
            "pre_framework_alias_rules": [
                {"when": {"bogus": true}, "action": {"type": "suppress", "reason": "x"}, "record": {"slug": "a"}}
            ],
            "post_framework_alias_rules": []
        }"#;
        assert!(compile_nl_route_adjustments(j).is_err());
    }

    #[test]
    fn compile_rejects_unknown_signal() {
        let j = r#"{
            "schema_version": "nl-route-adjustments-v1",
            "pre_framework_alias_rules": [
                {"when": {"signal": "not_a_real_nl_signal_xyz"}, "action": {"type": "suppress", "reason": "x"}, "record": {"slug": "a"}}
            ],
            "post_framework_alias_rules": []
        }"#;
        assert!(compile_nl_route_adjustments(j).is_err());
    }

    #[test]
    fn compile_rejects_unknown_action_type() {
        let j = r#"{
            "schema_version": "nl-route-adjustments-v1",
            "pre_framework_alias_rules": [
                {"when": true, "action": {"type": "nope"}, "record": {"slug": "a"}}
            ],
            "post_framework_alias_rules": []
        }"#;
        assert!(compile_nl_route_adjustments(j).is_err());
    }

    #[test]
    fn compile_rejects_non_finite_boost_delta() {
        let j = r#"{
            "schema_version": "nl-route-adjustments-v1",
            "pre_framework_alias_rules": [
                {"when": true, "action": {"type": "boost", "delta": 1e400}, "record": {"slug": "a"}}
            ],
            "post_framework_alias_rules": []
        }"#;
        assert!(compile_nl_route_adjustments(j).is_err());
        let j2 = r#"{
            "schema_version": "nl-route-adjustments-v1",
            "pre_framework_alias_rules": [
                {"when": true, "action": {"type": "boost", "delta": 1.5}, "record": {"slug": "a"}}
            ],
            "post_framework_alias_rules": []
        }"#;
        assert!(compile_nl_route_adjustments(j2).is_ok());
    }

    #[test]
    fn compile_rejects_unknown_record_key() {
        let j = r#"{
            "schema_version": "nl-route-adjustments-v1",
            "pre_framework_alias_rules": [
                {"when": true, "action": {"type": "suppress", "reason": "x"}, "record": {"slug": "a", "extra": 1}}
            ],
            "post_framework_alias_rules": []
        }"#;
        assert!(compile_nl_route_adjustments(j).is_err());
    }

    #[test]
    fn systematic_debug_suppressed_on_meta_routing() {
        let slug = "systematic-debugging".to_string();
        let record = SkillRecord {
            slug: slug.clone(),
            skill_path: None,
            layer: "L2".to_string(),
            owner: "owner".to_string(),
            gate: "none".to_string(),
            priority: "P1".to_string(),
            session_start: "preferred".to_string(),
            summary: String::new(),
            slug_lower: slug.to_ascii_lowercase(),
            owner_lower: "owner".to_string(),
            gate_lower: "none".to_string(),
            session_start_lower: "preferred".to_string(),
            gate_phrases: vec![],
            trigger_hints: vec![],
            name_tokens: HashSet::new(),
            keyword_tokens: HashSet::new(),
            alias_tokens: HashSet::new(),
            do_not_use_tokens: HashSet::new(),
            framework_alias_entrypoints: vec![],
            metadata_positive_triggers: vec![],
            host_platforms: vec![],
            record_kind: "skill".to_string(),
            primary_allowed: true,
            fallback_policy_mode: String::new(),
        };
        let q = "路由系统 核查 runtime 行为";
        let tokens = tokenize_route_text(q);
        let set: HashSet<String> = tokens.iter().cloned().collect();
        let mut score = 0.0f64;
        let mut reasons = vec![];
        let out = apply_nl_pre_framework_alias_rules(
            &record,
            q,
            &tokens,
            &set,
            true,
            &mut score,
            &mut reasons,
        );
        assert!(out.is_some());
        assert_eq!(out.expect("suppressed").score, 0.0);
    }
}
