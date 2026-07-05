//! Z3 solver and Lean verification MCP tools.
//!
//! Each function in this module is `pub(super)` — visible only to the parent
//! `mcp_tools` module (i.e. `mod.rs`), which routes calls via
//! [`math_tool_dispatch`](super::math_tool_dispatch).

use core_errors::FrameworkError;
use serde_json::{Value, json};

/// Maximum number of elements in any array-type parameter to a research tool.
/// Prevents single-call memory exhaustion via malicious oversized arrays.
const MAX_ARRAY_ELEMENTS: usize = 10_000;

pub(super) fn tool_math_z3_prove(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_z3_solver_push(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_z3_solver_pop(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_z3_solver_add(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_z3_solver_check(arguments: &Value) -> Result<String, FrameworkError> {
    let timeout_ms = arguments.get("timeout_ms").and_then(Value::as_u64);
    let vr = crate::verification::z3_bridge::solver_check(timeout_ms);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "timeout_ms": timeout_ms,
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_z3_solver_reset(_arguments: &Value) -> Result<String, FrameworkError> {
    let vr = crate::verification::z3_bridge::solver_reset();
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details,
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_z3_solver_batch(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_z3_optimize(arguments: &Value) -> Result<String, FrameworkError> {
    let objective = arguments
        .get("objective")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_z3_optimize requires 'objective' (string)",
        ))?;
    let constraints: Vec<String> = arguments
        .get("constraints")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let direction = arguments
        .get("direction")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_z3_optimize requires 'direction' (string: minimize/maximize)",
        ))?;
    match crate::verification::z3_bridge::optimize_formula(objective, &constraints, None, direction) {
        Ok(result) => serde_json::to_string_pretty(&json!({
            "check_name": "math_z3_optimize",
            "status": "Pass",
            "result": result,
            "objective": objective,
            "direction": direction,
        })).map_err(FrameworkError::Json),
        Err(e) => serde_json::to_string_pretty(&json!({
            "check_name": "math_z3_optimize",
            "status": "Fail",
            "details": e,
        })).map_err(FrameworkError::Json),
    }
}

pub(super) fn tool_math_z3_check_system(arguments: &Value) -> Result<String, FrameworkError> {
    let constraints: Vec<String> = arguments
        .get("constraints")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .ok_or(FrameworkError::validation(
            "math_z3_check_system requires 'constraints' (array of strings)",
        ))?;
    let timeout_ms = arguments.get("timeout_ms").and_then(Value::as_u64);
    match crate::verification::z3_bridge::check_system(&constraints, None, timeout_ms) {
        Ok(result) => serde_json::to_string_pretty(&json!({
            "check_name": "math_z3_check_system",
            "status": "Pass",
            "result": result,
        })).map_err(FrameworkError::Json),
        Err(e) => serde_json::to_string_pretty(&json!({
            "check_name": "math_z3_check_system",
            "status": "Fail",
            "details": e,
        })).map_err(FrameworkError::Json),
    }
}

pub(super) fn tool_math_backend_available(arguments: &Value) -> Result<String, FrameworkError> {
    let backend = arguments.get("backend").and_then(Value::as_str).unwrap_or("all");

    match backend {
        "z3" => {
            let available = crate::verification::z3_bridge::z3_available();
            let version = if available { "native z3 crate" } else { "" };
            serde_json::to_string_pretty(&json!({
                "backend": "z3", "available": available,
                "version": version,
                "description": "Z3 SMT solver (Microsoft Research)",
                "install_hint": "bundled via z3 crate",
            }))
        }
        "sympy" => {
            // Pure Rust symbolic engine — always available
            serde_json::to_string_pretty(&json!({
                "backend": "sympy", "available": true,
                "version": "pure Rust symbolic engine",
                "description": "Symbolic mathematics (pure Rust, no Python dependency)",
                "install_hint": "built-in",
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

pub(super) fn tool_math_lean_verify(arguments: &Value) -> Result<String, FrameworkError> {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::super::handle_research_tool;
    use serde_json::json;

    #[test]
    fn test_math_z3_prove_missing_expression() {
        let result = handle_research_tool("math_z3_prove", &json!({}));
        assert!(result.is_err(), "missing expression should error");
    }

    #[test]
    fn test_math_z3_solver_push_default_n() {
        let result = handle_research_tool("math_z3_solver_push", &json!({}));
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
}
