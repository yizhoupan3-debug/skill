//! SymPy bridge MCP tools.
//!
//! Each function in this module is `pub(super)` — visible only to the parent
//! `mcp_tools` module (i.e. `mod.rs`), which routes calls via
//! [`math_tool_dispatch`](super::math_tool_dispatch).

use core_errors::FrameworkError;
use serde_json::{Value, json};

pub(super) fn tool_math_sympy_verify(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_simplify(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
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
    let vr = crate::verification::sympy_bridge::simplify_expression_with_assumptions(expr, &assumptions);
    serde_json::to_string_pretty(&json!({
        "check_name": vr.check_name, "status": format!("{:?}", vr.status),
        "details": vr.details, "expression": expr, "assumptions": assumptions,
    }))
    .map_err(FrameworkError::Json)
}

pub(super) fn tool_math_sympy_trig_simplify(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
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

pub(super) fn tool_math_sympy_subs(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_limit(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_lambdify(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_expand(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_factor(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_series(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_differentiate(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_integrate(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_solve(arguments: &Value) -> Result<String, FrameworkError> {
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

pub(super) fn tool_math_sympy_dimension_propagate(arguments: &Value) -> Result<String, FrameworkError> {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::super::handle_research_tool;
    use serde_json::{Value, json};

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
}
