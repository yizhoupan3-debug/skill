//! Data-driven NL hot-route suppress/boost rules from embedded
//! [`NL_ROUTE_ADJUSTMENTS.json`](../../../configs/framework/NL_ROUTE_ADJUSTMENTS.json).

use super::signal_cache::cached_signal;
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

type NlSignalEvalFn = fn(&SkillRecord, &str, &[String], &HashSet<&str>) -> bool;

#[derive(Debug, Clone, Copy)]
struct NlSignalEntry {
    name: &'static str,
    eval: NlSignalEvalFn,
}

// ---------------------------------------------------------------------------
// Macro-driven signal adapter generation.
//
// Four adapter patterns cover all underlying signal signatures:
//   qt_ql     – fn(query_text, query_token_list)          (most has_* signals)
//   qt        – fn(query_text)                            (is_meta_routing_task)
//   slug      – fn(&str) via record.slug                  (paper_skill_requires_context)
//   rec_qt_ql – fn(record, query_text, query_token_list)  (should_defer/…/suppress)
// ---------------------------------------------------------------------------
macro_rules! nl_signals {
    // --- internal adapter rules (one per signature family) ---

    (@adapter qt_ql: $wrapper:ident, $inner:ident) => {
        fn $wrapper(
            _record: &SkillRecord,
            query_text: &str,
            query_token_list: &[String],
            _query_tokens: &HashSet<&str>,
        ) -> bool {
            cached_signal(stringify!($inner), query_text, query_token_list, || {
                $inner(query_text, query_token_list)
            })
        }
    };
    (@adapter qt: $wrapper:ident, $inner:ident) => {
        fn $wrapper(
            _record: &SkillRecord,
            query_text: &str,
            _query_token_list: &[String],
            _query_tokens: &HashSet<&str>,
        ) -> bool {
            $inner(query_text)
        }
    };
    (@adapter slug: $wrapper:ident, $inner:ident) => {
        fn $wrapper(
            record: &SkillRecord,
            _query_text: &str,
            _query_token_list: &[String],
            _query_tokens: &HashSet<&str>,
        ) -> bool {
            $inner(&record.slug)
        }
    };
    (@adapter rec_qt_ql: $wrapper:ident, $inner:ident) => {
        fn $wrapper(
            record: &SkillRecord,
            query_text: &str,
            query_token_list: &[String],
            _query_tokens: &HashSet<&str>,
        ) -> bool {
            $inner(record, query_text, query_token_list)
        }
    };

    // --- main entry: generate adapters + sorted registry ---

    ( $( $name:expr => $kind:ident : $wrapper:ident => $inner:ident ),+ $(,)? ) => {
        $(
            nl_signals!(@adapter $kind: $wrapper, $inner);
        )+

        /// Sorted `when.signal` registry: single source for allowlist + evaluation.
        const NL_SIGNAL_REGISTRY: &[NlSignalEntry] = &[
            $( NlSignalEntry { name: $name, eval: $wrapper }, )+
        ];
    };
}

