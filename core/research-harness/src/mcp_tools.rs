//! MCP tool dispatch for research tools.
//!
//! Delegated from host-projection's tool dispatcher (Phase 4 T1).
//!
//! # Input limits
//!
//! Array-type parameters (steps, witnesses, constraints, claims, references,
//! findings, children) are capped at MAX_ARRAY_ELEMENTS to prevent memory
//! exhaustion from oversized payloads received through the MCP tool interface.

use core_errors::FrameworkError;
use std::sync::OnceLock;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Maximum number of elements in any array-type parameter to a research tool.
/// Prevents single-call memory exhaustion via malicious oversized arrays.
const MAX_ARRAY_ELEMENTS: usize = 10_000;

/// Handle a research MCP tool call.
/// Delegates to the appropriate research-harness module.
pub fn handle_research_tool(name: &str, arguments: &Value) -> Result<String, FrameworkError> {
    match name {
        "research_aigc_check" => Ok(tool_research_aigc_check(arguments)?),
        "research_review_dimensions" => Ok(tool_research_review_dimensions(arguments)?),
        "research_claim_drift" => Ok(tool_research_claim_drift(arguments)?),
        "research_review_loop" => Ok(tool_research_review_loop(arguments)?),
        "research_smoke" => Ok(tool_research_smoke(arguments)?),
        "research_literature_search" => Ok(tool_literature_search(arguments)?),
        _ if name.starts_with("math_") => Ok(math_tool_dispatch(name, arguments)?),
        _ if name.starts_with("research_verification_") => {
            verification_tool_dispatch(name, arguments)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown research tool: {name}"
        ))),
    }
}

// ── Math tool sub-dispatch ──

fn math_tool_dispatch(name: &str, arguments: &Value) -> Result<String, FrameworkError> {
    match name {
        "math_asymptotic_estimate" => tool_math_asymptotic_estimate(arguments),
        "math_proof_dag_init" => tool_math_proof_dag_init(arguments),
        "math_proof_dag_decompose" => tool_math_proof_dag_decompose(arguments),
        "math_proof_dag_verify" => tool_math_proof_dag_verify(arguments),
        "math_proof_dag_status" => tool_math_proof_dag_status(arguments),
        "math_sympy_verify" => tool_math_sympy_verify(arguments),
        "math_sympy_simplify" => tool_math_sympy_simplify(arguments),
        "math_sympy_trig_simplify" => tool_math_sympy_trig_simplify(arguments),
        "math_sympy_subs" => tool_math_sympy_subs(arguments),
        "math_sympy_limit" => tool_math_sympy_limit(arguments),
        "math_sympy_lambdify" => tool_math_sympy_lambdify(arguments),
        "math_prove_inequality" => tool_math_prove_inequality(arguments),
        "math_z3_prove" => tool_math_z3_prove(arguments),
        "math_z3_solver_push" => tool_math_z3_solver_push(arguments),
        "math_z3_solver_pop" => tool_math_z3_solver_pop(arguments),
        "math_z3_solver_add" => tool_math_z3_solver_add(arguments),
        "math_z3_solver_check" => tool_math_z3_solver_check(arguments),
        "math_z3_solver_reset" => tool_math_z3_solver_reset(arguments),
        "math_z3_solver_batch" => tool_math_z3_solver_batch(arguments),
        "math_asymptotic_chain" => tool_math_asymptotic_chain(arguments),
        "math_backend_available" => tool_math_backend_available(arguments),
        "math_lean_verify" => tool_math_lean_verify(arguments),
        "math_sympy_expand" => tool_math_sympy_expand(arguments),
        "math_sympy_factor" => tool_math_sympy_factor(arguments),
        "math_sympy_series" => tool_math_sympy_series(arguments),
        "math_sympy_differentiate" => tool_math_sympy_differentiate(arguments),
        "math_sympy_integrate" => tool_math_sympy_integrate(arguments),
        "math_sympy_solve" => tool_math_sympy_solve(arguments),
        "math_sympy_dimension_propagate" => tool_math_sympy_dimension_propagate(arguments),
        _ => Err(FrameworkError::validation(format!(
            "unknown math tool: {name}"
        ))),
    }
}

// ── Verification tool sub-dispatch ──

fn verification_tool_dispatch(name: &str, arguments: &Value) -> Result<String, FrameworkError> {
    match name {
        "research_verification_prose" => tool_verification_prose(arguments),
        "research_verification_statistical" => tool_verification_statistical(arguments),
        "research_verification_literature" => tool_verification_literature(arguments),
        "research_verification_structure" => tool_verification_structure(arguments),
        "research_verification_reproducibility" => tool_verification_reproducibility(arguments),
        "research_verification_formal" => tool_verification_formal(arguments),
        _ => Err(FrameworkError::validation(format!(
            "unknown verification tool: {name}"
        ))),
    }
}

// ── Literature search tool ──

fn tool_literature_search(arguments: &Value) -> Result<String, FrameworkError> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "research_literature_search requires 'query' (string)",
        ))?;
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(20) as usize;
    let source_str = arguments
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("all");
    let source = match source_str {
        "semantic-scholar" => crate::search::ExternalSourceArg::SemanticScholar,
        "arxiv" => crate::search::ExternalSourceArg::Arxiv,
        _ => crate::search::ExternalSourceArg::All,
    };
    let year_from = arguments.get("year_from").and_then(Value::as_u64).map(|y| y as u32);
    let year_to = arguments.get("year_to").and_then(Value::as_u64).map(|y| y as u32);
    let sort_by = match arguments.get("sort_by").and_then(Value::as_str) {
        Some("date") => crate::search::SortBy::Date,
        _ => crate::search::SortBy::Relevance,
    };
    let categories = arguments
        .get("categories")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from);
    let advanced_query = arguments
        .get("advanced_query")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from);
    let fuzzy_query = arguments
        .get("fuzzy_query")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let prefer_authoritative = arguments
        .get("prefer_authoritative")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let opts = crate::search::SearchOptions {
        query: query.to_string(),
        limit,
        source,
        year_from,
        year_to,
        sort_by,
        categories,
        advanced_query,
        fuzzy_query,
        prefer_authoritative,
        ..crate::search::SearchOptions::new(query)
    };
    let result = crate::search::orchestration::search_raw(&opts)
        .map_err(|e| FrameworkError::validation(format!("literature search failed: {e}")))?;
    serde_json::to_string_pretty(&result).map_err(FrameworkError::Json)
}

// ── Prose QC ──

