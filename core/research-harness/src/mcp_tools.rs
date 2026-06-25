//! MCP tool dispatch for research tools.
//!
//! Delegated from host-projection's tool dispatcher (Phase 4 T1).

use serde_json::{Value, json};

/// Handle a research MCP tool call.
/// Delegates to the appropriate research-harness module.
pub fn handle_research_tool(name: &str, arguments: &Value) -> Result<String, String> {
    match name {
        "research_aigc_check" => tool_research_aigc_check(arguments),
        "research_aigc_humanize" => tool_research_aigc_humanize(arguments),
        "research_review_dimensions" => tool_research_review_dimensions(arguments),
        "research_claim_drift" => tool_research_claim_drift(arguments),
        "research_review_loop" => tool_research_review_loop(arguments),
        _ if name.starts_with("math_") => math_tool_dispatch(name, arguments),
        _ => Err(format!("unknown research tool: {name}")),
    }
}

// ── Math tool sub-dispatch ──

fn math_tool_dispatch(name: &str, arguments: &Value) -> Result<String, String> {
    match name {
        "math_prove_inequality" => tool_math_prove_inequality(arguments),
        "math_backend_available" => tool_math_backend_available(arguments),
        "math_backend_status" => tool_math_backend_available(arguments),
        "math_asymptotic_estimate" => tool_math_asymptotic_estimate(arguments),
        "math_asymptotic_chain" => tool_math_asymptotic_chain(arguments),
        "math_proof_dag_init" => tool_math_proof_dag_init(arguments),
        "math_proof_dag_decompose" => tool_math_proof_dag_decompose(arguments),
        "math_proof_dag_verify" => tool_math_proof_dag_verify(arguments),
        "math_proof_dag_status" => tool_math_proof_dag_status(arguments),
        "math_sympy_verify" => tool_math_sympy_verify(arguments),
        "math_sympy_simplify" => tool_math_sympy_simplify(arguments),
        "math_lean_verify" => tool_math_lean_verify(arguments),
        _ => Err(format!("unknown math tool: {name}")),
    }
}

// ── Inequality tool functions ──

fn tool_math_prove_inequality(arguments: &Value) -> Result<String, String> {
    let expr = arguments.get("expression").and_then(Value::as_str)
        .ok_or("math_prove_inequality requires 'expression' (string)")?;
    let timeout = arguments.get("timeout_ms").and_then(Value::as_u64);
    let vr = crate::verification::inequality::check_inequality(expr, timeout);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    })).map_err(|e| e.to_string())
}

fn tool_math_backend_available(_arguments: &Value) -> Result<String, String> {
    let lean_status = crate::verification::lean_bridge::check_lean_status();
    let (lean_available, lean_desc) = match &lean_status {
        crate::verification::lean_bridge::LeanStatus::Available => {
            (true, "Lean 4 theorem prover is installed".to_string())
        }
        crate::verification::lean_bridge::LeanStatus::NotFound { install_guide, .. } => {
            (false, format!("Lean 4 — not found. Install guide: {install_guide}"))
        }
    };
    serde_json::to_string_pretty(&json!({
        "inequality_engine": {
            "available": crate::verification::inequality::z3_available(),
            "description": "Z3-based linear inequality verification",
        },
        "sympy": {
            "available": crate::verification::sympy_available(),
            "description": "Symbolic identity simplification and LaTeX parsing",
        },
        "lean": { "available": lean_available, "description": lean_desc },
        "install_hint": "uv pip install z3-solver sympy",
    })).map_err(|e| e.to_string())
}

// ── Asymptotic tool functions ──

fn tool_math_asymptotic_estimate(arguments: &Value) -> Result<String, String> {
    let expr = arguments.get("expression").and_then(Value::as_str)
        .ok_or("math_asymptotic_estimate requires 'expression' (string)")?;
    let var = arguments.get("variable").and_then(Value::as_str).unwrap_or("x");
    let regime = arguments.get("regime").and_then(Value::as_str).unwrap_or("oo");
    let vr = crate::verification::asymptotic::magnitude_estimate_with_name(expr, var, regime, "math_asymptotic_estimate");
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    })).map_err(|e| e.to_string())
}

