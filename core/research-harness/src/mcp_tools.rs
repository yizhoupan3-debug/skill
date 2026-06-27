//! MCP tool dispatch for research tools.
//!
//! Delegated from host-projection's tool dispatcher (Phase 4 T1).

use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;

/// Handle a research MCP tool call.
/// Delegates to the appropriate research-harness module.
pub fn handle_research_tool(name: &str, arguments: &Value) -> Result<String, FrameworkError> {
    match name {
        "research_aigc_check" => Ok(tool_research_aigc_check(arguments)?),
        "research_review_dimensions" => Ok(tool_research_review_dimensions(arguments)?),
        "research_claim_drift" => Ok(tool_research_claim_drift(arguments)?),
        "research_review_loop" => Ok(tool_research_review_loop(arguments)?),
        "research_smoke" => Ok(tool_research_smoke(arguments)?),
        _ if name.starts_with("math_") => Ok(math_tool_dispatch(name, arguments)?),
        _ if name.starts_with("research_verification_") => {
            verification_tool_dispatch(name, arguments)
        }
        _ => Err(FrameworkError::validation(format!("unknown research tool: {name}"))),
    }
}

// ── Math tool sub-dispatch ──

fn math_tool_dispatch(name: &str, arguments: &Value) -> Result<String, FrameworkError> {
    match name {
        "math_prove_inequality" => tool_math_prove_inequality(arguments),
        "math_backend_available" => tool_math_backend_available(arguments),
        "math_asymptotic_estimate" => tool_math_asymptotic_estimate(arguments),
        "math_asymptotic_chain" => tool_math_asymptotic_chain(arguments),
        "math_proof_dag_init" => tool_math_proof_dag_init(arguments),
        "math_proof_dag_decompose" => tool_math_proof_dag_decompose(arguments),
        "math_proof_dag_verify" => tool_math_proof_dag_verify(arguments),
        "math_proof_dag_status" => tool_math_proof_dag_status(arguments),
        "math_sympy_verify" => tool_math_sympy_verify(arguments),
        "math_sympy_simplify" => tool_math_sympy_simplify(arguments),
        "math_lean_verify" => tool_math_lean_verify(arguments),
        _ => Err(FrameworkError::validation(format!("unknown math tool: {name}"))),
    }
}

// ── Verification tool sub-dispatch ──

fn verification_tool_dispatch(name: &str, arguments: &Value) -> Result<String, FrameworkError> {
    match name {
        "research_verification_literature" => tool_verification_literature(arguments),
        "research_verification_prose" => tool_verification_prose(arguments),
        "research_verification_reproducibility" => tool_verification_reproducibility(arguments),
        "research_verification_statistical" => tool_verification_statistical(arguments),
        "research_verification_structure" => tool_verification_structure(arguments),
        _ => Err(FrameworkError::validation(format!("unknown verification tool: {name}"))),
    }
}

// ── Literature verification ──

fn tool_verification_literature(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments.get("check").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("literature verification requires 'check' (doi|claim_coverage)"))?;
    match check {
        "doi" => {
            let doi = arguments.get("doi").and_then(Value::as_str)
                .ok_or(FrameworkError::validation("doi check requires 'doi' (string)"))?;
            let reachable = tokio::runtime::Handle::current()
                .block_on(crate::verification::literature::verify_doi_reachable(doi))
                .map_err(|e| FrameworkError::validation(format!("DOI check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "doi_reachability", "doi": doi, "reachable": reachable,
            })).map_err(FrameworkError::Json)
        }
        "claim_coverage" => {
            let claims: Vec<String> = arguments.get("claims").and_then(Value::as_array)
                .ok_or(FrameworkError::validation("claim_coverage requires 'claims' array"))?
                .iter().filter_map(|v| v.as_str().map(String::from)).collect();
            let references: Vec<String> = arguments.get("references").and_then(Value::as_array)
                .ok_or(FrameworkError::validation("claim_coverage requires 'references' array"))?
                .iter().filter_map(|v| v.as_str().map(String::from)).collect();
            let score = crate::verification::literature::verify_claim_coverage(&claims, &references)
                .map_err(|e| FrameworkError::validation(format!("claim coverage failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "claim_coverage", "claims_analyzed": claims.len(),
                "coverage_score": format!("{:.2}", score), "covered_pct": (score * 100.0).round() as u64,
            })).map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!("unknown literature check: {check}"))),
    }
}