nl_signals! {
    "has_beamer_slide_context"                     => qt_ql: nl_sig_has_beamer_slide_context                     => has_beamer_slide_context,
    "has_bounded_subagent_context"                 => qt_ql: nl_sig_has_bounded_subagent_context                 => has_bounded_subagent_context,
    "has_ci_failure_context"                       => qt_ql: nl_sig_has_ci_failure_context                       => has_ci_failure_context,
    "has_copywriting_context"                      => qt_ql: nl_sig_has_copywriting_context                      => has_copywriting_context,
    "has_design_contract_context"                  => qt_ql: nl_sig_has_design_contract_context                  => has_design_contract_context,
    "has_design_contract_negation_context"         => qt_ql: nl_sig_has_design_contract_negation_context         => has_design_contract_negation_context,
    "has_design_output_audit_context"              => qt_ql: nl_sig_has_design_output_audit_context              => has_design_output_audit_context,
    "has_design_reference_context"                 => qt_ql: nl_sig_has_design_reference_context                 => has_design_reference_context,
    "has_design_workflow_protocol_context"         => qt_ql: nl_sig_has_design_workflow_protocol_context         => has_design_workflow_protocol_context,
    "has_diagramming_context"                      => qt_ql: nl_sig_has_diagramming_context                      => has_diagramming_context,
    "has_github_pr_context"                        => qt_ql: nl_sig_has_github_pr_context                        => has_github_pr_context,
    "has_math_review_context"                      => qt_ql: nl_sig_has_math_review_context                      => has_math_review_context,
    "has_mcp_tool_invocation_intent"               => qt: nl_sig_has_mcp_tool_invocation_intent               => has_mcp_tool_invocation_intent,
    "has_paper_context"                            => qt_ql: nl_sig_has_paper_context                            => has_paper_context,
    "has_paper_direct_revision_context"            => qt_ql: nl_sig_has_paper_direct_revision_context            => has_paper_direct_revision_context,
    "has_paper_figure_layout_review_context"       => qt_ql: nl_sig_has_paper_figure_layout_review_context       => has_paper_figure_layout_review_context,
    "has_paper_logic_evidence_review_context"      => qt_ql: nl_sig_has_paper_logic_evidence_review_context      => has_paper_logic_evidence_review_context,
    "has_paper_prose_edit_context"                 => qt_ql: nl_sig_has_paper_prose_edit_context                 => has_paper_prose_edit_context,
    "has_paper_prose_negation_context"             => qt_ql: nl_sig_has_paper_prose_negation_context             => has_paper_prose_negation_context,
    "has_paper_ref_first_workflow_context"         => qt_ql: nl_sig_has_paper_ref_first_workflow_context         => has_paper_ref_first_workflow_context,
    "has_paper_review_judgment_context"            => qt_ql: nl_sig_has_paper_review_judgment_context            => has_paper_review_judgment_context,
    "has_paper_workbench_frontdoor_context"        => qt_ql: nl_sig_has_paper_workbench_frontdoor_context        => has_paper_workbench_frontdoor_context,
    "has_paper_writing_context"                    => qt_ql: nl_sig_has_paper_writing_context                    => has_paper_writing_context,
    "has_parallel_execution_context"               => qt_ql: nl_sig_has_parallel_execution_context               => has_parallel_execution_context,
    "has_pr_triage_summary_context"                => qt_ql: nl_sig_has_pr_triage_summary_context                => has_pr_triage_summary_context,
    "has_prose_naturalization_context"             => qt_ql: nl_sig_has_prose_naturalization_context             => has_prose_naturalization_context,
    "has_rendered_visual_evidence_context"         => qt_ql: nl_sig_has_rendered_visual_evidence_context         => has_rendered_visual_evidence_context,
    "has_research_context"                          => qt_ql: nl_sig_has_research_context                          => has_research_context,
    "has_runtime_lightweighting_context"           => qt_ql: nl_sig_has_runtime_lightweighting_context           => has_runtime_lightweighting_context,
    "has_scientific_figure_plotting_context"       => qt_ql: nl_sig_has_scientific_figure_plotting_context       => has_scientific_figure_plotting_context,
    "has_sentry_context"                           => qt_ql: nl_sig_has_sentry_context                           => has_sentry_context,
    "has_skill_creator_context"                    => qt_ql: nl_sig_has_skill_creator_context                    => has_skill_creator_context,
    "has_skill_framework_maintenance_context"      => qt_ql: nl_sig_has_skill_framework_maintenance_context      => has_skill_framework_maintenance_context,
    "has_skill_installer_context"                  => qt_ql: nl_sig_has_skill_installer_context                  => has_skill_installer_context,
    "has_source_slide_format_context"              => qt_ql: nl_sig_has_source_slide_format_context              => has_source_slide_format_context,
    "has_systematic_debug_context"                 => qt_ql: nl_sig_has_systematic_debug_context                 => has_systematic_debug_context,
    "has_workflow_orchestration_context"           => qt_ql: nl_sig_has_workflow_orchestration_context           => has_workflow_orchestration_context,
    "is_meta_routing_task"                         => qt:    nl_sig_is_meta_routing_task                         => is_meta_routing_task,
    "paper_skill_requires_context"                 => slug:  nl_sig_paper_skill_requires_context                 => paper_skill_requires_context,
    "should_defer_to_artifact_gate"                => rec_qt_ql: nl_sig_should_defer_to_artifact_gate            => should_defer_to_artifact_gate,
    "should_prefer_design_contract_over_artifact"  => rec_qt_ql: nl_sig_should_prefer_design_contract_over_artifact => should_prefer_design_contract_over_artifact,
    "should_route_to_gh_fix_ci"                    => qt_ql: nl_sig_should_route_to_gh_fix_ci                    => should_route_to_gh_fix_ci,
    "should_suppress_non_target_artifact_gate"     => rec_qt_ql: nl_sig_should_suppress_non_target_artifact_gate => should_suppress_non_target_artifact_gate,
}

/// Returns the schema version constant embedded at compile time for
/// `NL_ROUTE_ADJUSTMENTS.json`. Used by integration tests to verify
/// the embedded JSON matches the disk file.
pub fn embedded_schema_version() -> &'static str {
    EXPECTED_SCHEMA
}

