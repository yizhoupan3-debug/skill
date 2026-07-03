//! Z3 operations — sort-inference, prove, solver batch.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! Uses real Z3 via Python subprocess (z3-solver) when available.
//! All operations are delegated to `python_bridge::call_math_backend`.
//!
//! # Cross-call state
//!
//! Each call to `call_math_backend` spawns a new Python subprocess. The
//! individual `solver_push` / `solver_pop` / `solver_add` / `solver_check`
//! operations work within a single subprocess but do NOT share state across
//! separate Rust calls. For truly incremental solving, use `solver_batch`
//! which sends multiple steps to the same Python process.

use crate::types::{VerificationResult, VerificationStatus};
use crate::verification::python_bridge;
use serde_json::json;

/// Call the Z3 `prove` convenience wrapper via the Python backend.
///
/// `z3.Prove(expr)` checks if a formula is universally valid by verifying
/// that its negation is unsatisfiable.
pub fn prove_formula(expr: &str) -> VerificationResult {
    if !python_bridge::z3_available() {
        return VerificationResult {
            check_name: "math_z3_prove".into(),
            status: VerificationStatus::Fail,
            details: format!(
                "z3_prove({expr}) — Z3 not available (requires z3-solver)"
            ),
            evidence_path: None,
        };
    }

    let params = json!({
        "expression": expr,
        "timeout_ms": 10000,
    });

    match python_bridge::call_math_backend("z3_prove", params) {
        Ok(result) => {
            let proved = result
                .get("proved")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let result_str = result
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            if proved {
                VerificationResult {
                    check_name: "math_z3_prove".into(),
                    status: VerificationStatus::Pass,
                    details: format!("z3_prove({expr}) — proved (formula is universally valid)"),
                    evidence_path: None,
                }
            } else {
                let counterexample = result
                    .get("counterexample")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .map(|(k, v)| format!("{k}={v}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();

                let detail = if !counterexample.is_empty() {
                    format!("z3_prove({expr}) — disproved. Counterexample: {{{counterexample}}}")
                } else {
                    format!("z3_prove({expr}) — {result_str}")
                };

                VerificationResult {
                    check_name: "math_z3_prove".into(),
                    status: VerificationStatus::Fail,
                    details: detail,
                    evidence_path: None,
                }
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_z3_prove".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_prove({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// A single step in a solver batch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SolverBatchStep {
    /// One of: "push", "pop", "add", "check", "reset"
    pub action: String,
    /// For push/pop: number of scopes (optional, default 1)
    pub n: Option<usize>,
    /// For add: the constraint expression
    pub expression: Option<String>,
    /// For check: timeout in ms (optional, default 5000)
    pub timeout_ms: Option<u64>,
}

/// Run a batch of incremental solver steps in a single Python process.
///
/// This is the recommended way to use incremental constraint solving
/// with push/pop, as all steps share the same solver instance.
///
/// Returns a JSON value with a `steps` array, each containing the
/// individual step result.
pub fn solver_batch(steps: &[SolverBatchStep]) -> Result<serde_json::Value, String> {
    if !python_bridge::z3_available() {
        return Err("Z3 not available (requires z3-solver)".into());
    }

    let batch_steps: Vec<serde_json::Value> = steps
        .iter()
        .map(|s| {
            let mut step = json!({
                "action": s.action,
            });
            if let Some(n) = s.n {
                step["n"] = json!(n);
            }
            if let Some(ref expr) = s.expression {
                step["expression"] = json!(expr);
            }
            if let Some(ms) = s.timeout_ms {
                step["timeout_ms"] = json!(ms);
            }
            step
        })
        .collect();

    let params = json!({
        "steps": batch_steps,
    });

    match python_bridge::call_math_backend("z3_solver_batch", params) {
        Ok(result) => Ok(result),
        Err(e) => Err(format!("z3_solver_batch failed: {e}")),
    }
}

/// Push a new context onto the solver stack (single-step, fresh solver each call).
pub fn solver_push(n: usize) -> VerificationResult {
    if !python_bridge::z3_available() {
        return VerificationResult {
            check_name: "math_z3_solver_push".into(),
            status: VerificationStatus::Fail,
            details: "z3_solver_push — Z3 not available (requires z3-solver)".into(),
            evidence_path: None,
        };
    }

    let params = json!({"n": n});
    match python_bridge::call_math_backend("z3_solver_push", params) {
        Ok(_) => VerificationResult {
            check_name: "math_z3_solver_push".into(),
            status: VerificationStatus::Pass,
            details: format!("z3_solver_push — pushed {n} context(s)"),
            evidence_path: None,
        },
        Err(e) => VerificationResult {
            check_name: "math_z3_solver_push".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_solver_push failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Pop contexts from the solver stack (single-step, fresh solver each call).
pub fn solver_pop(n: usize) -> VerificationResult {
    if !python_bridge::z3_available() {
        return VerificationResult {
            check_name: "math_z3_solver_pop".into(),
            status: VerificationStatus::Fail,
            details: "z3_solver_pop — Z3 not available (requires z3-solver)".into(),
            evidence_path: None,
        };
    }

    let params = json!({"n": n});
    match python_bridge::call_math_backend("z3_solver_pop", params) {
        Ok(_) => VerificationResult {
            check_name: "math_z3_solver_pop".into(),
            status: VerificationStatus::Pass,
            details: format!("z3_solver_pop — popped {n} context(s)"),
            evidence_path: None,
        },
        Err(e) => VerificationResult {
            check_name: "math_z3_solver_pop".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_solver_pop failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Add a constraint expression to the solver (single-step, fresh solver each call).
pub fn solver_add(expr: &str) -> VerificationResult {
    if !python_bridge::z3_available() {
        return VerificationResult {
            check_name: "math_z3_solver_add".into(),
            status: VerificationStatus::Fail,
            details: format!(
                "z3_solver_add({expr}) — Z3 not available (requires z3-solver)"
            ),
            evidence_path: None,
        };
    }

    let params = json!({"expression": expr});
    match python_bridge::call_math_backend("z3_solver_add", params) {
        Ok(_) => VerificationResult {
            check_name: "math_z3_solver_add".into(),
            status: VerificationStatus::Pass,
            details: format!("z3_solver_add({expr}) — constraint added"),
            evidence_path: None,
        },
        Err(e) => VerificationResult {
            check_name: "math_z3_solver_add".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_solver_add({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Check satisfiability of the solver (single-step, fresh solver each call).
pub fn solver_check(timeout_ms: Option<u64>) -> VerificationResult {
    if !python_bridge::z3_available() {
        return VerificationResult {
            check_name: "math_z3_solver_check".into(),
            status: VerificationStatus::Fail,
            details: "z3_solver_check — Z3 not available (requires z3-solver)".into(),
            evidence_path: None,
        };
    }

    let mut params = json!({});
    if let Some(ms) = timeout_ms {
        params["timeout_ms"] = json!(ms);
    }

    match python_bridge::call_math_backend("z3_solver_check", params) {
        Ok(result) => {
            let status_str = result
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            match status_str {
                "sat" => {
                    let model = result
                        .get("model")
                        .and_then(|v| v.as_object())
                        .map(|obj| {
                            obj.iter()
                                .map(|(k, v)| format!("{k}={v}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();

                    VerificationResult {
                        check_name: "math_z3_solver_check".into(),
                        status: VerificationStatus::Pass,
                        details: format!(
                            "z3_solver_check — SAT. Model: {{{model}}}"
                        ),
                        evidence_path: None,
                    }
                }
                "unsat" => VerificationResult {
                    check_name: "math_z3_solver_check".into(),
                    status: VerificationStatus::Fail,
                    details: "z3_solver_check — UNSAT (no solution)".into(),
                    evidence_path: None,
                },
                _ => VerificationResult {
                    check_name: "math_z3_solver_check".into(),
                    status: VerificationStatus::Warn,
                    details: format!("z3_solver_check — {status_str}"),
                    evidence_path: None,
                },
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_z3_solver_check".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_solver_check failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Reset the persistent Z3 solver.
pub fn solver_reset() -> VerificationResult {
    if !python_bridge::z3_available() {
        return VerificationResult {
            check_name: "math_z3_solver_reset".into(),
            status: VerificationStatus::Fail,
            details: "z3_solver_reset — Z3 not available (requires z3-solver)".into(),
            evidence_path: None,
        };
    }

    match python_bridge::call_math_backend("z3_solver_reset", json!({})) {
        Ok(_) => VerificationResult {
            check_name: "math_z3_solver_reset".into(),
            status: VerificationStatus::Pass,
            details: "z3_solver_reset — solver cleared".into(),
            evidence_path: None,
        },
        Err(e) => VerificationResult {
            check_name: "math_z3_solver_reset".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_solver_reset failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Optimize an objective function subject to constraints using Z3.
///
/// Calls Python z3_ops.z3_optimize to perform optimization.
/// Supports both minimize and maximize with multiple constraints.
pub fn optimize_formula(
    objective: &str,
    constraints: &[String],
    variables: Option<&[String]>,
    direction: &str,
) -> Result<serde_json::Value, String> {
    if !python_bridge::z3_available() {
        return Err("Z3 not available (requires z3-solver)".into());
    }

    let mut params = json!({
        "objective": objective,
        "constraints": constraints,
        "direction": direction,
    });
    if let Some(vars) = variables {
        params["variables"] = json!(vars);
    }

    match python_bridge::call_math_backend("z3_optimize", params) {
        Ok(result) => Ok(result),
        Err(e) => Err(format!("z3_optimize failed: {e}")),
    }
}

/// Check a system of constraints for satisfiability using Z3.
///
/// Calls Python z3_ops.z3_check_system to check multiple constraints together.
/// Supports automatic variable detection and sort inference.
pub fn check_system(
    constraints: &[String],
    variables: Option<&[String]>,
    timeout_ms: Option<u64>,
) -> Result<serde_json::Value, String> {
    if !python_bridge::z3_available() {
        return Err("Z3 not available (requires z3-solver)".into());
    }

    let mut params = json!({
        "constraints": constraints,
    });
    if let Some(vars) = variables {
        params["variables"] = json!(vars);
    }
    if let Some(timeout) = timeout_ms {
        params["timeout_ms"] = json!(timeout);
    }

    match python_bridge::call_math_backend("z3_check_system", params) {
        Ok(result) => Ok(result),
        Err(e) => Err(format!("z3_check_system failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_z3_probe() {
        // Should not panic
        let _ = python_bridge::z3_available();
    }

    // Individual solver_push / solver_pop / solver_add / solver_check
    // operations each start a fresh Python subprocess, so they don't
    // share state across separate function calls. For cross-step
    // incremental testing, use solver_batch instead.

    #[test]
    fn test_solver_push_single() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping");
            return;
        }
        // Push creates a fresh solver and pushes — should succeed
        let r = solver_push(1);
        assert_eq!(r.status, VerificationStatus::Pass, "push: {}", r.details);
    }

    #[test]
    fn test_solver_pop_single() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping");
            return;
        }
        // Pop on a fresh solver will error (no push yet) — we just
        // verify the operation doesn't panic
        let _ = solver_pop(1);
    }

    #[test]
    fn test_solver_add_single() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping");
            return;
        }
        let r = solver_add("x > 0");
        assert_eq!(r.status, VerificationStatus::Pass, "add: {}", r.details);
    }

    #[test]
    fn test_solver_check_single() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping");
            return;
        }
        // Check on an empty fresh solver should be SAT
        let r = solver_check(Some(5000));
        assert_eq!(r.status, VerificationStatus::Pass, "check empty: {}", r.details);
    }

    #[test]
    fn test_solver_reset_single() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping");
            return;
        }
        let r = solver_reset();
        assert_eq!(r.status, VerificationStatus::Pass, "reset: {}", r.details);
    }

    #[test]
    fn test_solver_batch_push_add_check_pop() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping batch test");
            return;
        }

        let steps = vec![
            SolverBatchStep { action: "reset".into(), n: None, expression: None, timeout_ms: None },
            SolverBatchStep { action: "push".into(), n: Some(1), expression: None, timeout_ms: None },
            SolverBatchStep {
                action: "add".into(),
                n: None,
                expression: Some("x > 0".into()),
                timeout_ms: None,
            },
            SolverBatchStep { action: "check".into(), n: None, expression: None, timeout_ms: None },
            SolverBatchStep { action: "pop".into(), n: Some(1), expression: None, timeout_ms: None },
        ];

        let result = solver_batch(&steps);
        assert!(result.is_ok(), "batch should succeed: {:?}", result.err());
        let parsed = result.unwrap();
        let steps_result = parsed.get("steps").and_then(|v| v.as_array());
        assert!(steps_result.is_some(), "batch response should have steps array");
        let steps = steps_result.unwrap();
        assert_eq!(steps.len(), 5, "should have 5 step results");

        // reset
        assert_eq!(
            steps[0].get("result").and_then(|v| v.as_str()),
            Some("ok"),
            "reset should be ok"
        );
        // push
        assert_eq!(
            steps[1].get("result").and_then(|v| v.as_str()),
            Some("ok"),
            "push should be ok"
        );
        // add
        assert_eq!(
            steps[2].get("result").and_then(|v| v.as_str()),
            Some("ok"),
            "add should be ok"
        );
        // check — should be SAT
        assert_eq!(
            steps[3].get("result").and_then(|v| v.as_str()),
            Some("sat"),
            "x > 0 should be SAT"
        );
        // pop
        assert_eq!(
            steps[4].get("result").and_then(|v| v.as_str()),
            Some("ok"),
            "pop should be ok"
        );
    }

    #[test]
    fn test_solver_batch_unsat() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping unsat test");
            return;
        }

        // x > 5 AND x < 0 should be UNSAT
        let steps = vec![
            SolverBatchStep {
                action: "add".into(),
                n: None,
                expression: Some("x > 5".into()),
                timeout_ms: None,
            },
            SolverBatchStep {
                action: "add".into(),
                n: None,
                expression: Some("x < 0".into()),
                timeout_ms: None,
            },
            SolverBatchStep {
                action: "check".into(),
                n: None,
                expression: None,
                timeout_ms: Some(5000),
            },
        ];

        let result = solver_batch(&steps);
        assert!(result.is_ok(), "batch should succeed");
        let parsed = result.unwrap();
        let steps_result = parsed
            .get("steps")
            .and_then(|v| v.as_array())
            .expect("should have steps");

        // Check result of the third step (the check)
        assert_eq!(
            steps_result[2]
                .get("result")
                .and_then(|v| v.as_str()),
            Some("unsat"),
            "x > 5 AND x < 0 should be UNSAT"
        );
    }

    #[test]
    fn test_solver_batch_check_sat_with_model() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping model test");
            return;
        }

        // x >= 0 AND x <= 10 should be SAT with a model
        let steps = vec![
            SolverBatchStep {
                action: "add".into(),
                n: None,
                expression: Some("x >= 0".into()),
                timeout_ms: None,
            },
            SolverBatchStep {
                action: "add".into(),
                n: None,
                expression: Some("x <= 10".into()),
                timeout_ms: None,
            },
            SolverBatchStep {
                action: "check".into(),
                n: None,
                expression: None,
                timeout_ms: Some(5000),
            },
        ];

        let result = solver_batch(&steps);
        assert!(result.is_ok(), "batch should succeed");
        let parsed = result.unwrap();
        let steps_result = parsed
            .get("steps")
            .and_then(|v| v.as_array())
            .expect("should have steps");

        assert_eq!(
            steps_result[2]
                .get("result")
                .and_then(|v| v.as_str()),
            Some("sat"),
            "x >= 0 AND x <= 10 should be SAT"
        );
        // Model should contain x
        let model = steps_result[2].get("model");
        assert!(model.is_some(), "SAT response should include a model");
    }

    #[test]
    fn test_solver_batch_invalid_action() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping");
            return;
        }

        let steps = vec![SolverBatchStep {
            action: "invalid_action_xyz".into(),
            n: None,
            expression: None,
            timeout_ms: None,
        }];

        let result = solver_batch(&steps);
        assert!(result.is_ok(), "batch should not error on invalid action");
        let parsed = result.unwrap();
        let steps_result = parsed
            .get("steps")
            .and_then(|v| v.as_array())
            .expect("should have steps");

        assert_eq!(
            steps_result[0]
                .get("result")
                .and_then(|v| v.as_str()),
            Some("error"),
            "invalid action should produce error"
        );
    }

    #[test]
    fn test_prove_trivial() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping prove test");
            return;
        }

        // x == x should be provable
        let r = prove_formula("x == x");
        assert_eq!(
            r.status,
            VerificationStatus::Pass,
            "x == x should be provable: {}",
            r.details
        );
    }

    #[test]
    fn test_prove_implication() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping prove_implication test");
            return;
        }

        // Implies(x > 0, x + 1 > 0) should be provable (for Reals)
        let r = prove_formula("Implies(x > 0, x + 1 > 0)");
        assert_eq!(
            r.status,
            VerificationStatus::Pass,
            "Implies(x > 0, x + 1 > 0) should be provable: {}",
            r.details
        );
    }

    #[test]
    fn test_prove_disprove() {
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping disprove test");
            return;
        }

        // x == 5 should be disproved (it is NOT universally valid)
        let r = prove_formula("x == 5");
        assert_eq!(
            r.status,
            VerificationStatus::Fail,
            "x == 5 should be disproved: {}",
            r.details
        );
    }
}
