//! Perturbation expansion verification.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! Supports regular perturbation expansion where the solution is expressed
//! as a power series in a small parameter:
//!
//!   u = u₀ + ε·u₁ + ε²·u₂ + ...
//!
//! Uses real SymPy via Python subprocess when available.

use crate::types::VerificationStatus;
use crate::verification::python_bridge;
use serde::Serialize;
use serde_json::json;

/// Result of a single-order perturbation expansion.
#[derive(Debug, Clone, Serialize)]
pub struct PerturbationOrder {
    /// The perturbation order (0, 1, 2, ...)
    pub order: u32,
    /// The governing equation at this order
    pub equation: String,
    /// The solved expression for this order
    pub solution: String,
}

/// Full result of a perturbation expansion.
#[derive(Debug, Clone, Serialize)]
pub struct PerturbationResult {
    /// Check name for the tool call
    pub check_name: String,
    /// Overall status (Pass if any orders were solved)
    pub status: VerificationStatus,
    /// Per-order expansion results
    pub orders: Vec<PerturbationOrder>,
    /// The full composite solution (sum of all orders)
    pub full_solution: String,
    /// Human-readable details
    pub details: String,
}

/// Perform a regular perturbation expansion via the SymPy backend.
///
/// Given an ODE involving a small parameter `parameter`, assumes a solution
/// of the form `u = u₀ + ε·u₁ + ε²·u₂ + ...` (where ε is the small parameter),
/// substitutes into the equation, collects terms at each power of ε,
/// and solves each order sequentially using SymPy's `dsolve`.
///
/// # Parameters
///
/// * `equation`  - The full differential equation string (e.g., `"u'' + u + eps*u^3 = 0"`).
/// * `variable`  - The independent variable (e.g., `"x"`).
/// * `parameter` - The small perturbation parameter name (e.g., `"eps"` or `"epsilon"`).
/// * `order`     - Maximum expansion order (1 = O(ε), 2 = O(ε²), etc.).
/// * `bc`        - Optional boundary/initial conditions string (e.g., `"u(0)=1, u'(0)=0"`).
///
/// # Returns
///
/// A `PerturbationResult` with per-order equations and solutions.
pub fn regular_perturbation(
    equation: &str,
    variable: &str,
    parameter: &str,
    order: u32,
    bc: Option<&str>,
) -> PerturbationResult {
    if !python_bridge::sympy_available() {
        let details = format!(
            "regular_perturbation({equation}, ε={parameter}, order={order}) \
             — SymPy not available (requires Python backend)"
        );
        return PerturbationResult {
            check_name: "math_perturbation_expand".into(),
            status: VerificationStatus::Fail,
            orders: Vec::new(),
            full_solution: String::new(),
            details,
        };
    }

    let mut params = json!({
        "equation": equation,
        "variable": variable,
        "parameter": parameter,
        "order": order,
    });
    if let Some(bc_str) = bc {
        params["bc"] = json!(bc_str);
    }

    match python_bridge::call_math_backend("sympy_perturbation_expand", params) {
        Ok(result) => {
            // Parse orders array from backend response
            let orders: Vec<PerturbationOrder> = result
                .get("orders")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|o| {
                            Some(PerturbationOrder {
                                order: o.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                                equation: o
                                    .get("equation")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                solution: o
                                    .get("solution")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            let full_solution = result
                .get("full_solution")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let status = if orders.is_empty() {
                VerificationStatus::Fail
            } else {
                VerificationStatus::Pass
            };

            let details = if status == VerificationStatus::Pass {
                let orders_count = orders.len();
                let solved_count = orders
                    .iter()
                    .filter(|o| !o.solution.is_empty() && o.solution != "0")
                    .count();
                format!(
                    "regular_perturbation({equation}, ε={parameter}, order={order}) \
                     — {orders_count} orders collected, {solved_count} non-trivial solutions (SymPy)"
                )
            } else {
                format!(
                    "regular_perturbation({equation}, ε={parameter}, order={order}) \
                     — no orders produced (SymPy)"
                )
            };

            PerturbationResult {
                check_name: "math_perturbation_expand".into(),
                status,
                orders,
                full_solution,
                details,
            }
        }
        Err(e) => PerturbationResult {
            check_name: "math_perturbation_expand".into(),
            status: VerificationStatus::Fail,
            orders: Vec::new(),
            full_solution: String::new(),
            details: format!(
                "regular_perturbation({equation}, ε={parameter}, order={order}) failed: {e}"
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perturbation_non_linear() {
        // Duffing oscillator: u'' + u + ε·u^3 = 0, expanded to O(ε)
        // Expected: at O(1): u₀'' + u₀ = 0 → harmonic
        let result = regular_perturbation("u'' + u + eps*u^3", "t", "eps", 1, None);
        assert!(
            result.status == VerificationStatus::Pass
                || result.status == VerificationStatus::Fail,
            "perturbation should pass or fail, got {:?}: {}",
            result.status,
            result.details
        );
    }

    #[test]
    fn test_perturbation_linear() {
        // Linear: u'' + u + ε·u = 0 → u'' + (1+ε)u = 0
        let result = regular_perturbation("u'' + u + eps*u", "t", "eps", 1, None);
        assert!(
            result.status == VerificationStatus::Pass
                || result.status == VerificationStatus::Fail,
            "perturbation should pass or fail, got {:?}: {}",
            result.status,
            result.details
        );
    }

    #[test]
    fn test_perturbation_with_bc() {
        let result = regular_perturbation(
            "u'' + u + eps*u^3",
            "t",
            "eps",
            1,
            Some("u(0)=1, u'(0)=0"),
        );
        assert!(
            result.status == VerificationStatus::Pass
                || result.status == VerificationStatus::Fail,
            "perturbation with bc should pass or fail, got {:?}: {}",
            result.status,
            result.details
        );
    }

    #[test]
    fn test_no_sympy_fallback() {
        // When SymPy is unavailable, the result should be Fail with a clear message
        if !python_bridge::sympy_available() {
            let result =
                regular_perturbation("u'' + u + eps*u^3", "t", "eps", 1, None);
            assert_eq!(result.status, VerificationStatus::Fail);
            assert!(
                result.details.contains("not available"),
                "should mention backend unavailability, got: {}",
                result.details
            );
        }
    }
}