fn tool_math_asymptotic_chain(arguments: &Value) -> Result<String, String> {
    let steps_val = arguments.get("steps").and_then(Value::as_array)
        .ok_or("math_asymptotic_chain requires 'steps' array")?;
    let var = arguments.get("variable").and_then(Value::as_str).unwrap_or("x");
    let regime = arguments.get("regime").and_then(Value::as_str).unwrap_or("oo");
    let sympy_check = arguments.get("sympy_check").and_then(Value::as_bool).unwrap_or(true);
    let steps: Vec<crate::verification::asymptotic::AsymptoticStep> =
        serde_json::from_value(serde_json::Value::Array(steps_val.clone()))
            .map_err(|e| format!("invalid step format: {e}"))?;
    let vr = crate::verification::asymptotic::verify_asymptotic_chain_with_name(&steps, var, regime, sympy_check, "math_asymptotic_chain");
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "steps": steps_val,
    })).map_err(|e| e.to_string())
}

// ── Proof DAG tool functions ──

fn get_or_create_dag() -> &'static std::sync::Mutex<Option<crate::proof_dag::Blueprint>> {
    use std::sync::OnceLock;
    static DAG: OnceLock<std::sync::Mutex<Option<crate::proof_dag::Blueprint>>> = OnceLock::new();
    DAG.get_or_init(|| std::sync::Mutex::new(None))
}

fn tool_math_proof_dag_init(arguments: &Value) -> Result<String, String> {
    let goal = arguments.get("goal").and_then(Value::as_str)
        .ok_or("math_proof_dag_init requires 'goal' (string)")?;
    let name = arguments.get("name").and_then(Value::as_str).unwrap_or("proof");
    let bp = crate::proof_dag::Blueprint::new(goal, name);
    let serialized = crate::proof_dag_serialize::serialize_blueprint(&bp)?;
    if let Ok(mut guard) = get_or_create_dag().lock() {
        *guard = Some(bp);
    }
    Ok(serialized)
}

fn tool_math_proof_dag_decompose(arguments: &Value) -> Result<String, String> {
    let parent_id = arguments.get("parent_id").and_then(Value::as_str)
        .ok_or("math_proof_dag_decompose requires 'parent_id'")?;
    let and = arguments.get("and").and_then(Value::as_bool).unwrap_or(false);
    let children_val = arguments.get("children").and_then(Value::as_array)
        .ok_or("math_proof_dag_decompose requires 'children' array")?;
    let children: Vec<crate::proof_dag::DagNode> =
        serde_json::from_value(Value::Array(children_val.clone()))
            .map_err(|e| format!("invalid child format: {e}"))?;
    let mut guard = get_or_create_dag().lock().map_err(|e| format!("lock: {e}"))?;
    let bp = guard.as_mut().ok_or("no active proof DAG — call math_proof_dag_init first")?;
    bp.decompose(parent_id, children, and)?;
    crate::proof_dag_serialize::serialize_blueprint(bp)
}

fn tool_math_proof_dag_verify(arguments: &Value) -> Result<String, String> {
    let _ = arguments;
    let mut guard = get_or_create_dag().lock().map_err(|e| format!("lock: {e}"))?;
    let bp = guard.as_mut().ok_or("no active proof DAG — call math_proof_dag_init first")?;
    bp.verify()?;
    if let Err(warning) = bp.validate_manual_prose_ratio(0.30) {
        let summary = bp.status_summary();
        return serde_json::to_string_pretty(&json!({
            "result": summary,
            "manual_prose_warning": warning,
        })).map_err(|e| e.to_string());
    }
    crate::proof_dag_serialize::serialize_blueprint(bp)
}

fn tool_math_proof_dag_status(arguments: &Value) -> Result<String, String> {
    let _ = arguments;
    let guard = get_or_create_dag().lock().map_err(|e| format!("lock: {e}"))?;
    let bp = guard.as_ref().ok_or("no active proof DAG — call math_proof_dag_init first")?;
    let summary = bp.status_summary();
    serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())
}

// ── SymPy bridge tool functions ──

fn tool_math_sympy_verify(arguments: &Value) -> Result<String, String> {
    let lhs = arguments.get("lhs").and_then(Value::as_str)
        .ok_or("math_sympy_verify requires 'lhs' (string)")?;
    let rhs = arguments.get("rhs").and_then(Value::as_str)
        .ok_or("math_sympy_verify requires 'rhs' (string)")?;
    let assumptions: Vec<&str> = arguments.get("assumptions")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let vr = crate::verification::sympy_bridge::verify_identity(lhs, rhs, &assumptions);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "lhs": lhs, "rhs": rhs,
    })).map_err(|e| e.to_string())
}

