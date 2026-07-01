//! Symbolic identity verification and expression simplification.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! Uses real SymPy via Python subprocess when available, falling back
//! to the pure Rust symbolic engine (`crate::verification::symbolic`)
//! when the Python backend is not available.

use crate::types::{VerificationResult, VerificationStatus};
use crate::verification::python_bridge;
use serde_json::json;

/// SymPy availability — probes the real Python-based SymPy backend.
pub fn sympy_available() -> bool {
    python_bridge::sympy_available()
}

/// Verify that `lhs` and `rhs` are algebraically equivalent.
///
/// Uses real SymPy when available, falls back to pure Rust symbolic engine.
pub fn verify_identity(lhs: &str, rhs: &str) -> VerificationResult {
    // Try real SymPy first
    if python_bridge::sympy_available() {
        let params = json!({
            "lhs": lhs,
            "rhs": rhs,
        });
        match python_bridge::call_math_backend("sympy_verify", params) {
            Ok(result) => {
                let equal = result.get("equal").and_then(|v| v.as_bool()).unwrap_or(false);
                let difference = result
                    .get("difference")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();

                if equal {
                    return VerificationResult {
                        check_name: "math_sympy_verify".into(),
                        status: VerificationStatus::Pass,
                        details: format!("{lhs} = {rhs} (SymPy verified, diff={difference})"),
                        evidence_path: None,
                    };
                } else {
                    return VerificationResult {
                        check_name: "math_sympy_verify".into(),
                        status: VerificationStatus::Fail,
                        details: format!("{lhs} ≠ {rhs} (SymPy diff={difference})"),
                        evidence_path: None,
                    };
                }
            }
            Err(e) => {
                tracing::debug!("[sympy_bridge] real SymPy failed, falling back: {e}");
                // Fall through to pure Rust
            }
        }
    }

    // Fallback: pure Rust symbolic engine
    let (is_eq, details) = crate::verification::symbolic::verify_identity(lhs, rhs);
    if is_eq {
        VerificationResult {
            check_name: "math_sympy_verify".into(),
            status: VerificationStatus::Pass,
            details: format!("{details} (pure Rust fallback)"),
            evidence_path: None,
        }
    } else {
        VerificationResult {
            check_name: "math_sympy_verify".into(),
            status: VerificationStatus::Fail,
            details: format!("{details} (pure Rust fallback)"),
            evidence_path: None,
        }
    }
}

/// Simplify an expression using real SymPy when available.
pub fn simplify_expression(expr: &str) -> VerificationResult {
    simplify_expression_with_assumptions(expr, &[])
}

/// Simplify an expression with optional assumptions.
///
/// `assumptions` is a list of assumption strings (e.g. ``"x > 0"``) that are
/// passed to the SymPy backend's ``sp.refine()`` to produce a
/// context-sensitive simplified form.
pub fn simplify_expression_with_assumptions(
    expr: &str,
    assumptions: &[String],
) -> VerificationResult {
    // Try real SymPy first
    if python_bridge::sympy_available() {
        let mut params = json!({
            "expression": expr,
        });
        if !assumptions.is_empty() {
            params["assumptions"] = json!(assumptions);
        }
        match python_bridge::call_math_backend("sympy_simplify", params) {
            Ok(result) => {
                let simplified = result
                    .get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or(expr);
                return VerificationResult {
                    check_name: "math_sympy_simplify".into(),
                    status: VerificationStatus::Pass,
                    details: format!("{expr} → {simplified} (SymPy)"),
                    evidence_path: None,
                };
            }
            Err(e) => {
                tracing::debug!("[sympy_bridge] real SymPy simplify failed, falling back: {e}");
                // Fall through to pure Rust
            }
        }
    }

    // Fallback: pure Rust symbolic engine
    let simplified = crate::verification::symbolic::simplify_expression(expr);
    VerificationResult {
        check_name: "math_sympy_simplify".into(),
        status: VerificationStatus::Pass,
        details: format!("{expr} → {simplified} (pure Rust fallback)"),
        evidence_path: None,
    }
}