/// Sorted JSON array of every `NL_SIGNAL_REGISTRY[].name` for policy tests / CI (`router-rs framework nl-route-signal-registry-contract`).
pub fn nl_route_signal_registry_names_json() -> String {
    let mut names: Vec<&'static str> = NL_SIGNAL_REGISTRY.iter().map(|e| e.name).collect();
    names.sort_unstable();
    serde_json::to_string(&names)
        .unwrap_or_else(|_| panic!("Vec<&str> serialization is infallible"))
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
    visual_evidence_markers: Vec<String>,
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
        "visual_evidence_markers",
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
    let visual_evidence_markers = root_obj
        .get("visual_evidence_markers")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(CompiledNl {
        pre: compile_rule_vec(pre, "pre_framework_alias_rules")?,
        post: compile_rule_vec(post, "post_framework_alias_rules")?,
        visual_evidence_markers,
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

/// Returns the visual evidence markers loaded from `NL_ROUTE_ADJUSTMENTS.json`.
/// Cached via `OnceLock` so the config file is parsed at most once.
pub fn visual_evidence_markers() -> &'static [String] {
    &compiled_nl().visual_evidence_markers
}

fn matches_record_filter(filter: &RecordFilter, record: &SkillRecord) -> bool {
    if let Some(s) = &filter.slug
        && record.slug != *s
    {
        return false;
    }
    if let Some(arr) = &filter.slugs {
        let ok = arr.iter().any(|s| s == record.slug.as_str());
        if !ok {
            return false;
        }
    }
    if let Some(g) = &filter.gate_lower
        && record.gate_lower != *g
    {
        return false;
    }
    true
}

fn eval_signal(
    name: &str,
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<&str>,
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
    query_tokens: &HashSet<&str>,
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

/// Maximum cumulative NL boost per apply_rule_list invocation.
/// Pre- and post-framework-alias phases each call apply_rule_list independently,
/// so a single skill may receive up to MAX_NL_BOOST_ACCUMULATION × 2 (90 pre + 90 post).
/// Prevents multiple boost rules within one phase from stacking to unbeatable 100+ scores.
/// Suppress rules are not affected by this cap.
///
/// **Cross-reference**: If [`super::scoring_config::ScoringWeights`] weight values change
/// significantly (e.g. max score doubles from ~100 to ~200), this constant must be
/// re-evaluated to prevent NL adjustments from dominating or being capped too early.
const MAX_NL_BOOST_ACCUMULATION: f64 = 150.0;

fn apply_rule_list<'a>(
    rules: &[CompiledRule],
    record: &'a SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<&str>,
    first_turn: bool,
    score: &mut f64,
    reasons: &mut Vec<String>,
) -> Option<RouteCandidate<'a>> {
    let mut nl_boost_accumulated = 0.0f64;
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
                // Suppress always takes effect regardless of boost cap.
                return Some(RouteCandidate {
                    record,
                    score: 0.0,
                    reasons: vec![reason.clone()],
                    matched_token_count: 0,
                });
            }
            CompiledAction::Boost { delta, reason } => {
                if nl_boost_accumulated >= MAX_NL_BOOST_ACCUMULATION {
                    continue;
                }
                let effective = if nl_boost_accumulated + *delta > MAX_NL_BOOST_ACCUMULATION {
                    MAX_NL_BOOST_ACCUMULATION - nl_boost_accumulated
                } else {
                    *delta
                };
                nl_boost_accumulated += effective;
                *score += effective;
                reasons.push(reason.clone());
            }
        }
    }
    None
}

pub fn apply_nl_pre_framework_alias_rules<'a>(
    record: &'a SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<&str>,
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

pub fn apply_nl_post_framework_alias_rules<'a>(
    record: &'a SkillRecord,
    query_text: &str,
    query_token_list: &[String],
    query_tokens: &HashSet<&str>,
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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

    /// Validate NL boost cap consistency against `MAX_NL_BOOST_ACCUMULATION`.
    ///
    /// **Hard checks** (panic):
    /// - No single boost delta exceeds the cap (otherwise the cap in
    ///   `apply_rule_list` would silently truncate the intended boost).
    ///
    /// **Soft warnings** (printed, non-blocking):
    /// - Cumulative deltas per slug per phase exceeding the cap, indicating
    ///   that some rules in the phase will be runtime-clipped.  Config authors
    ///   should review rule ordering when cumulative deltas are high.
    ///
    /// **Cross-reference**: If [`ScoringWeights`] weight values change
    /// significantly (e.g. max possible base score doubles), re-evaluate
    /// this constant (see doc comment on `MAX_NL_BOOST_ACCUMULATION`).
    #[test]
    fn nl_boost_cap_cross_reference() {
        let nl = compiled_nl();
        for (phase_name, rules) in [("pre", &nl.pre), ("post", &nl.post)] {
            let mut slug_totals: std::collections::HashMap<String, f64> =
                std::collections::HashMap::new();
            for rule in rules {
                if let CompiledAction::Boost { delta, .. } = &rule.action {
                    assert!(
                        *delta <= MAX_NL_BOOST_ACCUMULATION,
                        "{phase_name} phase rule for {:?} has delta {delta} > MAX_NL_BOOST_ACCUMULATION ({MAX_NL_BOOST_ACCUMULATION})",
                        rule.record.slug,
                    );
                    let key = rule.record.slug.clone().unwrap_or_else(|| "*".to_string());
                    *slug_totals.entry(key).or_insert(0.0) += delta;
                }
            }
            for (slug, total) in &slug_totals {
                if *total > MAX_NL_BOOST_ACCUMULATION {
                    tracing::info!(phase = %phase_name, slug = %slug, total = %total, cap = %MAX_NL_BOOST_ACCUMULATION, "cumulative boost exceeds cap (runtime clips this)");
                }
            }
        }
    }
}