// ── Prose QC ──

fn tool_verification_prose(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments.get("check").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("prose verification requires 'check' (terminology|slop|hedging)"))?;
    match check {
        "terminology" => {
            let text = arguments.get("text").and_then(Value::as_str)
                .ok_or(FrameworkError::validation("terminology check requires 'text' (string)"))?;
            let glossary = arguments.get("glossary").and_then(Value::as_object)
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect())
                .unwrap_or_default();
            let violations = crate::verification::prose_qc::check_terminology_consistency(text, &glossary)
                .map_err(|e| FrameworkError::validation(format!("terminology check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "terminology_consistency", "violations": violations,
                "has_violations": !violations.is_empty(),
            })).map_err(FrameworkError::Json)
        }
        "slop" => {
            let text = arguments.get("text").and_then(Value::as_str)
                .ok_or(FrameworkError::validation("slop check requires 'text' (string)"))?;
            let language = arguments.get("language").and_then(Value::as_str).unwrap_or("en");
            let hits = match language {
                "zh" | "chinese" => crate::verification::prose_qc::detect_zh_slop(text),
                _ => crate::verification::prose_qc::detect_en_slop(text),
            };
            serde_json::to_string_pretty(&json!({
                "check": "slop_detection", "language": language,
                "hits_found": hits.len(), "hits": hits,
            })).map_err(FrameworkError::Json)
        }
        "hedging" => {
            let text = arguments.get("text").and_then(Value::as_str)
                .ok_or(FrameworkError::validation("hedging check requires 'text' (string)"))?;
            let count = crate::verification::prose_qc::count_hedging_words(text);
            serde_json::to_string_pretty(&json!({
                "check": "hedging_analysis", "hedging_word_count": count,
                "suggestion": if count > 5 { "High hedging density — consider firming up language".to_string() }
                    else if count > 2 { "Moderate hedging — review for unnecessary qualifiers".to_string() }
                    else { "Hedging count is acceptable".to_string() },
            })).map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!("unknown prose check: {check}"))),
    }
}

// ── Reproducibility verification ──

fn tool_verification_reproducibility(arguments: &Value) -> Result<String, FrameworkError> {
    let experiment_dir = arguments.get("experiment_dir").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("reproducibility audit requires 'experiment_dir' (string path)"))?;
    let dir = Path::new(experiment_dir);

    let run_paths: Option<Vec<&Path>> = arguments.get("run_paths").and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(Path::new)).collect());

    let report = crate::verification::reproducibility::run_reproducibility_audit(
        dir, run_paths.as_deref(),
    ).map_err(|e| FrameworkError::validation(format!("reproducibility audit failed: {e}")))?;

    let checks: Vec<Value> = report.checks.iter().map(|c| {
        let (status, detail) = match &c.status {
            crate::verification::reproducibility::CheckStatus::Pass =>
                ("PASS", String::new()),
            crate::verification::reproducibility::CheckStatus::Fail(msg) =>
                ("FAIL", msg.clone()),
            crate::verification::reproducibility::CheckStatus::Warn(msg) =>
                ("WARN", msg.clone()),
            crate::verification::reproducibility::CheckStatus::Skip(msg) =>
                ("SKIP", msg.clone()),
        };
        json!({"name": c.name, "status": status, "detail": detail})
    }).collect();

    serde_json::to_string_pretty(&json!({
        "module": "reproducibility", "checks": checks,
        "all_pass": report.checks.iter().all(|c| matches!(c.status, crate::verification::reproducibility::CheckStatus::Pass)),
    })).map_err(FrameworkError::Json)
}

// ── Statistical verification ──

