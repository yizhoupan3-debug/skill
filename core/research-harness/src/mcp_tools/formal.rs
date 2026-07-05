//! Formal verification tool — dimensional consistency, witness consistency, step dependency.
//!
//! Extracted from `mod.rs` — called via `verification_tool_dispatch` in the parent module.

use crate::verification::*;
use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Maximum number of elements in any array-type parameter to a research tool.
/// Prevents single-call memory exhaustion via malicious oversized arrays.
const MAX_ARRAY_ELEMENTS: usize = 10_000;

pub(super) fn tool_verification_formal(arguments: &Value) -> Result<String, FrameworkError> {
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
    use super::super::handle_research_tool;
    use serde_json::json;
}