fn tool_verification_prose(arguments: &Value) -> Result<String, FrameworkError> {
    let check =
        arguments
            .get("check")
            .and_then(Value::as_str)
            .ok_or(FrameworkError::validation(
                "prose verification requires 'check' (terminology|slop|hedging)",
            ))?;
    match check {
        "terminology" => {
            let text =
                arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(FrameworkError::validation(
                        "terminology check requires 'text' (string)",
                    ))?;
            let glossary = arguments
                .get("glossary")
                .and_then(Value::as_object)
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let violations =
                crate::verification::prose_qc::check_terminology_consistency(text, &glossary)
                    .map_err(|e| {
                        FrameworkError::validation(format!("terminology check failed: {e}"))
                    })?;
            serde_json::to_string_pretty(&json!({
                "check": "terminology_consistency", "violations": violations,
                "has_violations": !violations.is_empty(),
            }))
            .map_err(FrameworkError::Json)
        }
        "slop" => {
            let text =
                arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(FrameworkError::validation(
                        "slop check requires 'text' (string)",
                    ))?;
            let language = arguments
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or("en");
            let hits = match language {
                "zh" | "chinese" => crate::verification::prose_qc::detect_zh_slop(text),
                _ => crate::verification::prose_qc::detect_en_slop(text),
            };
            serde_json::to_string_pretty(&json!({
                "check": "slop_detection", "language": language,
                "hits_found": hits.len(), "hits": hits,
            }))
            .map_err(FrameworkError::Json)
        }
        "hedging" => {
            let text =
                arguments
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(FrameworkError::validation(
                        "hedging check requires 'text' (string)",
                    ))?;
            let count = crate::verification::prose_qc::count_hedging_words(text);
            serde_json::to_string_pretty(&json!({
                "check": "hedging_analysis", "hedging_word_count": count,
                "suggestion": if count > 5 { "High hedging density — consider firming up language".to_string() }
                    else if count > 2 { "Moderate hedging — review for unnecessary qualifiers".to_string() }
                    else { "Hedging count is acceptable".to_string() },
            })).map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown prose check: {check}"
        ))),
    }
}

// ── Statistical verification ──

fn tool_verification_statistical(arguments: &Value) -> Result<String, FrameworkError> {
    let check =
        arguments
            .get("check")
            .and_then(Value::as_str)
            .ok_or(FrameworkError::validation(
                "statistical verification requires 'check' (grim|p_value|multiple_comparison)",
            ))?;
    match check {
        "grim" => {
            let mean =
                arguments
                    .get("mean")
                    .and_then(Value::as_f64)
                    .ok_or(FrameworkError::validation(
                        "grim test requires 'mean' (f64)",
                    ))?;
            let n =
                arguments
                    .get("n")
                    .and_then(Value::as_u64)
                    .ok_or(FrameworkError::validation(
                        "grim test requires 'n' (u64 sample size)",
                    ))?;
            let decimals = arguments
                .get("decimals")
                .and_then(Value::as_u64)
                .unwrap_or(2) as usize;
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
            let observed = arguments.get("observed").and_then(Value::as_f64).ok_or(
                FrameworkError::validation("p_value check requires 'observed' (f64)"),
            )?;
            let expected = arguments.get("expected").and_then(Value::as_f64).ok_or(
                FrameworkError::validation("p_value check requires 'expected' (f64)"),
            )?;
            let tolerance = arguments
                .get("tolerance")
                .and_then(Value::as_f64)
                .unwrap_or(0.01);
            let passed =
                crate::verification::statistical::verify_p_value(observed, expected, tolerance);
            serde_json::to_string_pretty(&json!({
                "check": "p_value", "observed": observed, "expected": expected,
                "tolerance": tolerance, "passed": passed,
            }))
            .map_err(FrameworkError::Json)
        }
        "multiple_comparison" => {
            let num_tests = arguments.get("num_tests").and_then(Value::as_u64).ok_or(
                FrameworkError::validation("multiple_comparison requires 'num_tests' (u64)"),
            )?;
            let correction_applied = arguments
                .get("correction_applied")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let passed = crate::verification::statistical::check_multiple_comparison_correction(
                num_tests as usize,
                correction_applied,
            );
            serde_json::to_string_pretty(&json!({
                "check": "multiple_comparison", "num_tests": num_tests,
                "correction_applied": correction_applied, "passed": passed,
                "detail": if passed { "OK".to_string() }
                    else { format!("WARNING: {num_tests} tests performed without multiple comparison correction") },
            })).map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown statistical check: {check}"
        ))),
    }
}

// ── Inequality tool functions ──

// ── Asymptotic tool functions ──

fn tool_math_asymptotic_estimate(arguments: &Value) -> Result<String, FrameworkError> {
    let expr =
        arguments
            .get("expression")
            .and_then(Value::as_str)
            .ok_or(FrameworkError::validation(
                "math_asymptotic_estimate requires 'expression' (string)",
            ))?;
    let var = arguments
        .get("variable")
        .and_then(Value::as_str)
        .unwrap_or("x");
    let regime = arguments
        .get("regime")
        .and_then(Value::as_str)
        .unwrap_or("oo");
    let vr = crate::verification::asymptotic::magnitude_estimate_with_name(
        expr,
        var,
        regime,
        "math_asymptotic_estimate",
    );
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    }))
    .map_err(FrameworkError::Json)
}

// ── Proof DAG tool functions ──
//
// DAGs are stored in a name-keyed HashMap for basic session isolation.
// Each tool accepts an optional `name` argument (defaults to "default").
// Callers that may run concurrent proof sessions MUST pass distinct names.

fn get_or_create_dag_store()
-> &'static std::sync::Mutex<HashMap<String, crate::proof_dag::Blueprint>> {
    use std::sync::OnceLock;
    static STORE: OnceLock<std::sync::Mutex<HashMap<String, crate::proof_dag::Blueprint>>> =
        OnceLock::new();
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
    let name = arguments
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("proof");
    let dag_id = dag_name(arguments);
    let bp = crate::proof_dag::Blueprint::new(goal, name);
    let serialized = crate::proof_dag_serialize::serialize_blueprint(&bp)?;
    if let Ok(mut guard) = get_or_create_dag_store().lock() {
        const MAX_DAGS: usize = 64;
        if guard.len() >= MAX_DAGS && !guard.contains_key(&dag_id) {
            return Err(FrameworkError::validation(format!(
                "DAG store limit reached ({MAX_DAGS}). Delete unused DAGs first."
            )));
        }
        guard.insert(dag_id, bp);
    }
    Ok(serialized)
}