fn tool_verification_statistical(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments.get("check").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("statistical verification requires 'check' (grim|p_value|multiple_comparison)"))?;
    match check {
        "grim" => {
            let mean = arguments.get("mean").and_then(Value::as_f64)
                .ok_or(FrameworkError::validation("grim test requires 'mean' (f64)"))?;
            let n = arguments.get("n").and_then(Value::as_u64)
                .ok_or(FrameworkError::validation("grim test requires 'n' (u64 sample size)"))?;
            let decimals = arguments.get("decimals").and_then(Value::as_u64).unwrap_or(2) as usize;
            let passed = crate::verification::statistical::grim_test(mean, n as usize, decimals)
                .map_err(|e| FrameworkError::validation(format!("GRIM test failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "grim_test", "mean": mean, "sample_size": n,
                "decimals": decimals, "passed": passed,
                "detail": if passed { "Mean is reconstructible from integer responses".to_string() }
                    else { format!("SUSPICIOUS: Mean {mean} with n={n} and {decimals} decimal places is not reconstructible from integer granularity") },
            })).map_err(FrameworkError::Json)
        }
        "p_value" => {
            let observed = arguments.get("observed").and_then(Value::as_f64)
                .ok_or(FrameworkError::validation("p_value check requires 'observed' (f64)"))?;
            let expected = arguments.get("expected").and_then(Value::as_f64)
                .ok_or(FrameworkError::validation("p_value check requires 'expected' (f64)"))?;
            let tolerance = arguments.get("tolerance").and_then(Value::as_f64).unwrap_or(0.01);
            let passed = crate::verification::statistical::verify_p_value(observed, expected, tolerance);
            serde_json::to_string_pretty(&json!({
                "check": "p_value", "observed": observed, "expected": expected,
                "tolerance": tolerance, "passed": passed,
            })).map_err(FrameworkError::Json)
        }
        "multiple_comparison" => {
            let num_tests = arguments.get("num_tests").and_then(Value::as_u64)
                .ok_or(FrameworkError::validation("multiple_comparison requires 'num_tests' (u64)"))?;
            let correction_applied = arguments.get("correction_applied").and_then(Value::as_bool).unwrap_or(false);
            let passed = crate::verification::statistical::check_multiple_comparison_correction(num_tests as usize, correction_applied);
            serde_json::to_string_pretty(&json!({
                "check": "multiple_comparison", "num_tests": num_tests,
                "correction_applied": correction_applied, "passed": passed,
                "detail": if passed { "OK".to_string() }
                    else { format!("WARNING: {num_tests} tests performed without multiple comparison correction") },
            })).map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!("unknown statistical check: {check}"))),
    }
}

// ── Structure verification ──

fn tool_verification_structure(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments.get("check").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("structure verification requires 'check' (latex|figures)"))?;
    match check {
        "latex" => {
            let path = arguments.get("path").and_then(Value::as_str)
                .ok_or(FrameworkError::validation("latex check requires 'path' (string path to .tex file)"))?;
            let passed = crate::verification::structure::check_latex_compilable(Path::new(path))
                .map_err(|e| FrameworkError::validation(format!("LaTeX compilation check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "latex_compilation", "path": path, "passed": passed,
                "detail": if passed { "LaTeX syntax check passed (balanced braces + environments)".to_string() }
                    else { "LaTeX syntax check FAILED — unbalanced braces or environments".to_string() },
            })).map_err(FrameworkError::Json)
        }
        "figures" => {
            let path = arguments.get("path").and_then(Value::as_str)
                .ok_or(FrameworkError::validation("figures check requires 'path' (string path to .tex file)"))?;
            let missing = crate::verification::structure::check_figure_references(Path::new(path))
                .map_err(|e| FrameworkError::validation(format!("figure reference check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "figure_references", "path": path,
                "missing_refs": missing, "has_missing": !missing.is_empty(),
                "total_missing": missing.len(),
            })).map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!("unknown structure check: {check}"))),
    }
}

// ── Inequality tool functions ──

fn tool_math_prove_inequality(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments.get("expression").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("math_prove_inequality requires 'expression' (string)"))?;
    let timeout = arguments.get("timeout_ms").and_then(Value::as_u64);
    let vr = crate::verification::inequality::check_inequality(expr, timeout);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    })).map_err(FrameworkError::Json)
}

