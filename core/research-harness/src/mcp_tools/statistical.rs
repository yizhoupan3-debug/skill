//! Statistical verification tool — GRIM test, p-value verification, multiple comparison correction.
//!
//! Extracted from `mod.rs` — called via `verification_tool_dispatch` in the parent module.

use crate::verification::*;
use core_errors::FrameworkError;
use serde_json::{Value, json};

pub(super) fn tool_verification_statistical(arguments: &Value) -> Result<String, FrameworkError> {
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

#[cfg(test)]
mod tests {
    use super::super::handle_research_tool;
    use serde_json::json;
}