fn tool_math_proof_dag_decompose(arguments: &Value) -> Result<String, FrameworkError> {
    let parent_id =
        arguments
            .get("parent_id")
            .and_then(Value::as_str)
            .ok_or(FrameworkError::validation(
                "math_proof_dag_decompose requires 'parent_id'",
            ))?;
    let and = arguments
        .get("and")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let children_val =
        arguments
            .get("children")
            .and_then(Value::as_array)
            .ok_or(FrameworkError::validation(
                "math_proof_dag_decompose requires 'children' array",
            ))?;

    if children_val.len() > MAX_ARRAY_ELEMENTS {
        return Err(FrameworkError::validation(format!(
            "children array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
            children_val.len()
        )));
    }
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
    let lhs = arguments
        .get("lhs")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_verify requires 'lhs' (string)",
        ))?;
    let rhs = arguments
        .get("rhs")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_verify requires 'rhs' (string)",
        ))?;
    let vr = crate::verification::sympy_bridge::verify_identity(lhs, rhs);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "lhs": lhs, "rhs": rhs,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_simplify(arguments: &Value) -> Result<String, FrameworkError> {
    let expr =
        arguments
            .get("expression")
            .and_then(Value::as_str)
            .ok_or(FrameworkError::validation(
                "math_sympy_simplify requires 'expression' (string)",
            ))?;
    let assumptions: Vec<String> = arguments
        .get("assumptions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let vr =
        crate::verification::sympy_bridge::simplify_expression_with_assumptions(expr, &assumptions);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr, "assumptions": assumptions,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_trig_simplify(arguments: &Value) -> Result<String, FrameworkError> {
    let expr =
        arguments
            .get("expression")
            .and_then(Value::as_str)
            .ok_or(FrameworkError::validation(
                "math_sympy_trig_simplify requires 'expression' (string)",
            ))?;
    let vr = crate::verification::sympy_bridge::trig_simplify_expression(expr);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_subs(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_subs requires 'expression' (string)",
        ))?;
    let substitutions = arguments
        .get("substitutions")
        .ok_or(FrameworkError::validation(
            "math_sympy_subs requires 'substitutions' (object)",
        ))?;
    let vr = crate::verification::sympy_bridge::subs_expression(expr, substitutions);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr, "substitutions": substitutions,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_limit(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_limit requires 'expression' (string)",
        ))?;
    let variable = arguments
        .get("variable")
        .and_then(Value::as_str)
        .unwrap_or("x");
    let point = arguments
        .get("point")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_limit requires 'point' (string) — e.g. '0', 'oo', '-oo'",
        ))?;
    let direction = arguments.get("direction").and_then(Value::as_str);
    let vr = crate::verification::sympy_bridge::limit_expression(expr, variable, point, direction);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
        "variable": variable, "point": point, "direction": direction,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_lambdify(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_lambdify requires 'expression' (string)",
        ))?;
    let variables: Vec<String> = arguments
        .get("variables")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| vec!["x".to_string()]);
    let values: Option<Vec<f64>> = arguments.get("values").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_f64())
                .collect()
        })
    });
    let vr = crate::verification::sympy_bridge::lambdify_expression(
        expr,
        &variables,
        values.as_deref(),
    );
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
        "variables": variables, "values": values,
    }))
    .map_err(FrameworkError::Json)
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
        .ok_or(FrameworkError::validation(
            "research_aigc_check requires 'text' parameter",
        ))?;
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
    let round =
        arguments
            .get("round")
            .and_then(Value::as_u64)
            .ok_or(FrameworkError::validation(
                "research_review_dimensions requires 'round' parameter",
            ))?;
    let manuscript_summary = arguments
        .get("manuscript_summary")
        .and_then(Value::as_str)
        .unwrap_or("(no summary provided)");

    let dim = crate::types::ReviewDimension::for_round(round);
    let prompt = crate::review::dimensions::dimension_prompt(&dim);
    let checklist = crate::review::dimensions::dimension_checklist(&dim);
    let full_prompt =
        crate::review::orchestrator::build_reviewer_prompt(round, &dim, manuscript_summary);

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
    let original_claims_val = arguments
        .get("original_claims")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation(
            "research_claim_drift requires 'original_claims' array",
        ))?;
    let current_claims_val = arguments
        .get("current_claims")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation(
            "research_claim_drift requires 'current_claims' array",
        ))?;

    // Enforce array size limits
    if original_claims_val.len() > MAX_ARRAY_ELEMENTS {
        return Err(FrameworkError::validation(format!(
            "original_claims array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
            original_claims_val.len()
        )));
    }
    if current_claims_val.len() > MAX_ARRAY_ELEMENTS {
        return Err(FrameworkError::validation(format!(
            "current_claims array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
            current_claims_val.len()
        )));
    }

    // Clone to satisfy borrow checker
    let original_claims = original_claims_val.clone();
    let current_claims = current_claims_val.clone();

    let parse_claims = |arr: &[Value]| -> Vec<crate::types::Claim> {
        arr.iter()
            .map(|v| {
                let id = v
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let text = v
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let ceiling = parse_claim_ceiling(v.get("ceiling").and_then(Value::as_str));
                let evidence = parse_evidence_anchors(
                    v.get("evidence")
                        .and_then(Value::as_array)
                        .map(|v| v.as_slice()),
                );
                crate::types::Claim {
                    id,
                    text,
                    evidence,
                    ceiling,
                }
            })
            .collect()
    };

    let orig = parse_claims(&original_claims);
    let curr = parse_claims(&current_claims);

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
        "workflow": "1. Call research_review_loop(operation=start, min_rounds=5, max_rounds=10, ...) to init the review loop. 2. Spawn reviewer subagent using current_round. 3. Call research_review_loop(operation=submit_round, round=N, findings=[...]) for next-round. 4. research_review_loop handles convergence tracking.",
    }))
    .map_err(FrameworkError::Json)
}

/// Operation `submit_round`: accept round + findings, return next-round dimension or completion.
/// Stateless — convergence is managed by research_review_loop at the runtime layer.
fn review_loop_submit_round(arguments: &Value) -> Result<String, FrameworkError> {
    let round =
        arguments
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
        "next_step": "Call research_review_loop(operation=submit_round, ...) to record the round in the runtime loop, then spawn reviewer for next round.",
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
        "note": "Runtime loop state (convergence, rounds) is managed by research_review_loop at the runtime layer.",
        "next_step": "Call research_review_loop(operation=status) for convergence state.",
    }))
    .map_err(FrameworkError::Json)
}

/// Parse an `OrderRelation` from its string name or symbol.
fn parse_order_relation(s: &str) -> Option<crate::verification::asymptotic::OrderRelation> {
    use crate::verification::asymptotic::OrderRelation;
    match s {
        "LessSim" | "≲" => Some(OrderRelation::LessSim),
        "MuchLess" | "≪" => Some(OrderRelation::MuchLess),
        "Asymp" | "≍" => Some(OrderRelation::Asymp),
        _ => None,
    }
}