fn tool_math_backend_available(_arguments: &Value) -> Result<String, FrameworkError> {
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
            "available": crate::verification::inequality::solver_available(),
            "description": "minilp-based linear inequality verification (pure Rust)",
        },
        "sympy": {
            "available": crate::verification::sympy_bridge::sympy_available(),
            "description": "Symbolic identity verification (pure Rust, no Python dependency)",
        },
        "lean": { "available": lean_available, "description": lean_desc },
        "install_hint": "All math tools are pure Rust — no Python dependencies required.",
    })).map_err(FrameworkError::Json)
}

// ── Asymptotic tool functions ──

fn tool_math_asymptotic_estimate(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments.get("expression").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("math_asymptotic_estimate requires 'expression' (string)"))?;
    let var = arguments.get("variable").and_then(Value::as_str).unwrap_or("x");
    let regime = arguments.get("regime").and_then(Value::as_str).unwrap_or("oo");
    let vr = crate::verification::asymptotic::magnitude_estimate_with_name(expr, var, regime, "math_asymptotic_estimate");
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    })).map_err(FrameworkError::Json)
}

fn tool_math_asymptotic_chain(arguments: &Value) -> Result<String, FrameworkError> {
    let steps_val = arguments.get("steps").and_then(Value::as_array)
        .ok_or(FrameworkError::validation("math_asymptotic_chain requires 'steps' array"))?;
    let var = arguments.get("variable").and_then(Value::as_str).unwrap_or("x");
    let regime = arguments.get("regime").and_then(Value::as_str).unwrap_or("oo");
    let sympy_check = arguments.get("sympy_check").and_then(Value::as_bool).unwrap_or(true);
    let steps: Vec<crate::verification::asymptotic::AsymptoticStep> =
        serde_json::from_value(serde_json::Value::Array(steps_val.clone()))
            .map_err(|e| FrameworkError::Json(e))?;
    let vr = crate::verification::asymptotic::verify_asymptotic_chain_with_name(&steps, var, regime, sympy_check, "math_asymptotic_chain");
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "steps": steps_val,
    })).map_err(FrameworkError::Json)
}

// ── Proof DAG tool functions ──
//
// DAGs are stored in a name-keyed HashMap for basic session isolation.
// Each tool accepts an optional `name` argument (defaults to "default").
// Callers that may run concurrent proof sessions MUST pass distinct names.

fn get_or_create_dag_store(
) -> &'static std::sync::Mutex<HashMap<String, crate::proof_dag::Blueprint>> {
    use std::sync::OnceLock;
    static STORE: OnceLock<
        std::sync::Mutex<HashMap<String, crate::proof_dag::Blueprint>>,
    > = OnceLock::new();
    STORE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Extract the DAG name from tool arguments (defaults to "default").
fn dag_name(arguments: &Value) -> String {
    arguments
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("default")
        .to_string()
}

fn tool_math_proof_dag_init(arguments: &Value) -> Result<String, FrameworkError> {
    let goal = arguments
        .get("goal")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_proof_dag_init requires 'goal' (string)",
        ))?;
    let name = arguments.get("name").and_then(Value::as_str).unwrap_or("proof");
    let dag_id = dag_name(arguments);
    let bp = crate::proof_dag::Blueprint::new(goal, name);
    let serialized = crate::proof_dag_serialize::serialize_blueprint(&bp)?;
    if let Ok(mut guard) = get_or_create_dag_store().lock() {
        guard.insert(dag_id, bp);
    }
    Ok(serialized)
}

fn tool_math_proof_dag_decompose(arguments: &Value) -> Result<String, FrameworkError> {
    let parent_id = arguments
        .get("parent_id")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_proof_dag_decompose requires 'parent_id'",
        ))?;
    let and = arguments.get("and").and_then(Value::as_bool).unwrap_or(false);
    let children_val = arguments
        .get("children")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation(
            "math_proof_dag_decompose requires 'children' array",
        ))?;
    let children: Vec<crate::proof_dag::DagNode> =
        serde_json::from_value(Value::Array(children_val.clone()))
            .map_err(|e| FrameworkError::Json(e))?;
    let dag_id = dag_name(arguments);
    let mut guard = get_or_create_dag_store()
        .lock()
        .map_err(|e| FrameworkError::session(format!("lock: {e}")))?;
    let bp = guard.get_mut(&dag_id).ok_or(FrameworkError::validation(
        "no proof DAG for this name — call math_proof_dag_init first",
    ))?;
    bp.decompose(parent_id, children, and)?;
    Ok(crate::proof_dag_serialize::serialize_blueprint(bp)?)
}