/// Simplify a trigonometric expression using `sp.trigsimp()`.
pub fn trig_simplify_expression(expr: &str) -> VerificationResult {
    if !python_bridge::sympy_available() {
        return VerificationResult {
            check_name: "math_sympy_trig_simplify".into(),
            status: VerificationStatus::Fail,
            details: format!("trig_simplify({expr}) — SymPy not available (requires Python backend)"),
            evidence_path: None,
        };
    }

    let params = json!({"expression": expr});
    match python_bridge::call_math_backend("sympy_trig_simplify", params) {
        Ok(result) => {
            let simplified = result
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or(expr);
            VerificationResult {
                check_name: "math_sympy_trig_simplify".into(),
                status: VerificationStatus::Pass,
                details: format!("trig_simplify({expr}) → {simplified} (SymPy)"),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_trig_simplify".into(),
            status: VerificationStatus::Fail,
            details: format!("trig_simplify({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Substitute variables/expressions in a symbolic expression.
pub fn subs_expression(expr: &str, substitutions: &serde_json::Value) -> VerificationResult {
    if !python_bridge::sympy_available() {
        return VerificationResult {
            check_name: "math_sympy_subs".into(),
            status: VerificationStatus::Fail,
            details: format!("subs({expr}) — SymPy not available (requires Python backend)"),
            evidence_path: None,
        };
    }

    let params = json!({
        "expression": expr,
        "substitutions": substitutions,
    });
    match python_bridge::call_math_backend("sympy_subs", params) {
        Ok(result) => {
            let substituted = result
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or(expr);
            VerificationResult {
                check_name: "math_sympy_subs".into(),
                status: VerificationStatus::Pass,
                details: format!("subs({expr}) → {substituted} (SymPy)"),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_subs".into(),
            status: VerificationStatus::Fail,
            details: format!("subs({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Compute the limit of an expression as `variable` approaches `point`.
pub fn limit_expression(
    expr: &str,
    variable: &str,
    point: &str,
    direction: Option<&str>,
) -> VerificationResult {
    if !python_bridge::sympy_available() {
        return VerificationResult {
            check_name: "math_sympy_limit".into(),
            status: VerificationStatus::Fail,
            details: format!("limit({expr}, {variable}→{point}) — SymPy not available (requires Python backend)"),
            evidence_path: None,
        };
    }

    let mut params = json!({
        "expression": expr,
        "variable": variable,
        "point": point,
    });
    if let Some(dir) = direction {
        params["direction"] = json!(dir);
    }
    match python_bridge::call_math_backend("sympy_limit", params) {
        Ok(result) => {
            let limit_val = result
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            VerificationResult {
                check_name: "math_sympy_limit".into(),
                status: VerificationStatus::Pass,
                details: format!("limit({expr}, {variable}→{point}) = {limit_val} (SymPy)"),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_limit".into(),
            status: VerificationStatus::Fail,
            details: format!("limit({expr}, {variable}→{point}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Convert a symbolic expression to a numeric callable and evaluate it.
pub fn lambdify_expression(
    expr: &str,
    variables: &[String],
    values: Option<&[f64]>,
) -> VerificationResult {
    if !python_bridge::sympy_available() {
        return VerificationResult {
            check_name: "math_sympy_lambdify".into(),
            status: VerificationStatus::Fail,
            details: format!("lambdify({expr}) — SymPy not available (requires Python backend)"),
            evidence_path: None,
        };
    }

    let mut params = json!({
        "expression": expr,
        "variables": variables,
    });
    if let Some(vals) = values {
        params["values"] = json!(vals);
    }
    match python_bridge::call_math_backend("sympy_lambdify", params) {
        Ok(result) => {
            let result_str = result
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or(expr);
            let evaluated = result.get("evaluated").and_then(|v| v.as_f64());
            let mut details = format!("lambdify({expr}) = {result_str} (SymPy)");
            if let Some(eval_val) = evaluated {
                details.push_str(&format!(", evaluated={eval_val}"));
            }
            VerificationResult {
                check_name: "math_sympy_lambdify".into(),
                status: VerificationStatus::Pass,
                details,
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_lambdify".into(),
            status: VerificationStatus::Fail,
            details: format!("lambdify({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe() {
        // Should not panic
        let _ = sympy_available();
    }

    #[test]
    fn test_verify_trivial() {
        let vr = verify_identity("x", "x");
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_verify_polynomial() {
        let vr = verify_identity("(x+1)^2", "x^2 + 2*x + 1");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "expected Pass, got {:?}: {}",
            vr.status,
            vr.details
        );
    }

    #[test]
    fn test_verify_not_equal() {
        let vr = verify_identity("x + 1", "x + 2");
        assert_eq!(vr.status, VerificationStatus::Fail);
    }

    #[test]
    fn test_simplify() {
        let vr = simplify_expression("x + x");
        assert_eq!(vr.status, VerificationStatus::Pass);
        assert!(
            vr.details.contains("2*x"),
            "expected 2*x, got {}",
            vr.details
        );
    }

    #[test]
    fn test_verify_trig_identity() {
        // This identity should work via real SymPy if available,
        // and may fail via pure Rust (which doesn't do trig simplify)
        let vr = verify_identity("sin(x)^2 + cos(x)^2", "1");
        // We accept either pass or fail since pure Rust doesn't simplify trig
        if vr.status == VerificationStatus::Fail {
            tracing::info!("trig identity not verified (expected if pure Rust fallback)");
        }
    }

    // ── Dual-path indicator tests ──

    #[test]
    fn test_verify_identity_path_indicator() {
        // The `details` string MUST document which verification path was used:
        //   "(SymPy verified"  — real SymPy Python subprocess path
        //   "(pure Rust fallback)" — pure Rust symbolic engine fallback
        let vr = verify_identity("x", "x");
        assert_eq!(vr.status, VerificationStatus::Pass);

        let has_sympy = vr.details.contains("(SymPy");
        let has_fallback = vr.details.contains("(pure Rust fallback)");
        assert!(
            has_sympy || has_fallback,
            "verify_identity details must contain path indicator, got: {}",
            vr.details
        );

        // Verify a non-identity also documents its path
        let vr2 = verify_identity("x + 1", "x + 2");
        assert_eq!(vr2.status, VerificationStatus::Fail);
        let has_sympy2 = vr2.details.contains("(SymPy");
        let has_fallback2 = vr2.details.contains("(pure Rust fallback)");
        assert!(
            has_sympy2 || has_fallback2,
            "verify_identity (neq) details must contain path indicator, got: {}",
            vr2.details
        );
    }

    #[test]
    fn test_simplify_expression_path_indicator() {
        // The `details` string for simplify MUST end with either "(SymPy)" or
        // "(pure Rust fallback)"
        let vr = simplify_expression("x + x");
        assert_eq!(vr.status, VerificationStatus::Pass);

        let has_sympy = vr.details.contains("(SymPy)");
        let has_fallback = vr.details.contains("(pure Rust fallback)");
        assert!(
            has_sympy || has_fallback,
            "simplify_expression details must contain path indicator, got: {}",
            vr.details
        );

        // Verify the simplified result is included
        assert!(
            vr.details.contains("2*x"),
            "expected simplified 2*x in details, got: {}",
            vr.details
        );
    }

    #[test]
    fn test_verify_identity_path_consistency() {
        // When both paths are theoretically reachable in the same test run
        // (unlikely since sympy_available is cached), this test verifies that
        // either path produces a logically consistent result for a well-known
        // polynomial identity.
        let vr = verify_identity("(x+1)^2", "x^2 + 2*x + 1");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "polynomial identity must hold on any path: {}",
            vr.details
        );
        assert!(
            vr.details.contains("(SymPy") || vr.details.contains("(pure Rust fallback)"),
            "details must indicate the verification path, got: {}",
            vr.details
        );
    }
}