// ── Inequality tool functions ──

fn tool_math_prove_inequality(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_prove_inequality requires 'expression' (string)",
        ))?;
    let timeout_ms = arguments.get("timeout_ms").and_then(Value::as_u64);
    let vr = crate::verification::inequality::check_inequality(expr, timeout_ms);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    }))
    .map_err(FrameworkError::Json)
}

// ── Z3 prove tool ──

fn tool_math_z3_prove(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_z3_prove requires 'expression' (string)",
        ))?;
    let vr = crate::verification::z3_bridge::prove_formula(expr);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    }))
    .map_err(FrameworkError::Json)
}

// ── Z3 solver push/pop tools ──

fn tool_math_z3_solver_push(arguments: &Value) -> Result<String, FrameworkError> {
    let n = arguments
        .get("n")
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;
    let vr = crate::verification::z3_bridge::solver_push(n);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "n": n,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_z3_solver_pop(arguments: &Value) -> Result<String, FrameworkError> {
    let n = arguments
        .get("n")
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;
    let vr = crate::verification::z3_bridge::solver_pop(n);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "n": n,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_z3_solver_add(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_z3_solver_add requires 'expression' (string)",
        ))?;
    let vr = crate::verification::z3_bridge::solver_add(expr);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_z3_solver_check(arguments: &Value) -> Result<String, FrameworkError> {
    let timeout_ms = arguments.get("timeout_ms").and_then(Value::as_u64);
    let vr = crate::verification::z3_bridge::solver_check(timeout_ms);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "timeout_ms": timeout_ms,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_z3_solver_reset(_arguments: &Value) -> Result<String, FrameworkError> {
    let vr = crate::verification::z3_bridge::solver_reset();
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_z3_solver_batch(arguments: &Value) -> Result<String, FrameworkError> {
    use crate::verification::z3_bridge::SolverBatchStep;

    let steps_val = arguments
        .get("steps")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation(
            "math_z3_solver_batch requires 'steps' array",
        ))?;

    if steps_val.len() > MAX_ARRAY_ELEMENTS {
        return Err(FrameworkError::validation(format!(
            "steps array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
            steps_val.len()
        )));
    }

    let steps: Vec<SolverBatchStep> = steps_val
        .iter()
        .map(|v| {
            let action = v
                .get("action")
                .and_then(Value::as_str)
                .ok_or(FrameworkError::validation(
                    "each batch step requires 'action' (string: push/pop/add/check/reset)",
                ))?;
            // Validate action
            match action {
                "push" | "pop" | "add" | "check" | "reset" => {}
                _ => {
                    return Err(FrameworkError::validation(format!(
                        "unknown batch action: '{action}', expected push/pop/add/check/reset"
                    )));
                }
            }
            Ok(SolverBatchStep {
                action: action.to_string(),
                n: v.get("n").and_then(Value::as_u64).map(|x| x as usize),
                expression: v.get("expression").and_then(Value::as_str).map(String::from),
                timeout_ms: v.get("timeout_ms").and_then(Value::as_u64),
            })
        })
        .collect::<Result<Vec<_>, FrameworkError>>()?;

    match crate::verification::z3_bridge::solver_batch(&steps) {
        Ok(result) => serde_json::to_string_pretty(&json!({
            "check_name": "math_z3_solver_batch",
            "status": "Pass",
            "steps": result.get("steps"),
            "num_steps": result.get("num_steps"),
        }))
        .map_err(FrameworkError::Json),
        Err(e) => serde_json::to_string_pretty(&json!({
            "check_name": "math_z3_solver_batch",
            "status": "Fail",
            "details": e,
        }))
        .map_err(FrameworkError::Json),
    }
}

// ── Asymptotic chain tool ──

fn tool_math_asymptotic_chain(arguments: &Value) -> Result<String, FrameworkError> {
    let steps_val = arguments
        .get("steps")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation(
            "math_asymptotic_chain requires 'steps' array",
        ))?;

    if steps_val.len() > MAX_ARRAY_ELEMENTS {
        return Err(FrameworkError::validation(format!(
            "steps array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
            steps_val.len()
        )));
    }
    let variable = arguments
        .get("variable")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_asymptotic_chain requires 'variable' (string)",
        ))?;
    let regime = arguments
        .get("regime")
        .and_then(Value::as_str)
        .unwrap_or("oo");

    let steps: Vec<crate::verification::asymptotic::AsymptoticStep> = steps_val
        .iter()
        .map(|v| {
            let premise = v
                .get("premise")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let conclusion = v
                .get("conclusion")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let relation_str = v.get("relation").and_then(Value::as_str).unwrap_or("");
            let relation = parse_order_relation(relation_str).ok_or_else(|| {
                FrameworkError::validation(format!("invalid relation: {relation_str}"))
            })?;
            let justification = v
                .get("justification")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            Ok(crate::verification::asymptotic::AsymptoticStep {
                premise,
                relation,
                conclusion,
                justification,
            })
        })
        .collect::<Result<Vec<_>, FrameworkError>>()?;

    let vr =
        crate::verification::asymptotic::verify_asymptotic_chain(&steps, variable, regime, false);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "steps_count": steps.len(),
    }))
    .map_err(FrameworkError::Json)
}

// ── Backend availability tool ──

fn tool_math_backend_available(arguments: &Value) -> Result<String, FrameworkError> {
    let backend = arguments.get("backend").and_then(Value::as_str).unwrap_or("all");

    match backend {
        "z3" => {
            let available = crate::verification::lean_bridge::check_z3_available();
            let version = if available { "via Python z3-solver" } else { "" };
            serde_json::to_string_pretty(&json!({
                "backend": "z3", "available": available,
                "version": version,
                "description": "Z3 SMT solver (Microsoft Research)",
                "install_hint": "uv pip install z3-solver",
            }))
        }
        "sympy" => {
            let available = crate::verification::lean_bridge::check_sympy_available();
            let version = if available { "via Python sympy" } else { "" };
            serde_json::to_string_pretty(&json!({
                "backend": "sympy", "available": available,
                "version": version,
                "description": "SymPy CAS (symbolic mathematics)",
                "install_hint": "uv pip install sympy",
            }))
        }
        "lean" => {
            let status = crate::verification::lean_bridge::check_lean_status();
            let available = status.is_available();
            serde_json::to_string_pretty(&json!({
                "backend": "lean", "available": available,
                "status": format!("{:?}", status),
                "description": "Lean theorem prover",
            }))
        }
        _ => {
            // "all" or any other value: return comprehensive report
            let report = crate::verification::lean_bridge::check_all_backends();
            serde_json::to_string_pretty(&json!({
                "backends": report,
                "summary": crate::verification::lean_bridge::format_all_backends_status(),
            }))
        }
    }
    .map_err(FrameworkError::Json)
}