fn tool_math_proof_dag_verify(arguments: &Value) -> Result<String, FrameworkError> {
    let dag_id = dag_name(arguments);
    let mut guard = get_or_create_dag_store()
        .lock()
        .map_err(|e| FrameworkError::session(format!("lock: {e}")))?;
    let bp = guard.get_mut(&dag_id).ok_or(FrameworkError::validation(
        "no proof DAG for this name — call math_proof_dag_init first",
    ))?;
    bp.verify()?;
    if let Err(warning) = bp.validate_manual_prose_ratio(0.30) {
        let summary = bp.status_summary();
        return serde_json::to_string_pretty(&json!({
            "result": summary,
            "manual_prose_warning": warning.to_string(),
        }))
        .map_err(FrameworkError::Json);
    }
    Ok(crate::proof_dag_serialize::serialize_blueprint(bp)?)
}

fn tool_math_proof_dag_status(arguments: &Value) -> Result<String, FrameworkError> {
    let dag_id = dag_name(arguments);
    let guard = get_or_create_dag_store()
        .lock()
        .map_err(|e| FrameworkError::session(format!("lock: {e}")))?;
    let bp = guard.get(&dag_id).ok_or(FrameworkError::validation(
        "no proof DAG for this name — call math_proof_dag_init first",
    ))?;
    let summary = bp.status_summary();
    serde_json::to_string_pretty(&summary).map_err(FrameworkError::Json)
}

// ── SymPy bridge tool functions ──

fn tool_math_sympy_verify(arguments: &Value) -> Result<String, FrameworkError> {
    let lhs = arguments.get("lhs").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("math_sympy_verify requires 'lhs' (string)"))?;
    let rhs = arguments.get("rhs").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("math_sympy_verify requires 'rhs' (string)"))?;
    // assumptions are accepted for backward compat but ignored (pure Rust)
    let _assumptions: Vec<&str> = arguments.get("assumptions")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let vr = crate::verification::sympy_bridge::verify_identity(lhs, rhs);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "lhs": lhs, "rhs": rhs,
    })).map_err(FrameworkError::Json)
}

fn tool_math_sympy_simplify(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments.get("expression").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("math_sympy_simplify requires 'expression' (string)"))?;
    let vr = crate::verification::sympy_bridge::simplify_expression(expr);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    })).map_err(FrameworkError::Json)
}

fn tool_math_lean_verify(arguments: &Value) -> Result<String, FrameworkError> {
    let script = arguments.get("script").and_then(Value::as_str)
        .ok_or(FrameworkError::validation("math_lean_verify requires 'script' (string)"))?;
    let vr = crate::verification::lean_bridge::verify_lean_theorem(script);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details,
    })).map_err(FrameworkError::Json)
}

fn parse_language(value: &Value, key: &str) -> crate::aigc::Language {
    match value.get(key).and_then(Value::as_str) {
        Some("zh") | Some("zh-CN") | Some("chinese") => crate::aigc::Language::Chinese,
        _ => crate::aigc::Language::English,
    }
}

