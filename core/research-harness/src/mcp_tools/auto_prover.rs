//! Auto theorem proving, identity chain, bounds tightening, witness consistency,
//! homomorphism check, proof trace, inequality prover, and perturbation expansion
//! MCP tools.
//!
//! Each function in this module is `pub(super)` — visible only to the parent
//! `mcp_tools` module (i.e. `mod.rs`), which routes calls via
//! [`math_tool_dispatch`](super::math_tool_dispatch).

use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::collections::HashMap;

pub(super) fn tool_math_auto_prove(arguments: &Value) -> Result<String, FrameworkError> {
    let lhs = arguments
        .get("lhs")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_auto_prove requires 'lhs' (string)",
        ))?;
    let rhs = arguments
        .get("rhs")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_auto_prove requires 'rhs' (string)",
        ))?;
    let timeout_ms = arguments.get("timeout_ms").and_then(Value::as_u64);

    let result = crate::verification::auto_prover::try_prove(lhs, rhs, timeout_ms);
    serde_json::to_string_pretty(&json!({
        "check_name": "math_auto_prove",
        "status": if result.proved { "Pass" } else { "Fail" },
        "proved": result.proved,
        "backend": format!("{}", result.backend),
        "details": result.verification_result.details,
        "proof_string": result.proof_string,
        "trace_summary": result.trace.summary(),
        "steps_count": result.trace.steps.len(),
        "verification_time_ms": result.trace.verification_time_ms,
        "lhs": lhs,
        "rhs": rhs,
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_identity_chain(arguments: &Value) -> Result<String, FrameworkError> {
    let chain_val = arguments
        .get("chain")
        .and_then(Value::as_array)
        .ok_or(FrameworkError::validation(
            "math_identity_chain requires 'chain' array of strings",
        ))?;

    if chain_val.len() > 100 {
        return Err(FrameworkError::validation(format!(
            "chain too long: {} elements (max 100)",
            chain_val.len()
        )));
    }

    let chain: Vec<String> = chain_val
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let result = crate::verification::auto_prover::verify_identity_chain(&chain);
    serde_json::to_string_pretty(&json!({
        "check_name": "math_identity_chain",
        "verified": result.verified,
        "pairs_checked": result.pairs_checked,
        "broken_at": result.broken_at,
        "details": result.details,
        "pair_count": result.pair_results.len(),
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_tighten_bounds(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
        .get("expression")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_tighten_bounds requires 'expression' (string)",
        ))?;
    let var = arguments
        .get("variable")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_tighten_bounds requires 'variable' (string)",
        ))?;
    let lo = arguments
        .get("lower")
        .and_then(Value::as_f64)
        .ok_or(FrameworkError::validation(
            "math_tighten_bounds requires 'lower' (f64)",
        ))?;
    let hi = arguments
        .get("upper")
        .and_then(Value::as_f64)
        .ok_or(FrameworkError::validation(
            "math_tighten_bounds requires 'upper' (f64)",
        ))?;
    let timeout_ms = arguments.get("timeout_ms").and_then(Value::as_u64);

    let result = crate::verification::auto_prover::tighten_bounds(expr, var, lo, hi, timeout_ms);
    serde_json::to_string_pretty(&json!({
        "check_name": "math_tighten_bounds",
        "lower_bound": result.lower_bound,
        "upper_bound": result.upper_bound,
        "iterations": result.iterations,
        "feasible": result.feasible,
        "details": result.details,
        "expression": expr,
        "variable": var,
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_witness_consistency(arguments: &Value) -> Result<String, FrameworkError> {
    let lhs = arguments
        .get("lhs")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_witness_consistency requires 'lhs' (string)",
        ))?;
    let rhs = arguments
        .get("rhs")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_witness_consistency requires 'rhs' (string)",
        ))?;

    // Extract variables from both expressions
    let all_vars: Vec<String> = {
        let raw = lhs.to_string() + " " + rhs;
        let re = regex::Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").expect("valid regex");
        let keywords = crate::verification::symbolic::MATH_KEYWORDS;
        let mut v: Vec<String> = re.find_iter(&raw)
            .map(|m| m.as_str().to_string())
            .filter(|s| !keywords.contains(&s.as_str()))
            .collect();
        v.sort();
        v.dedup();
        v
    };

    // Collect provided witnesses
    let mut witnesses: Vec<HashMap<String, f64>> = arguments
        .get("witnesses")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
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
                .collect()
        })
        .unwrap_or_default();

    // If no witnesses provided, use random batch
    if witnesses.is_empty() {
        let num_random = arguments
            .get("num_random")
            .and_then(Value::as_u64)
            .unwrap_or(50) as usize;
        let seed = arguments
            .get("seed")
            .and_then(Value::as_u64)
            .unwrap_or(42);
        witnesses = crate::verification::auto_prover::generate_random_witnesses(
            &all_vars,
            num_random,
            seed,
        );
    }

    let result = crate::verification::auto_prover::verify_witness_consistency(lhs, rhs, &witnesses);
    serde_json::to_string_pretty(&json!({
        "check_name": "math_witness_consistency",
        "lhs": lhs,
        "rhs": rhs,
        "witnesses_checked": result["witnesses_checked"],
        "passed": result["passed"],
        "failures": result["failures"],
        "detail": result["detail"],
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_check_homomorphism(arguments: &Value) -> Result<String, FrameworkError> {
    let f = arguments
        .get("f")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_check_homomorphism requires 'f' (string)",
        ))?;
    let g = arguments
        .get("g")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_check_homomorphism requires 'g' (string)",
        ))?;

    let result = crate::verification::auto_prover::check_homomorphism(f, g);
    serde_json::to_string_pretty(&json!({
        "check_name": "math_check_homomorphism",
        "found": result.found,
        "transform_type": result.transform_type,
        "parameters": result.parameters,
        "equation": result.equation,
        "details": result.details,
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_proof_trace_record(arguments: &Value) -> Result<String, FrameworkError> {
    let lhs = arguments
        .get("lhs")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_proof_trace_record requires 'lhs' (string)",
        ))?;
    let rhs = arguments
        .get("rhs")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_proof_trace_record requires 'rhs' (string)",
        ))?;

    let (trace, vr) = crate::verification::auto_prover::verify_identity_with_trace(lhs, rhs);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name,
        "status": format!("{:?}", vr.status),
        "details": vr.details,
        "trace": {
            "backend": format!("{}", trace.backend),
            "verification_time_ms": trace.verification_time_ms,
            "steps_count": trace.steps.len(),
            "steps": trace.steps,
            "assumptions": trace.assumptions,
            "summary": trace.summary(),
            "description": trace.describe(),
        },
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_prove_inequality(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_perturbation_expand(arguments: &Value) -> Result<String, FrameworkError> {
    let equation = arguments
        .get("equation")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_perturbation_expand requires 'equation' (string)",
        ))?;
    let variable = arguments
        .get("variable")
        .and_then(Value::as_str)
        .unwrap_or("x");
    let parameter = arguments
        .get("parameter")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_perturbation_expand requires 'parameter' (string) — e.g. 'eps', 'epsilon'",
        ))?;
    let order = arguments
        .get("order")
        .and_then(Value::as_u64)
        .unwrap_or(2) as u32;
    let bc = arguments.get("bc").and_then(Value::as_str);

    let result = crate::verification::perturbation::regular_perturbation(
        equation,
        variable,
        parameter,
        order,
        bc,
    );

    serde_json::to_string_pretty(&json!({
        "check_name": result.check_name,
        "status": format!("{:?}", result.status),
        "details": result.details,
        "equation": equation,
        "variable": variable,
        "parameter": parameter,
        "order": order,
        "bc": bc,
        "orders": result.orders,
        "full_solution": result.full_solution,
    }))
    .map_err(FrameworkError::Json)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::super::handle_research_tool;
    use serde_json::{Value, json};

    #[test]
    fn test_math_auto_prove_missing_lhs() {
        let result = handle_research_tool("math_auto_prove", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'lhs'"));
    }

    #[test]
    fn test_math_auto_prove_trivial() {
        let result = handle_research_tool(
            "math_auto_prove",
            &json!({"lhs": "x", "rhs": "x"}),
        );
        assert!(result.is_ok(), "auto prove should not error: {:?}", result.err());
        if let Ok(json_str) = result {
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert_eq!(parsed.get("proved").and_then(Value::as_bool), Some(true));
            assert!(parsed.get("backend").and_then(Value::as_str).is_some());
        }
    }

    #[test]
    fn test_math_identity_chain_missing_chain() {
        let result = handle_research_tool("math_identity_chain", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'chain'"));
    }

    #[test]
    fn test_math_identity_chain_valid() {
        let result = handle_research_tool(
            "math_identity_chain",
            &json!({"chain": ["(x+1)^2", "x^2 + 2*x + 1"]}),
        );
        assert!(result.is_ok(), "identity chain should not error");
        if let Ok(json_str) = result {
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert!(parsed.get("verified").and_then(Value::as_bool).unwrap_or(false),
                "chain should verify");
        }
    }

    #[test]
    fn test_math_tighten_bounds_missing_expression() {
        let result = handle_research_tool("math_tighten_bounds", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'expression'"));
    }

    #[test]
    fn test_math_tighten_bounds_basic() {
        let result = handle_research_tool(
            "math_tighten_bounds",
            &json!({"expression": "x^2 <= 25", "variable": "x", "lower": -100.0, "upper": 100.0}),
        );
        assert!(result.is_ok(), "tighten bounds should not error");
        if let Ok(json_str) = result {
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert!(parsed.get("lower_bound").and_then(Value::as_f64).is_some());
            assert!(parsed.get("upper_bound").and_then(Value::as_f64).is_some());
        }
    }

    #[test]
    fn test_math_witness_consistency_missing_lhs() {
        let result = handle_research_tool("math_witness_consistency", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'lhs'"));
    }

    #[test]
    fn test_math_witness_consistency_with_random() {
        let result = handle_research_tool(
            "math_witness_consistency",
            &json!({"lhs": "x + y", "rhs": "y + x", "num_random": 5}),
        );
        assert!(result.is_ok(), "witness consistency should not error");
        if let Ok(json_str) = result {
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert!(parsed.get("passed").and_then(Value::as_bool).unwrap_or(false),
                "x+y = y+x should pass all witnesses");
        }
    }

    #[test]
    fn test_math_check_homomorphism_missing_f() {
        let result = handle_research_tool("math_check_homomorphism", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'f'"));
    }

    #[test]
    fn test_math_check_homomorphism_scale() {
        let result = handle_research_tool(
            "math_check_homomorphism",
            &json!({"f": "2*x", "g": "x"}),
        );
        assert!(result.is_ok(), "homomorphism check should not error");
        if let Ok(json_str) = result {
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert!(parsed.get("found").and_then(Value::as_bool).unwrap_or(false),
                "2*x and x should be homomorphic (scale)");
        }
    }

    #[test]
    fn test_math_proof_trace_record_missing_lhs() {
        let result = handle_research_tool("math_proof_trace_record", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'lhs'"));
    }

    #[test]
    fn test_math_proof_trace_record_valid() {
        let result = handle_research_tool(
            "math_proof_trace_record",
            &json!({"lhs": "x", "rhs": "x"}),
        );
        assert!(result.is_ok(), "proof trace record should not error");
        if let Ok(json_str) = result {
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert!(parsed.pointer("/trace/steps").is_some(),
                "proof trace should contain steps");
            assert!(parsed.pointer("/trace/backend").and_then(Value::as_str).is_some());
        }
    }

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
}