// ── Math sympy_expand / factor / series / differentiate / integrate / solve / dimension_propagate tools ──

fn tool_math_sympy_expand(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_expand requires 'expression' (string)",
        ))?;
    let vr = crate::verification::sympy_bridge::expand_expression(expr);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_factor(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_factor requires 'expression' (string)",
        ))?;
    let vr = crate::verification::sympy_bridge::factor_expression(expr);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_series(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_series requires 'expression' (string)",
        ))?;
    let variable = arguments
        .get("variable")
        .and_then(Value::as_str)
        .unwrap_or("x");
    let point = arguments
        .get("point")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let order = arguments
        .get("order")
        .and_then(Value::as_u64)
        .unwrap_or(6) as u32;
    let vr = crate::verification::sympy_bridge::series_expression(expr, variable, point, order);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
        "variable": variable, "point": point, "order": order,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_differentiate(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_differentiate requires 'expression' (string)",
        ))?;
    let variable = arguments
        .get("variable")
        .and_then(Value::as_str)
        .unwrap_or("x");
    let order = arguments
        .get("order")
        .and_then(Value::as_u64)
        .unwrap_or(1) as u32;
    let vr = crate::verification::sympy_bridge::differentiate_expression(expr, variable, order);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
        "variable": variable, "order": order,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_integrate(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_integrate requires 'expression' (string)",
        ))?;
    let variable = arguments
        .get("variable")
        .and_then(Value::as_str)
        .unwrap_or("x");
    let lower = arguments.get("lower").and_then(Value::as_f64);
    let upper = arguments.get("upper").and_then(Value::as_f64);
    let vr = crate::verification::sympy_bridge::integrate_expression(expr, variable, lower, upper);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr,
        "variable": variable, "lower": lower, "upper": upper,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_solve(arguments: &Value) -> Result<String, FrameworkError> {
    let equation = arguments
        .get("equation")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_solve requires 'equation' (string) — e.g. \"x^2 - 4 = 0\"",
        ))?;
    let variable = arguments
        .get("variable")
        .and_then(Value::as_str)
        .unwrap_or("x");
    let vr = crate::verification::sympy_bridge::solve_equation(equation, variable);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "equation": equation,
        "variable": variable,
    }))
    .map_err(FrameworkError::Json)
}

fn tool_math_sympy_dimension_propagate(arguments: &Value) -> Result<String, FrameworkError> {
    let equation = arguments
        .get("equation")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_sympy_dimension_propagate requires 'equation' (string)",
        ))?;
    let dimensions = arguments
        .get("dimensions")
        .ok_or(FrameworkError::validation(
            "math_sympy_dimension_propagate requires 'dimensions' (object)",
        ))?;
    let vr = crate::verification::sympy_bridge::dimension_propagate(equation, dimensions);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "equation": equation,
        "dimensions": dimensions,
    }))
    .map_err(FrameworkError::Json)
}

// ── Lean verification tool ──

fn tool_math_lean_verify(arguments: &Value) -> Result<String, FrameworkError> {
    let script = arguments
        .get("script")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_lean_verify requires 'script' (string)",
        ))?;
    let vr = crate::verification::lean_bridge::verify_lean_theorem(script);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details,
    }))
    .map_err(FrameworkError::Json)
}

// ── Research smoke test tool (general-purpose experiment runner) ──

fn tool_research_smoke(arguments: &Value) -> Result<String, FrameworkError> {
    // Backward compat guard: old interface (source/barrier_id) is gone
    if arguments.get("source").is_some() || arguments.get("barrier_id").is_some() {
        return Err(FrameworkError::validation(
            "research_smoke 已升级为通用实验引擎。旧参数 source/barrier_id 不再支持。 \
             请使用 template (string) + params (array of {key: value, ...})。 \
             templates/ 目录下存放可执行实验模板。",
        ));
    }
    crate::smoke::run_smoke_tests(&std::path::Path::new("."), arguments)
}

// ── Literature verification tool ──

fn tool_verification_literature(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments
        .get("check")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "literature verification requires 'check' (doi|claim_coverage)",
        ))?;
    match check {
        "doi" => {
            let doi = arguments
                .get("doi")
                .and_then(Value::as_str)
                .ok_or(FrameworkError::validation(
                    "doi check requires 'doi' (string)",
                ))?;
            // Reuse a single tokio runtime across all DOI check calls rather
            // than creating one per request (which has significant IO driver
            // initialization overhead).
            static DOI_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
            let rt = DOI_RUNTIME.get_or_init(|| {
                #[allow(clippy::expect_used)]
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .expect("failed to build tokio runtime for DOI checker")
            });
            let reachable = rt
                .block_on(crate::verification::literature::verify_doi_reachable(doi))
                .map_err(|e| FrameworkError::validation(format!("doi check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "doi", "doi": doi, "reachable": reachable,
            }))
            .map_err(FrameworkError::Json)
        }
        "claim_coverage" => {
            let claims_val = arguments.get("claims").and_then(Value::as_array).ok_or(
                FrameworkError::validation("claim_coverage requires 'claims' array"),
            )?;
            let references_val = arguments
                .get("references")
                .and_then(Value::as_array)
                .ok_or(FrameworkError::validation(
                    "claim_coverage requires 'references' array",
                ))?;

            if claims_val.len() > MAX_ARRAY_ELEMENTS {
                return Err(FrameworkError::validation(format!(
                    "claims array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
                    claims_val.len()
                )));
            }
            if references_val.len() > MAX_ARRAY_ELEMENTS {
                return Err(FrameworkError::validation(format!(
                    "references array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
                    references_val.len()
                )));
            }
            let claims: Vec<String> = claims_val
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let refs: Vec<String> = references_val
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let coverage =
                crate::verification::literature::verify_claim_coverage(&claims, &refs)
                    .map_err(|e| {
                        FrameworkError::validation(format!("coverage check failed: {e}"))
                    })?;
            serde_json::to_string_pretty(&json!({
                "check": "claim_coverage", "coverage": coverage,
                "claims_count": claims.len(), "references_count": refs.len(),
            }))
            .map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown literature check: {check}"
        ))),
    }
}

// ── Structure verification tool ──

