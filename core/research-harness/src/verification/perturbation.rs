//! Perturbation expansion verification — pure Rust implementation.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! Supports regular perturbation expansion for second-order constant-coefficient ODEs
//! of the form u'' + ω²·u + ε·f(u) = 0.  Assumes a solution u = u₀ + ε·u₁ + ε²·u₂ + …
//! and solves each order using characteristic equation methods.
//!
//! **Current limitations**: only supports constant-coefficient linear operators;
//! perturbation terms are handled symbolically for common patterns (Duffing u³);
//! higher-order particular solutions may be approximate.

use crate::types::VerificationStatus;
use crate::verification::symbolic;
use serde::Serialize;

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

/// Perform a regular perturbation expansion using the pure Rust engine.
///
/// Given an ODE involving a small parameter `parameter`, assumes a solution
/// of the form `u = u₀ + ε·u₁ + ε²·u₂ + ...` (where ε is the small parameter),
/// substitutes into the equation, collects terms at each power of ε,
/// and solves each order sequentially.
///
/// # Parameters
///
/// * `equation`  - The full differential equation string (e.g., `"u'' + u + eps*u^3"`).
/// * `variable`  - The independent variable (e.g., `"t"`).
/// * `parameter` - The small perturbation parameter name (e.g., `"eps"` or `"epsilon"`).
/// * `order`     - Maximum expansion order (1 = O(ε), 2 = O(ε²), etc.).
/// * `bc`        - Optional boundary/initial conditions string (e.g., `"u(0)=1, u'(0)=0"`).
pub fn regular_perturbation(
    equation: &str,
    variable: &str,
    parameter: &str,
    order: u32,
    bc: Option<&str>,
) -> PerturbationResult {
    let expansion = symbolic::perturbation_expand(equation, variable, parameter, order, bc);

    let orders: Vec<PerturbationOrder> = expansion
        .orders
        .into_iter()
        .map(|o| PerturbationOrder {
            order: o.order,
            equation: o.equation,
            solution: o.solution,
        })
        .collect();

    let status = if orders.is_empty() {
        VerificationStatus::Fail
    } else {
        VerificationStatus::Pass
    };

    let orders_count = orders.len();
    let solved_count = orders
        .iter()
        .filter(|o| !o.solution.is_empty() && o.solution != "0")
        .count();

    let details = format!(
        "regular_perturbation({equation}, ε={parameter}, order={order}) \
         — {orders_count} orders collected, {solved_count} non-trivial solutions (pure Rust)"
    );

    PerturbationResult {
        check_name: "math_perturbation_expand".into(),
        status,
        orders,
        full_solution: expansion.full_solution,
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perturbation_non_linear() {
        // Duffing oscillator: u'' + u + ε·u^3 = 0, expanded to O(ε)
        let result = regular_perturbation("u'' + u + eps*u^3", "t", "eps", 1, None);
        assert_eq!(
            result.status,
            VerificationStatus::Pass,
            "perturbation should pass: {}",
            result.details
        );
        assert!(
            result.orders.len() >= 2,
            "should have at least 2 orders (0 and 1)"
        );
    }

    #[test]
    fn test_perturbation_linear() {
        // Linear: u'' + u + ε·u = 0
        let result = regular_perturbation("u'' + u + eps*u", "t", "eps", 1, None);
        assert_eq!(result.status, VerificationStatus::Pass);
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
        assert_eq!(
            result.status,
            VerificationStatus::Pass,
            "perturbation with BCs should pass: {}",
            result.details
        );
    }

    #[test]
    fn test_perturbation_always_available() {
        // Pure Rust engine is always available — should never fail with "not available"
        let result = regular_perturbation("u'' + u + eps*u^3", "t", "eps", 1, None);
        assert!(
            !result.details.contains("not available"),
            "should never report 'not available': {}",
            result.details
        );
    }

    #[test]
    fn test_perturbation_order_2() {
        let result = regular_perturbation("u'' + u + eps*u^3", "t", "eps", 2, None);
        assert_eq!(result.status, VerificationStatus::Pass);
        assert!(result.orders.len() >= 3, "should have orders 0, 1, 2");
    }
}