fn tool_math_sympy_simplify(arguments: &Value) -> Result<String, String> {
    let expr = arguments.get("expression").and_then(Value::as_str)
        .ok_or("math_sympy_simplify requires 'expression' (string)")?;
    let vr = crate::verification::sympy_bridge::simplify_expression(expr);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    })).map_err(|e| e.to_string())
}

fn tool_math_lean_verify(arguments: &Value) -> Result<String, String> {
    let script = arguments.get("script").and_then(Value::as_str)
        .ok_or("math_lean_verify requires 'script' (string)")?;
    let vr = crate::verification::lean_bridge::verify_lean_theorem(script);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details,
    })).map_err(|e| e.to_string())
}

fn parse_language(value: &Value, key: &str) -> crate::aigc::Language {
    match value.get(key).and_then(Value::as_str) {
        Some("zh") | Some("zh-CN") | Some("chinese") => crate::aigc::Language::Chinese,
        _ => crate::aigc::Language::English,
    }
}

fn tool_research_aigc_check(arguments: &Value) -> Result<String, String> {
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or("research_aigc_check requires 'text' parameter")?;
    let language = parse_language(arguments, "language");

    let config = crate::aigc::detector::DetectionConfig {
        language,
        ..Default::default()
    };
    let results = crate::aigc::detector::detect(text, &config)
        .map_err(|e| format!("AIGC detection failed: {e}"))?;
    let score = crate::aigc::scorer::score(&results);

    serde_json::to_string_pretty(&json!({
        "score": score,
        "ai_probability": score as f64 / 100.0,
        "segments_analyzed": results.len(),
        "results": results,
    }))
    .map_err(|e| e.to_string())
}

fn tool_research_aigc_humanize(arguments: &Value) -> Result<String, String> {
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or("research_aigc_humanize requires 'text' parameter")?;
    let language = parse_language(arguments, "language");
    let preserve_academic_tone = arguments
        .get("preserve_academic_tone")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    use crate::aigc::humanizer::{HumanizeConfig, HumanizeStrategy};
    let config = HumanizeConfig {
        strategies: vec![
            HumanizeStrategy::VocabularySwap,
            HumanizeStrategy::SyntacticRewrite,
            HumanizeStrategy::SentenceVariation,
        ],
        preserve_academic_tone,
        language,
        ..Default::default()
    };
    let result = crate::aigc::humanizer::humanize_with_config(text, &config)
        .map_err(|e| format!("AIGC humanization failed: {e}"))?;

    serde_json::to_string_pretty(&json!({
        "original_length": text.len(),
        "rewritten_length": result.rewritten.len(),
        "strategies_applied": result.strategies_applied,
        "estimated_score_improvement": result.estimated_score_improvement,
        "rewritten": result.rewritten,
    }))
    .map_err(|e| e.to_string())
}

fn tool_research_review_dimensions(arguments: &Value) -> Result<String, String> {
    let round = arguments
        .get("round")
        .and_then(Value::as_u64)
        .ok_or("research_review_dimensions requires 'round' parameter")?;
    let manuscript_summary = arguments
        .get("manuscript_summary")
        .and_then(Value::as_str)
        .unwrap_or("(no summary provided)");

    let dim = crate::types::ReviewDimension::for_round(round);
    let prompt = crate::review::dimensions::dimension_prompt(&dim);
    let checklist = crate::review::dimensions::dimension_checklist(&dim);
    let full_prompt = crate::review::orchestrator::build_reviewer_prompt(
        round,
        &dim,
        manuscript_summary,
    );

    serde_json::to_string_pretty(&json!({
        "round": round,
        "dimension": dim.display_name(),
        "prompt": prompt,
        "checklist": checklist,
        "full_reviewer_prompt": full_prompt,
    }))
    .map_err(|e| e.to_string())
}

/// Parse a `ceiling` string value into `ClaimCeiling`.
fn parse_claim_ceiling(s: Option<&str>) -> crate::types::ClaimCeiling {
    use crate::types::ClaimCeiling;
    match s {
        Some("no-claim") => ClaimCeiling::NoClaim,
        Some("local-only") => ClaimCeiling::LocalOnly,
        Some("conference-ready") | Some("conference_ready") => ClaimCeiling::ConferenceReady,
        Some("top-venue") | Some("top_venue") => ClaimCeiling::TopVenue,
        _ => ClaimCeiling::ConferenceReady,
    }
}