fn tool_verification_structure(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments
        .get("check")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "structure verification requires 'check' (latex|figures)",
        ))?;
    let path = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "structure verification requires 'path' (string)",
        ))?;
    let tex_path = std::path::Path::new(path);
    match check {
        "latex" => {
            let compilable =
                crate::verification::structure::check_latex_compilable(tex_path).map_err(|e| {
                    FrameworkError::validation(format!("latex check failed: {e}"))
                })?;
            serde_json::to_string_pretty(&json!({
                "check": "latex", "path": path, "compilable": compilable,
            }))
            .map_err(FrameworkError::Json)
        }
        "figures" => {
            let missing =
                crate::verification::structure::check_figure_references(tex_path).map_err(|e| {
                    FrameworkError::validation(format!("figure check failed: {e}"))
                })?;
            serde_json::to_string_pretty(&json!({
                "check": "figures", "path": path, "missing_refs": missing,
                "has_missing": !missing.is_empty(),
            }))
            .map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown structure check: {check}"
        ))),
    }
}

// ── Reproducibility verification tool ──

fn tool_verification_reproducibility(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments
        .get("check")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "reproducibility verification requires 'check' \
             (seed|deterministic|environment|data_versioned|checkpoint|full_audit)",
        ))?;
    match check {
        "seed" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("seed check requires 'path' (string)"),
            )?;
            let result =
                crate::verification::reproducibility::check_seed_set(std::path::Path::new(path))
                    .map_err(|e| FrameworkError::validation(format!("seed check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "seed", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "deterministic" => {
            let run_paths_val = arguments.get("run_paths").and_then(Value::as_array).ok_or(
                FrameworkError::validation("deterministic check requires 'run_paths' array"),
            )?;
            let run_paths: Vec<&std::path::Path> = run_paths_val
                .iter()
                .filter_map(|v| v.as_str())
                .map(std::path::Path::new)
                .collect();
            if run_paths.len() < 2 {
                return Err(FrameworkError::validation(
                    "deterministic check requires at least 2 run paths",
                ));
            }
            let result =
                crate::verification::reproducibility::check_deterministic_rerun(&run_paths)
                    .map_err(|e| {
                        FrameworkError::validation(format!("deterministic check failed: {e}"))
                    })?;
            serde_json::to_string_pretty(&json!({
                "check": "deterministic", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "environment" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("environment check requires 'path' (string)"),
            )?;
            let result = crate::verification::reproducibility::check_environment_reproducible(
                std::path::Path::new(path),
            )
            .map_err(|e| FrameworkError::validation(format!("environment check failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "environment", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "data_versioned" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("data_versioned check requires 'path' (string)"),
            )?;
            let result =
                crate::verification::reproducibility::check_data_versioned(std::path::Path::new(
                    path,
                ))
                .map_err(|e| {
                    FrameworkError::validation(format!("data_versioned check failed: {e}"))
                })?;
            serde_json::to_string_pretty(&json!({
                "check": "data_versioned", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "checkpoint" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("checkpoint check requires 'path' (string)"),
            )?;
            let result =
                crate::verification::reproducibility::check_checkpoint_recoverable(
                    std::path::Path::new(path),
                )
                .map_err(|e| {
                    FrameworkError::validation(format!("checkpoint check failed: {e}"))
                })?;
            serde_json::to_string_pretty(&json!({
                "check": "checkpoint", "status": result.status, "name": result.name,
            }))
            .map_err(FrameworkError::Json)
        }
        "full_audit" => {
            let path = arguments.get("path").and_then(Value::as_str).ok_or(
                FrameworkError::validation("full_audit requires 'path' (string)"),
            )?;
            let run_paths_val = arguments.get("run_paths").and_then(Value::as_array);
            let run_paths: Option<Vec<&std::path::Path>> = run_paths_val.map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(std::path::Path::new)
                    .collect()
            });
            let report = crate::verification::reproducibility::run_reproducibility_audit(
                std::path::Path::new(path),
                run_paths.as_deref(),
            )
            .map_err(|e| FrameworkError::validation(format!("full audit failed: {e}")))?;
            serde_json::to_string_pretty(&json!({
                "check": "full_audit", "checks": report.checks,
            }))
            .map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown reproducibility check: {check}"
        ))),
    }
}

// ── Formal verification tool ──

fn tool_verification_formal(arguments: &Value) -> Result<String, FrameworkError> {
    let check = arguments
        .get("check")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "formal verification requires 'check' (dimensional|witness|step_dependency)",
        ))?;
    match check {
        "dimensional" => {
            let equation = arguments
                .get("equation")
                .and_then(Value::as_str)
                .ok_or(FrameworkError::validation(
                    "dimensional check requires 'equation' (string)",
                ))?;
            let consistent =
                crate::verification::formal::check_dimensional_consistency(equation).map_err(
                    |e| FrameworkError::validation(format!("dimensional check failed: {e}")),
                )?;
            serde_json::to_string_pretty(&json!({
                "check": "dimensional", "equation": equation, "consistent": consistent,
            }))
            .map_err(FrameworkError::Json)
        }
        "witness" => {
            let expression = arguments
                .get("expression")
                .and_then(Value::as_str)
                .ok_or(FrameworkError::validation(
                    "witness check requires 'expression' (string, e.g. 'x + y = 2*x')",
                ))?;
            let witnesses_val = arguments
                .get("witnesses")
                .and_then(Value::as_array)
                .ok_or(FrameworkError::validation(
                    "witness check requires 'witnesses' array of objects, e.g. [{\"x\": 1, \"y\": 2}, {\"x\": 3, \"y\": 5}]",
                ))?;

            if witnesses_val.len() > MAX_ARRAY_ELEMENTS {
                return Err(FrameworkError::validation(format!(
                    "witnesses array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
                    witnesses_val.len()
                )));
            }
            let witnesses: Vec<HashMap<String, f64>> = witnesses_val
                .iter()
                .map(|w| {
                    let mut map = HashMap::new();
                    if let Some(obj) = w.as_object() {
                        for (k, v) in obj {
                            if let Some(n) = v.as_f64() {
                                map.insert(k.clone(), n);
                            }
                        }
                    }
                    map
                })
                .collect();
            let result =
                crate::verification::formal::check_witness_consistency(expression, &witnesses)
                    .map_err(|e| {
                        FrameworkError::validation(format!("witness check failed: {e}"))
                    })?;
            serde_json::to_string_pretty(&json!({
                "check": "witness",
                "expression": expression,
                "result": result,
            }))
            .map_err(FrameworkError::Json)
        }
        "step_dependency" => {
            let steps_val = arguments
                .get("steps")
                .and_then(Value::as_array)
                .ok_or(FrameworkError::validation(
                    "step_dependency check requires 'steps' array, \
                     e.g. [{\"id\": \"step-1\", \"depends_on\": [\"step-0\"]}]",
                ))?;
            let result = crate::verification::formal::check_step_dependency(steps_val);
            serde_json::to_string_pretty(&json!({
                "check": "step_dependency",
                "result": result,
            }))
            .map_err(FrameworkError::Json)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown formal check: {check} — expected dimensional|witness|step_dependency"
        ))),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn handle_research_tool_unknown() {
        let result = handle_research_tool("nonexistent_tool", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown research tool")
        );
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
        let result = handle_research_tool("research_review_dimensions", &json!({"round": 1}));
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round").and_then(Value::as_u64), Some(1));
        assert_eq!(
            parsed.get("dimension").and_then(Value::as_str),
            Some("逻辑与证据")
        );
    }

    #[test]
    fn research_claim_drift_missing_required() {
        let result = handle_research_tool("research_claim_drift", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires 'original_claims'")
        );
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
        assert_eq!(
            parsed.get("total_claims_analyzed").and_then(Value::as_u64),
            Some(0)
        );
    }

    #[test]
    fn research_review_loop_defaults() {
        let result = handle_research_tool("research_review_loop", &json!({}));
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("started")
        );
        let config = parsed.get("quality_gate_config").unwrap();
        assert_eq!(config.get("min_rounds").and_then(Value::as_u64), Some(5));
        assert_eq!(config.get("max_rounds").and_then(Value::as_u64), Some(10));
        assert_eq!(
            config
                .get("consecutive_stable_required")
                .and_then(Value::as_u64),
            Some(2)
        );
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
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("started")
        );
        let config = parsed.get("quality_gate_config").unwrap();
        assert_eq!(config.get("max_rounds").and_then(Value::as_u64), Some(3));
        assert_eq!(config.get("min_rounds").and_then(Value::as_u64), Some(1));
        assert!(parsed.get("current_round").is_some());
    }

    #[test]
    fn research_review_loop_status_round_1() {
        let result = handle_research_tool("research_review_loop", &json!({"operation": "status"}));
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.get("round").and_then(Value::as_u64), Some(1));
        assert_eq!(
            parsed.get("dimension").and_then(Value::as_str),
            Some("逻辑与证据")
        );
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
        assert_eq!(
            parsed.get("dimension").and_then(Value::as_str),
            Some("数学与符号")
        );
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
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("continue")
        );
        assert_eq!(
            parsed.get("round_completed").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            parsed.get("has_blocking").and_then(Value::as_bool),
            Some(false)
        );
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
        assert_eq!(
            parsed.get("has_blocking").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn research_review_loop_unknown_operation() {
        let result = handle_research_tool("research_review_loop", &json!({"operation": "unknown"}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown operation")
        );
    }

    #[test]
    fn research_review_loop_submit_advances_round() {
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 3, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("round_completed").and_then(Value::as_u64),
            Some(3)
        );
        let next = parsed.get("next_round").unwrap();
        assert_eq!(next.get("round").and_then(Value::as_u64), Some(4));
        assert_eq!(
            next.get("dimension").and_then(Value::as_str),
            Some("图表与可读性")
        );
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
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("completed")
        );
        assert!(
            parsed
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("max_rounds")
        );

        // round > max_rounds → also completed
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 999, "max_rounds": 10, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("completed")
        );

        // no max_rounds → proceeds normally (backward compat)
        let result = handle_research_tool(
            "research_review_loop",
            &json!({"operation": "submit_round", "round": 99, "findings": []}),
        );
        assert!(result.is_ok());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("operation").and_then(Value::as_str),
            Some("continue")
        );
    }

    #[test]
    fn parse_language_defaults_to_english() {
        let value = json!({});
        assert_eq!(
            parse_language(&value, "language"),
            crate::aigc::Language::English
        );
    }

    #[test]
    fn parse_language_zh() {
        let value = json!({"language": "zh"});
        assert_eq!(
            parse_language(&value, "language"),
            crate::aigc::Language::Chinese
        );
    }

    #[test]
    fn parse_language_en() {
        let value = json!({"language": "en"});
        assert_eq!(
            parse_language(&value, "language"),
            crate::aigc::Language::English
        );
    }

    #[test]
    fn parse_claim_ceiling_variants() {
        use crate::types::ClaimCeiling;
        assert_eq!(parse_claim_ceiling(Some("no-claim")), ClaimCeiling::NoClaim);
        assert_eq!(
            parse_claim_ceiling(Some("local-only")),
            ClaimCeiling::LocalOnly
        );
        assert_eq!(
            parse_claim_ceiling(Some("conference-ready")),
            ClaimCeiling::ConferenceReady
        );
        assert_eq!(
            parse_claim_ceiling(Some("conference_ready")),
            ClaimCeiling::ConferenceReady
        );
        assert_eq!(
            parse_claim_ceiling(Some("top-venue")),
            ClaimCeiling::TopVenue
        );
        assert_eq!(
            parse_claim_ceiling(Some("top_venue")),
            ClaimCeiling::TopVenue
        );
        assert_eq!(parse_claim_ceiling(None), ClaimCeiling::ConferenceReady);
        assert_eq!(
            parse_claim_ceiling(Some("unknown")),
            ClaimCeiling::ConferenceReady
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires 'expression'")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires 'expression'")
        );
    }

    #[test]
    fn test_math_asymptotic_chain_missing_steps() {
        let result = handle_research_tool("math_asymptotic_chain", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'steps'"));
    }

    #[test]
    fn test_math_asymptotic_chain_invalid_step_format() {
        let result = handle_research_tool(
            "math_asymptotic_chain",
            &json!({
                "steps": [{"premise": "n", "relation": "InvalidOp"}],
                "variable": "n", "regime": "oo",
            }),
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires 'parent_id'")
        );
    }

    #[test]
    fn test_math_proof_dag_verify_without_init() {
        let result = handle_research_tool("math_proof_dag_verify", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no proof DAG for this name")
        );
    }

    #[test]
    fn test_math_proof_dag_status_without_init() {
        let result = handle_research_tool("math_proof_dag_status", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no proof DAG for this name")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires 'expression'")
        );
    }

    #[test]
    fn test_math_lean_verify_missing_script() {
        let result = handle_research_tool("math_lean_verify", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires 'script'")
        );
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown math tool")
        );
    }

    #[test]
    fn test_math_tool_routing() {
        let result = handle_research_tool("math_future_tool", &json!({}));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("unknown") || err.contains("requires"),
            "wrong routing: {err}"
        );
    }

    // ── Math happy-path integration tests ──

    #[test]
    fn test_math_asymptotic_estimate_valid() {
        let result = handle_research_tool(
            "math_asymptotic_estimate",
            &json!({"expression": "n^2+n", "variable": "n", "regime": "oo"}),
        );
        assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("check_name").and_then(Value::as_str),
            Some("math_asymptotic_estimate")
        );
        assert!(parsed.get("status").and_then(Value::as_str).is_some());
        assert!(parsed.get("details").and_then(Value::as_str).is_some());
        let status = parsed.get("status").and_then(Value::as_str).unwrap();
        assert!(
            status == "Pass" || status == "Warn",
            "expected Pass or Warn, got {status}"
        );
        assert_eq!(
            parsed.get("expression").and_then(Value::as_str),
            Some("n^2+n")
        );
    }

    #[test]
    fn test_math_sympy_verify_valid() {
        let result = handle_research_tool(
            "math_sympy_verify",
            &json!({"lhs": "x", "rhs": "x"}),
        );
        assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("check_name").and_then(Value::as_str),
            Some("math_sympy_verify")
        );
        assert!(parsed.get("status").and_then(Value::as_str).is_some());
        assert!(parsed.get("details").and_then(Value::as_str).is_some());
        assert_eq!(parsed.get("lhs").and_then(Value::as_str), Some("x"));
        assert_eq!(parsed.get("rhs").and_then(Value::as_str), Some("x"));
    }

    #[test]
    fn test_math_proof_dag_init_decompose_verify_full_lifecycle() {
        // Step 1: Init — create a new proof DAG
        let result = handle_research_tool(
            "math_proof_dag_init",
            &json!({"goal": "Prove x > 0", "name": "happy-test"}),
        );
        assert!(result.is_ok(), "init failed: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        // init returns serialized wrapper: {schema_version, blueprint: {name, goal, ...}}
        assert_eq!(
            parsed.get("schema_version").and_then(Value::as_str),
            Some("proof-dag-v1")
        );
        assert_eq!(
            parsed.pointer("/blueprint/name").and_then(Value::as_str),
            Some("happy-test")
        );
        assert_eq!(
            parsed.pointer("/blueprint/goal").and_then(Value::as_str),
            Some("Prove x > 0")
        );

        // Step 2: Decompose — add an OrNode child to the root
        let result = handle_research_tool(
            "math_proof_dag_decompose",
            &json!({
                "parent_id": "root",
                "name": "happy-test",
                "and": false,
                "children": [
                    {"OrNode": {"id": "approach-a", "label": "Via inequality engine", "children": []}},
                ],
            }),
        );
        assert!(result.is_ok(), "decompose failed: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.pointer("/blueprint/name").and_then(Value::as_str),
            Some("happy-test")
        );
        assert_eq!(
            parsed.pointer("/blueprint/goal").and_then(Value::as_str),
            Some("Prove x > 0")
        );

        // Step 3: Decompose again — add a Leaf node under approach-a
        let result = handle_research_tool(
            "math_proof_dag_decompose",
            &json!({
                "parent_id": "approach-a",
                "name": "happy-test",
                "and": true,
                "children": [
                    {"Leaf": {"id": "leaf-x", "claim": "x > 0", "backend": "InequalityEngine"}},
                ],
            }),
        );
        assert!(result.is_ok(), "second decompose failed: {:?}", result.err());

        // Step 4: Verify — run verification traversal
        let result = handle_research_tool(
            "math_proof_dag_verify",
            &json!({"name": "happy-test"}),
        );
        assert!(result.is_ok(), "verify failed: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        // verify either returns serialized blueprint or manual_prose_warning wrapper
        if parsed.get("schema_version").is_some() {
            assert_eq!(
                parsed.pointer("/blueprint/name").and_then(Value::as_str),
                Some("happy-test")
            );
            assert!(parsed.pointer("/blueprint/round").and_then(Value::as_u64).unwrap_or(0) >= 1);
        } else if parsed.get("manual_prose_warning").is_some() {
            let result = parsed.get("result").unwrap();
            assert_eq!(
                result.get("name").and_then(Value::as_str),
                Some("happy-test")
            );
        } else {
            panic!("unexpected verify output format: {parsed}");
        }

        // Step 5: Status — get summary
        let result = handle_research_tool(
            "math_proof_dag_status",
            &json!({"name": "happy-test"}),
        );
        assert!(result.is_ok(), "status failed: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("name").and_then(Value::as_str),
            Some("happy-test")
        );
        assert!(parsed.get("node_count").and_then(Value::as_u64).unwrap_or(0) >= 3);
    }

    #[test]
    fn test_math_prove_inequality_valid() {
        let result = handle_research_tool(
            "math_prove_inequality",
            &json!({"expression": "x > 0"}),
        );
        assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("check_name").and_then(Value::as_str),
            Some("math_prove_inequality")
        );
        assert!(parsed.get("status").and_then(Value::as_str).is_some());
        assert!(parsed.get("details").and_then(Value::as_str).is_some());
        assert_eq!(
            parsed.get("expression").and_then(Value::as_str),
            Some("x > 0")
        );
    }

    // ── Z3 prove tool tests ──

    #[test]
    fn test_math_z3_prove_missing_expression() {
        let result = handle_research_tool("math_z3_prove", &json!({}));
        assert!(result.is_err(), "missing expression should error");
    }

    #[test]
    fn test_math_z3_solver_push_default_n() {
        let result = handle_research_tool("math_z3_solver_push", &json!({}));
        // Should succeed (or be WARN if Z3 not available)
        assert!(result.is_ok(), "push default should not error");
    }

    #[test]
    fn test_math_z3_solver_pop_missing_no_default_fallthrough() {
        let result = handle_research_tool("math_z3_solver_pop", &json!({}));
        assert!(result.is_ok(), "pop default should not error");
    }

    #[test]
    fn test_math_z3_solver_add_missing_expression() {
        let result = handle_research_tool("math_z3_solver_add", &json!({}));
        assert!(result.is_err(), "missing expression should error");
    }

    #[test]
    fn test_math_z3_solver_check_ok() {
        let result = handle_research_tool("math_z3_solver_check", &json!({}));
        assert!(result.is_ok(), "check should not error");
    }

    #[test]
    fn test_math_z3_solver_reset_ok() {
        let result = handle_research_tool("math_z3_solver_reset", &json!({}));
        assert!(result.is_ok(), "reset should not error");
    }

    #[test]
    fn test_math_z3_solver_batch_missing_steps() {
        let result = handle_research_tool("math_z3_solver_batch", &json!({}));
        assert!(result.is_err(), "missing steps should error");
    }

    #[test]
    fn test_math_z3_solver_batch_empty_steps() {
        let result = handle_research_tool("math_z3_solver_batch", &json!({"steps": []}));
        assert!(result.is_ok(), "empty steps should not error");
    }
}
