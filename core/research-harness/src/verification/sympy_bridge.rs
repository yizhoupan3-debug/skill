//! SymPy bridge — identity verification and expression simplification.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! Delegates to the `asymptotic_solver` Python module (same process as
//! asymptotic analysis) via `crate::subprocess::run_uv_module`.

use crate::types::{VerificationResult, VerificationStatus};

/// Check if SymPy is available via `uv run python -c "import sympy"`.
pub fn sympy_available() -> bool {
    crate::verification::sympy_available()
}

/// Verify that `lhs - rhs` simplifies to zero via SymPy.
pub fn verify_identity(lhs: &str, rhs: &str, assumptions: &[&str]) -> VerificationResult {
    let input = serde_json::json!({
        "command": "verify_identity",
        "lhs": lhs,
        "rhs": rhs,
        "assumptions": assumptions,
    });

    match crate::subprocess::run_uv_module("asymptotic_solver", &input) {
        Ok(resp) => {
            if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
                return VerificationResult {
                    check_name: "math_sympy_verify".into(),
                    status: VerificationStatus::Warn,
                    details: format!("SymPy error: {err}"),
                    evidence_path: None,
                };
            }
            let diff = resp.get("difference").and_then(|v| v.as_str()).unwrap_or("?");
            let is_zero = resp.get("is_zero").and_then(|v| v.as_bool()).unwrap_or(false);

            if is_zero {
                VerificationResult {
                    check_name: "math_sympy_verify".into(),
                    status: VerificationStatus::Pass,
                    details: format!("{lhs} = {rhs} (difference: {diff})"),
                    evidence_path: None,
                }
            } else {
                VerificationResult {
                    check_name: "math_sympy_verify".into(),
                    status: VerificationStatus::Fail,
                    details: format!("{lhs} ≠ {rhs} (difference: {diff})"),
                    evidence_path: None,
                }
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_verify".into(),
            status: VerificationStatus::Warn,
            details: format!("subprocess: {e}"),
            evidence_path: None,
        },
    }
}

/// Simplify an expression via SymPy.
pub fn simplify_expression(expr: &str) -> VerificationResult {
    let input = serde_json::json!({
        "command": "simplify",
        "expr": expr,
    });

    match crate::subprocess::run_uv_module("asymptotic_solver", &input) {
        Ok(resp) => {
            if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
                return VerificationResult {
                    check_name: "math_sympy_simplify".into(),
                    status: VerificationStatus::Warn,
                    details: format!("SymPy error: {err}"),
                    evidence_path: None,
                };
            }
            let simplified = resp.get("simplified").and_then(|v| v.as_str()).unwrap_or("?");
            VerificationResult {
                check_name: "math_sympy_simplify".into(),
                status: VerificationStatus::Pass,
                details: format!("{expr} → {simplified}"),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_simplify".into(),
            status: VerificationStatus::Warn,
            details: format!("subprocess: {e}"),
            evidence_path: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sympy_probe() {
        let _ = sympy_available();
    }
}