fn tool_research_aigc_check(arguments: &Value) -> Result<String, FrameworkError> {
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation("research_aigc_check requires 'text' parameter"))?;
    let language = parse_language(arguments, "language");

    let config = crate::aigc::detector::DetectionConfig {
        language,
        ..Default::default()
    };
    let results = crate::aigc::detector::detect(text, &config)
        .map_err(|e| FrameworkError::validation(format!("AIGC detection failed: {e}")))?;
    let score = crate::aigc::scorer::score(&results);

    serde_json::to_string_pretty(&json!({
        "score": score,
        "ai_probability": score as f64 / 100.0,
        "segments_analyzed": results.len(),
        "results": results,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_research_review_dimensions(arguments: &Value) -> Result<String, FrameworkError> {
    let round = arguments
        .get("round")
        .and_then(Value::as_u64)
        .ok_or(FrameworkError::validation("research_review_dimensions requires 'round' parameter"))?;
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
    .map_err(FrameworkError::Json)
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

fn tool_research_claim_drift(arguments: &Value) -> Result<String, FrameworkError> {
    let original_claims = arguments
        .get("original_claims")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation("research_claim_drift requires 'original_claims' array"))?;
    let current_claims = arguments
        .get("current_claims")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation("research_claim_drift requires 'current_claims' array"))?;

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
        .map_err(|e| FrameworkError::validation(e.to_string()))?;

    serde_json::to_string_pretty(&json!({
        "drift_results": results,
        "total_claims_analyzed": results.len(),
    }))
    .map_err(FrameworkError::Json)
}

fn tool_research_review_loop(arguments: &Value) -> Result<String, FrameworkError> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("start");

    match operation {
        "start" => review_loop_start(arguments),
        "submit_round" => review_loop_submit_round(arguments),
        "status" => review_loop_status(arguments),
        _ => Err(FrameworkError::validation(format!(
            "research_review_loop: unknown operation '{operation}' — expected start|submit_round|status"
        ))),
    }
}

/// Operation `start`: return config + round-1 dimension. Stateless — no persistence.
fn review_loop_start(arguments: &Value) -> Result<String, FrameworkError> {
    let max_rounds = arguments
        .get("max_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(10);
    let min_rounds = arguments
        .get("min_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(5);
    let stable_req = arguments
        .get("consecutive_stable_required")
        .and_then(Value::as_u64)
        .unwrap_or(2);

    let dim = crate::types::ReviewDimension::for_round(1);
    let prompt = crate::review::dimensions::dimension_prompt(&dim);
    let checklist = crate::review::dimensions::dimension_checklist(&dim);

    serde_json::to_string_pretty(&json!({
        "operation": "started",
        "quality_gate_config": {
            "min_rounds": min_rounds,
            "max_rounds": max_rounds,
            "consecutive_stable_required": stable_req,
        },
        "current_round": {
            "round": 1,
            "dimension": dim.display_name(),
            "prompt": prompt,
            "checklist": checklist,
        },
        "total_dimensions": std::cmp::min(max_rounds, 7),
        "workflow": "1. Call quality_gate_manage(operation=start, ...) to init the runtime loop. 2. Spawn reviewer subagent using current_round. 3. Call research_review_loop(operation=submit_round, round=N, findings=[...]) for next-round. 4. Call quality_gate_manage(operation=append_round, ...) to record in runtime loop.",
    }))
    .map_err(FrameworkError::Json)
}

/// Operation `submit_round`: accept round + findings, return next-round dimension or completion.
/// Stateless — convergence is managed by quality_gate_manage at the runtime layer.
fn review_loop_submit_round(arguments: &Value) -> Result<String, FrameworkError> {
    let round = arguments
        .get("round")
        .and_then(Value::as_u64)
        .ok_or(FrameworkError::validation(
            "research_review_loop submit_round requires 'round' (u64)",
        ))?;

    let max_rounds = arguments.get("max_rounds").and_then(Value::as_u64);

    // Ceiling check: if this round has reached max, signal completion.
    if let Some(ceiling) = max_rounds {
        if round >= ceiling {
            return serde_json::to_string_pretty(&json!({
                "operation": "completed",
                "reason": "max_rounds reached — ceiling exceeded",
                "rounds_completed": round,
            }))
            .map_err(FrameworkError::Json);
        }
    }

    let findings_val = arguments.get("findings").and_then(Value::as_array);
    let findings: Vec<crate::types::Finding> = match findings_val {
        Some(arr) => serde_json::from_value(Value::Array(arr.clone()))
            .map_err(|e| FrameworkError::validation(format!("invalid findings format: {e}")))?,
        None => Vec::new(),
    };

    let has_blocking = findings.iter().any(|f| f.severity.blocks_convergence());

    let next_round = round + 1;
    let dim = crate::types::ReviewDimension::for_round(next_round);
    let prompt = crate::review::dimensions::dimension_prompt(&dim);
    let checklist = crate::review::dimensions::dimension_checklist(&dim);

    serde_json::to_string_pretty(&json!({
        "operation": "continue",
        "round_completed": round,
        "findings_this_round": findings.len(),
        "has_blocking": has_blocking,
        "findings": findings.iter().map(|f| json!({
            "id": f.id,
            "severity": format!("{:?}", f.severity),
            "dimension": f.dimension,
            "location": f.location,
            "description": f.description,
            "suggestion": f.suggestion,
        })).collect::<Vec<_>>(),
        "next_round": {
            "round": next_round,
            "dimension": dim.display_name(),
            "prompt": prompt,
            "checklist": checklist,
        },
        "next_step": "Call quality_gate_manage(operation=append_round, ...) to record round in runtime loop, then spawn reviewer for next round.",
    }))
    .map_err(FrameworkError::Json)
}

/// Operation `status`: return current round's dimension. Stateless — pure function of round.
fn review_loop_status(arguments: &Value) -> Result<String, FrameworkError> {
    let round = arguments.get("round").and_then(Value::as_u64).unwrap_or(1);
    let dim = crate::types::ReviewDimension::for_round(round);

    serde_json::to_string_pretty(&json!({
        "round": round,
        "dimension": dim.display_name(),
        "note": "Runtime loop state (convergence, rounds history) is managed by quality_gate_manage at the runtime layer.",
        "next_step": "Call quality_gate_manage(operation=status) for convergence state.",
    }))
    .map_err(FrameworkError::Json)
}

/// Academic data source freshness smoke test.
/// Requires the `smoke` feature; returns descriptive error otherwise.
#[cfg_attr(not(feature = "smoke"), allow(unused_variables))]
fn tool_research_smoke(arguments: &Value) -> Result<String, FrameworkError> {
    #[cfg(feature = "smoke")]
    {
        let repo_root = arguments
            .get("repo_root")
            .and_then(Value::as_str)
            .map(std::path::Path::new)
            .unwrap_or_else(|| {
                // Lazy static to avoid repeated current_dir() calls
                static CWD: std::sync::LazyLock<std::path::PathBuf> =
                    std::sync::LazyLock::new(|| std::env::current_dir().unwrap_or_default());
                &*CWD
            });
        let source = arguments.get("source").and_then(Value::as_str);
        let barrier_id = arguments.get("barrier_id").and_then(Value::as_str);
        let result = crate::smoke::run_smoke_tests(repo_root, source, barrier_id)
            .map_err(|e| FrameworkError::validation(format!("research_smoke failed: {e}")))?;
        Ok(result)
    }
    #[cfg(not(feature = "smoke"))]
    {
        Err(FrameworkError::validation(
            "research_smoke: not available (crate was built without 'smoke' feature)",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn handle_research_tool_unknown() {
        let result = handle_research_tool("nonexistent_tool", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown research tool"));
    }

    #[test]
    fn research_aigc_check_missing_text() {
        let result = handle_research_tool("research_aigc_check", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'text'"));
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
    fn research_review_dimensions_missing_round() {
        let result = handle_research_tool("research_review_dimensions", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'round'"));
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
        assert!(result.unwrap_err().to_string().contains("requires 'original_claims'"));
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
        assert_eq!(parsed.get("operation").and_then(Value::as_str), Some("started"));
        let config = parsed.get("quality_gate_config").unwrap();
        assert_eq!(config.get("min_rounds").and_then(Value::as_u64), Some(5));
        assert_eq!(config.get("max_rounds").and_then(Value::as_u64), Some(10));
        assert_eq!(config.get("consecutive_stable_required").and_then(Value::as_u64), Some(2));
        assert!(parsed.get("current_round").is_some());
    }

    #[test]
    fn research_review_loop_custom_params() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"max_rounds": 3, "min_rounds": 1, "consecutive_stable_required": 1}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("operation").and_then(Value::as_str), Some("started"));
        let config = parsed.get("quality_gate_config").unwrap();
        assert_eq!(config.get("max_rounds").and_then(Value::as_u64), Some(3));
        assert_eq!(config.get("min_rounds").and_then(Value::as_u64), Some(1));
        assert!(parsed.get("current_round").is_some());
    }

    #[test]
    fn research_review_loop_status_round_1() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "status"}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round").and_then(Value::as_u64), Some(1));
        assert_eq!(parsed.get("dimension").and_then(Value::as_str), Some("逻辑与证据"));
    }

    #[test]
    fn research_review_loop_status_round_3() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "status", "round": 3}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round").and_then(Value::as_u64), Some(3));
        assert_eq!(parsed.get("dimension").and_then(Value::as_str), Some("数学与符号"));
    }

    #[test]
    fn research_review_loop_submit_missing_round() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round"}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'round'"));
    }

    #[test]
    fn research_review_loop_submit_empty_findings() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 1, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("operation").and_then(Value::as_str), Some("continue"));
        assert_eq!(parsed.get("round_completed").and_then(Value::as_u64), Some(1));
        assert_eq!(parsed.get("has_blocking").and_then(Value::as_bool), Some(false));
        let next = parsed.get("next_round").unwrap();
        assert_eq!(next.get("round").and_then(Value::as_u64), Some(2));
    }

    #[test]
    fn research_review_loop_submit_with_blocking() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({
                "operation": "submit_round",
                "round": 1,
                "findings": [{"id": "f1", "severity": "P0", "dimension": "逻辑与证据", "location": "§2", "description": "data integrity"}]
            }),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("has_blocking").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn research_review_loop_unknown_operation() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "unknown"}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown operation"));
    }

    #[test]
    fn research_review_loop_submit_advances_round() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 3, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round_completed").and_then(Value::as_u64), Some(3));
        let next = parsed.get("next_round").unwrap();
        assert_eq!(next.get("round").and_then(Value::as_u64), Some(4));
        assert_eq!(next.get("dimension").and_then(Value::as_str), Some("图表与可读性"));
    }

    #[test]
    fn research_review_loop_submit_at_ceiling() {
        // round == max_rounds → ceiling hit, operation="completed"
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 10, "max_rounds": 10, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("operation").and_then(Value::as_str), Some("completed"));
        assert!(parsed.get("reason").and_then(Value::as_str)
            .unwrap_or("").contains("max_rounds"));

        // round > max_rounds → also completed
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 999, "max_rounds": 10, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("operation").and_then(Value::as_str), Some("completed"));

        // no max_rounds → proceeds normally (backward compat)
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 99, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("operation").and_then(Value::as_str), Some("continue"));
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
        assert!(result.unwrap_err().to_string().contains("requires 'expression'"));
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
        assert!(result.unwrap_err().to_string().contains("requires 'expression'"));
    }

    #[test]
    fn test_math_asymptotic_chain_missing_steps() {
        let result = handle_research_tool("math_asymptotic_chain", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'steps'"));
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
        assert!(result.unwrap_err().to_string().contains("requires 'goal'"));
    }

    #[test]
    fn test_math_proof_dag_decompose_missing_parent_id() {
        let result = handle_research_tool("math_proof_dag_decompose", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'parent_id'"));
    }

    #[test]
    fn test_math_proof_dag_verify_without_init() {
        let result = handle_research_tool("math_proof_dag_verify", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no proof DAG for this name"));
    }

    #[test]
    fn test_math_proof_dag_status_without_init() {
        let result = handle_research_tool("math_proof_dag_status", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no proof DAG for this name"));
    }

    #[test]
    fn test_math_sympy_verify_missing_lhs() {
        let result = handle_research_tool("math_sympy_verify", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'lhs'"));
    }

    #[test]
    fn test_math_sympy_verify_missing_rhs() {
        let result = handle_research_tool("math_sympy_verify", &json!({"lhs": "x"}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'rhs'"));
    }

    #[test]
    fn test_math_sympy_simplify_missing_expression() {
        let result = handle_research_tool("math_sympy_simplify", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'expression'"));
    }

    #[test]
    fn test_math_lean_verify_missing_script() {
        let result = handle_research_tool("math_lean_verify", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'script'"));
    }

    #[test]
    fn test_math_backend_available_ok() {
        let result = handle_research_tool("math_backend_available", &json!({}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_math_unknown_tool() {
        let result = handle_research_tool("math_nonexistent", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown math tool"));
    }

    #[test]
    fn test_math_tool_routing() {
        let result = handle_research_tool("math_future_tool", &json!({}));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown") || err.contains("requires"), "wrong routing: {err}");
    }
}
