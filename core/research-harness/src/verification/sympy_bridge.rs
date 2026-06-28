//! Symbolic identity verification and expression simplification.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! Pure Rust implementation using the `symbolic` module. No Python/SymPy subprocess.

use crate::types::{VerificationResult, VerificationStatus};

/// SymPy availability probe — always returns true (pure Rust implementation).
pub fn sympy_available() -> bool {
    true
}

/// Verify that `lhs` and `rhs` are algebraically equivalent.
pub fn verify_identity(lhs: &str, rhs: &str) -> VerificationResult {
    let (is_eq, details) = crate::verification::symbolic::verify_identity(lhs, rhs);
    if is_eq {
        VerificationResult {
            check_name: "math_sympy_verify".into(),
            status: VerificationStatus::Pass,
            details,
            evidence_path: None,
        }
    } else {
        VerificationResult {
            check_name: "math_sympy_verify".into(),
            status: VerificationStatus::Fail,
            details,
            evidence_path: None,
        }
    }
}

/// Simplify an expression.
pub fn simplify_expression(expr: &str) -> VerificationResult {
    let simplified = crate::verification::symbolic::simplify_expression(expr);
    VerificationResult {
        check_name: "math_sympy_simplify".into(),
        status: VerificationStatus::Pass,
        details: format!("{expr} → {simplified}"),
        evidence_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe() {
        assert!(sympy_available());
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
}