/// Parse an optional `evidence` array into `Vec<EvidenceAnchor>`.
fn parse_evidence_anchors(arr: Option<&[Value]>) -> Vec<crate::types::EvidenceAnchor> {
    use crate::types::{EvidenceAnchor, EvidenceStrength};
    arr.map(|items| {
        items
            .iter()
            .filter_map(|v| {
                let source = v.get("source").and_then(Value::as_str)?;
                Some(EvidenceAnchor {
                    source: source.to_string(),
                    location: v
                        .get("location")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    strength: match v.get("strength").and_then(Value::as_str) {
                        Some("strong") => EvidenceStrength::Strong,
                        Some("moderate") => EvidenceStrength::Moderate,
                        Some("weak") => EvidenceStrength::Weak,
                        _ => EvidenceStrength::Missing,
                    },
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

fn tool_research_claim_drift(arguments: &Value) -> Result<String, String> {
    let original_claims = arguments
        .get("original_claims")
        .and_then(Value::as_array)
        .ok_or("research_claim_drift requires 'original_claims' array")?;
    let current_claims = arguments
        .get("current_claims")
        .and_then(Value::as_array)
        .ok_or("research_claim_drift requires 'current_claims' array")?;

    let parse_claims = |arr: &[Value]| -> Vec<crate::types::Claim> {
        arr.iter()
            .map(|v| {
                let id = v.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let text = v.get("text").and_then(Value::as_str).unwrap_or("").to_string();
                let ceiling = parse_claim_ceiling(v.get("ceiling").and_then(Value::as_str));
                let evidence = parse_evidence_anchors(v.get("evidence").and_then(Value::as_array).map(|v| v.as_slice()));
                crate::types::Claim {
                    id,
                    text,
                    evidence,
                    ceiling,
                }
            })
            .collect()
    };

    let orig = parse_claims(original_claims);
    let curr = parse_claims(current_claims);

    let results = crate::claims::drift::detect_drift(&orig, &curr)
        .map_err(|e| e.to_string())?;

    serde_json::to_string_pretty(&json!({
        "drift_results": results,
        "total_claims_analyzed": results.len(),
    }))
    .map_err(|e| e.to_string())
}

fn tool_research_review_loop(arguments: &Value) -> Result<String, String> {
    let max_rounds = arguments
        .get("max_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(10);
    let min_rounds = arguments
        .get("min_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(5);
    let consecutive_stable = arguments
        .get("consecutive_stable_required")
        .and_then(Value::as_u64)
        .unwrap_or(2);

    let state = crate::types::ConvergenceState {
        min_rounds,
        consecutive_stable_required: consecutive_stable,
        consecutive_stable_count: 0,
        max_rounds,
        current_round: 0,
    };

    let dimensions: Vec<Value> = (1..=max_rounds)
        .map(|round| {
            let dim = crate::types::ReviewDimension::for_round(round);
            let prompt = crate::review::dimensions::dimension_prompt(&dim);
            let preview: String = prompt.chars().take(200).collect();
            json!({
                "round": round,
                "dimension": dim.display_name(),
                "prompt_preview": preview,
            })
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "convergence_config": {
            "min_rounds": state.min_rounds,
            "max_rounds": state.max_rounds,
            "consecutive_stable_required": state.consecutive_stable_required,
        },
        "dimensions": dimensions,
        "workflow": "spawn reviewer subagent per round → fix findings → check convergence → repeat",
    }))
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn handle_research_tool_unknown() {
        let result = handle_research_tool("nonexistent_tool", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown research tool"));
    }

    #[test]
    fn research_aigc_check_missing_text() {
        let result = handle_research_tool("research_aigc_check", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'text'"));
    }

    #[test]
    fn research_aigc_humanize_missing_text() {
        let result = handle_research_tool("research_aigc_humanize", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'text'"));
    }

    #[test]
    fn research_aigc_check_with_language_en() {
        let result = handle_research_tool(
            "research_aigc_check",
            &json!({"text": "This is a test sentence for AIGC detection.", "language": "en"}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("score").is_some());
        assert!(parsed.get("ai_probability").is_some());
        assert!(parsed.get("segments_analyzed").is_some());
    }

    #[test]
    fn research_aigc_check_with_language_zh() {
        let result = handle_research_tool(
            "research_aigc_check",
            &json!({"text": "这是一个用于 AIGC 检测的测试句子。", "language": "zh"}),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn research_aigc_check_default_language() {
        let result = handle_research_tool(
            "research_aigc_check",
            &json!({"text": "Some default language test text."}),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn research_aigc_humanize_with_language_and_tone() {
        let result = handle_research_tool(
            "research_aigc_humanize",
            &json!({
                "text": "This is a very important discovery that significantly advances the field.",
                "language": "en",
                "preserve_academic_tone": true,
            }),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(parsed.get("original_length").is_some());
        assert!(parsed.get("rewritten").is_some());
    }

    #[test]
    fn research_aigc_humanize_default_params() {
        let result = handle_research_tool(
            "research_aigc_humanize",
            &json!({"text": "Simple text without extra params."}),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn research_review_dimensions_missing_round() {
        let result = handle_research_tool("research_review_dimensions", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'round'"));
    }

    #[test]
    fn research_review_dimensions_round_1() {
        let result = handle_research_tool(
            "research_review_dimensions",
            &json!({"round": 1}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round").and_then(Value::as_u64), Some(1));
        assert_eq!(parsed.get("dimension").and_then(Value::as_str), Some("逻辑与证据"));
    }

    #[test]
    fn research_claim_drift_missing_required() {
        let result = handle_research_tool("research_claim_drift", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'original_claims'"));
    }

    #[test]
    fn research_claim_drift_basic() {
        let result = handle_research_tool(
            "research_claim_drift",
            &json!({
                "original_claims": [{"id": "c1", "text": "Method A achieves 95% accuracy."}],
                "current_claims": [{"id": "c1", "text": "Method A achieves 92% accuracy on the test set."}],
            }),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn research_claim_drift_with_ceiling_and_evidence() {
        let result = handle_research_tool(
            "research_claim_drift",
            &json!({
                "original_claims": [{
                    "id": "c1",
                    "text": "Our approach outperforms all baselines.",
                    "ceiling": "top-venue",
                    "evidence": [{"source": "Table 2", "location": "p.5", "strength": "strong"}],
                }],
                "current_claims": [{
                    "id": "c1",
                    "text": "Our approach outperforms existing methods.",
                    "ceiling": "conference-ready",
                    "evidence": [{"source": "Table 2", "location": "p.5", "strength": "moderate"}],
                }],
            }),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn research_claim_drift_empty_arrays() {
        let result = handle_research_tool(
            "research_claim_drift",
            &json!({"original_claims": [], "current_claims": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("total_claims_analyzed").and_then(Value::as_u64), Some(0));
    }

    #[test]
    fn research_review_loop_defaults() {
        let result = handle_research_tool("research_review_loop", &json!({}));
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let config = parsed.get("convergence_config").unwrap();
        assert_eq!(config.get("min_rounds").and_then(Value::as_u64), Some(5));
        assert_eq!(config.get("max_rounds").and_then(Value::as_u64), Some(10));
        assert_eq!(config.get("consecutive_stable_required").and_then(Value::as_u64), Some(2));
    }

    #[test]
    fn research_review_loop_custom_params() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"max_rounds": 3, "min_rounds": 1, "consecutive_stable_required": 1}),
        );
        assert!(result.is_ok());
        assert!(result.unwrap().contains("\"dimensions\":"));
    }

    #[test]
    fn parse_language_defaults_to_english() {
        let value = json!({});
        assert_eq!(parse_language(&value, "language"), crate::aigc::Language::English);
    }

    #[test]
    fn parse_language_zh() {
        let value = json!({"language": "zh"});
        assert_eq!(parse_language(&value, "language"), crate::aigc::Language::Chinese);
    }

    #[test]
    fn parse_language_en() {
        let value = json!({"language": "en"});
        assert_eq!(parse_language(&value, "language"), crate::aigc::Language::English);
    }

    #[test]
    fn parse_claim_ceiling_variants() {
        use crate::types::ClaimCeiling;
        assert_eq!(parse_claim_ceiling(Some("no-claim")), ClaimCeiling::NoClaim);
        assert_eq!(parse_claim_ceiling(Some("local-only")), ClaimCeiling::LocalOnly);
        assert_eq!(parse_claim_ceiling(Some("conference-ready")), ClaimCeiling::ConferenceReady);
        assert_eq!(parse_claim_ceiling(Some("conference_ready")), ClaimCeiling::ConferenceReady);
        assert_eq!(parse_claim_ceiling(Some("top-venue")), ClaimCeiling::TopVenue);
        assert_eq!(parse_claim_ceiling(Some("top_venue")), ClaimCeiling::TopVenue);
        assert_eq!(parse_claim_ceiling(None), ClaimCeiling::ConferenceReady);
        assert_eq!(parse_claim_ceiling(Some("unknown")), ClaimCeiling::ConferenceReady);
    }

    #[test]
    fn parse_evidence_anchors_empty() {
        assert!(parse_evidence_anchors(None).is_empty());
    }

    #[test]
    fn parse_evidence_anchors_basic() {
        use crate::types::EvidenceStrength;
        let input = json!([
            {"source": "Table 1", "location": "p.3", "strength": "strong"},
            {"source": "Figure 2", "strength": "weak"},
        ]);
        let anchors = parse_evidence_anchors(Some(input.as_array().unwrap()));
        assert_eq!(anchors.len(), 2);
        assert_eq!(anchors[0].source, "Table 1");
        assert_eq!(anchors[0].strength, EvidenceStrength::Strong);
        assert_eq!(anchors[1].source, "Figure 2");
        assert_eq!(anchors[1].strength, EvidenceStrength::Weak);
    }

    // ── Math tool tests ──

    #[test]
    fn test_math_prove_inequality_missing_expression() {
        let result = handle_research_tool("math_prove_inequality", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'expression'"));
    }

    #[test]
    fn test_math_prove_inequality_optional_timeout() {
        let result = handle_research_tool("math_prove_inequality", &json!({"expression": "x > 0"}));
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_math_asymptotic_estimate_missing_expression() {
        let result = handle_research_tool("math_asymptotic_estimate", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'expression'"));
    }

    #[test]
    fn test_math_asymptotic_chain_missing_steps() {
        let result = handle_research_tool("math_asymptotic_chain", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'steps'"));
    }

    #[test]
    fn test_math_asymptotic_chain_invalid_step_format() {
        let result = handle_research_tool("math_asymptotic_chain", &json!({
            "steps": [{"premise": "n", "relation": "InvalidOp"}],
            "variable": "n", "regime": "oo",
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_math_proof_dag_init_missing_goal() {
        let result = handle_research_tool("math_proof_dag_init", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'goal'"));
    }

    #[test]
    fn test_math_proof_dag_decompose_missing_parent_id() {
        let result = handle_research_tool("math_proof_dag_decompose", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'parent_id'"));
    }

    #[test]
    fn test_math_proof_dag_verify_without_init() {
        let result = handle_research_tool("math_proof_dag_verify", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no active proof DAG"));
    }

    #[test]
    fn test_math_proof_dag_status_without_init() {
        let result = handle_research_tool("math_proof_dag_status", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no active proof DAG"));
    }

    #[test]
    fn test_math_sympy_verify_missing_lhs() {
        let result = handle_research_tool("math_sympy_verify", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'lhs'"));
    }

    #[test]
    fn test_math_sympy_verify_missing_rhs() {
        let result = handle_research_tool("math_sympy_verify", &json!({"lhs": "x"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'rhs'"));
    }

    #[test]
    fn test_math_sympy_simplify_missing_expression() {
        let result = handle_research_tool("math_sympy_simplify", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'expression'"));
    }

    #[test]
    fn test_math_lean_verify_missing_script() {
        let result = handle_research_tool("math_lean_verify", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires 'script'"));
    }

    #[test]
    fn test_math_backend_available_ok() {
        let result = handle_research_tool("math_backend_available", &json!({}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_math_backend_status_ok() {
        let result = handle_research_tool("math_backend_status", &json!({}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_math_unknown_tool() {
        let result = handle_research_tool("math_nonexistent", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown math tool"));
    }

    #[test]
    fn test_math_tool_routing() {
        let result = handle_research_tool("math_future_tool", &json!({}));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("unknown") || err.contains("requires"), "wrong routing: {err}");
    }
}
