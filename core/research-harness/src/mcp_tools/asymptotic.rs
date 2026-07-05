//! Asymptotic analysis MCP tools.
//!
//! Each function in this module is `pub(super)` — visible only to the parent
//! `mcp_tools` module (i.e. `mod.rs`), which routes calls via
//! [`math_tool_dispatch`](super::math_tool_dispatch).

use core_errors::FrameworkError;
use serde_json::{Value, json};

/// Maximum number of elements in any array-type parameter to a research tool.
const MAX_ARRAY_ELEMENTS: usize = 10_000;

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

pub(super) fn tool_math_asymptotic_estimate(arguments: &Value) -> Result<String, FrameworkError> {
    let expr = arguments
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

pub(super) fn tool_math_asymptotic_chain(arguments: &Value) -> Result<String, FrameworkError> {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::super::handle_research_tool;
    use serde_json::{Value, json};

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
}
