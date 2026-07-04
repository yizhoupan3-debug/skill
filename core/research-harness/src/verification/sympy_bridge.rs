//! Pure Rust symbolic math bridge — replaces the former SymPy Python subprocess.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! All operations are implemented in the pure Rust symbolic engine
//! (`crate::verification::symbolic`).  No Python/SymPy dependency.

use crate::types::{VerificationResult, VerificationStatus};
use crate::verification::symbolic;
use serde_json::json;

/// Pure Rust symbolic engine is always available (no external dependency).
pub fn sympy_available() -> bool {
    true
}

/// Verify that `lhs` and `rhs` are algebraically equivalent.
pub fn verify_identity(lhs: &str, rhs: &str) -> VerificationResult {
    let (is_eq, details) = symbolic::verify_identity(lhs, rhs);
    VerificationResult {
        check_name: "math_sympy_verify".into(),
        status: if is_eq {
            VerificationStatus::Pass
        } else {
            VerificationStatus::Fail
        },
        details: format!("{details} (pure Rust)"),
        evidence_path: None,
    }
}

/// Simplify an expression using the pure Rust symbolic engine.
pub fn simplify_expression(expr: &str) -> VerificationResult {
    simplify_expression_with_assumptions(expr, &[])
}

/// Simplify with optional assumptions (pure Rust: assumptions are noted but
/// the Rust engine does not yet support context-sensitive refinement).
pub fn simplify_expression_with_assumptions(
    expr: &str,
    assumptions: &[String],
) -> VerificationResult {
    let simplified = symbolic::simplify_expression(expr);
    let suffix = if assumptions.is_empty() {
        String::new()
    } else {
        format!(" [assumptions: {}]", assumptions.join(", "))
    };
    VerificationResult {
        check_name: "math_sympy_simplify".into(),
        status: VerificationStatus::Pass,
        details: format!("{expr} → {simplified}{suffix} (pure Rust)"),
        evidence_path: None,
    }
}

/// Simplify a trigonometric expression.
pub fn trig_simplify_expression(expr: &str) -> VerificationResult {
    match symbolic::parse(expr) {
        Ok(parsed) => {
            let simplified = symbolic::trig_simplify(&parsed);
            let result = symbolic::display(&simplified);
            VerificationResult {
                check_name: "math_sympy_trig_simplify".into(),
                status: VerificationStatus::Pass,
                details: format!("trig_simplify({expr}) → {result} (pure Rust)"),
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
    match symbolic::parse(expr) {
        Ok(parsed) => {
            let mut mapping: std::collections::HashMap<String, symbolic::Expr> =
                std::collections::HashMap::new();
            if let Some(obj) = substitutions.as_object() {
                for (key, val) in obj {
                    if key == "simultaneous" {
                        continue;
                    }
                    if let Some(num) = val.as_f64() {
                        mapping.insert(key.clone(), symbolic::Expr::Const(num));
                    } else if let Some(s) = val.as_str() {
                        if let Ok(sub_expr) = symbolic::parse(s) {
                            mapping.insert(key.clone(), sub_expr);
                        }
                    }
                }
            }

            if mapping.is_empty() {
                return VerificationResult {
                    check_name: "math_sympy_subs".into(),
                    status: VerificationStatus::Fail,
                    details: format!("subs({expr}) — no valid substitutions"),
                    evidence_path: None,
                };
            }

            let result = symbolic::substitute_all(&parsed, &mapping);
            let result_str = symbolic::display(&result);
            VerificationResult {
                check_name: "math_sympy_subs".into(),
                status: VerificationStatus::Pass,
                details: format!("subs({expr}) → {result_str} (pure Rust)"),
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
    match symbolic::parse(expr) {
        Ok(parsed) => {
            match symbolic::limit(&parsed, variable, point, direction) {
                Ok(limit_expr) => {
                    let limit_val = symbolic::display(&limit_expr);
                    VerificationResult {
                        check_name: "math_sympy_limit".into(),
                        status: VerificationStatus::Pass,
                        details: format!(
                            "limit({expr}, {variable}→{point}) = {limit_val} (pure Rust)"
                        ),
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
        Err(e) => VerificationResult {
            check_name: "math_sympy_limit".into(),
            status: VerificationStatus::Fail,
            details: format!("limit({expr}) failed: {e}"),
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
    match symbolic::parse(expr) {
        Ok(parsed) => {
            let mut details = format!("lambdify({expr}) (pure Rust)");
            if let Some(vals) = values {
                if variables.len() == vals.len() {
                    let mut vars_map = std::collections::HashMap::new();
                    for (v, val) in variables.iter().zip(vals.iter()) {
                        vars_map.insert(v.clone(), *val);
                    }
                    match symbolic::eval(&parsed, &vars_map) {
                        Ok(eval_val) => {
                            details = format!(
                                "lambdify({expr}) (pure Rust), evaluated={eval_val}"
                            );
                        }
                        Err(e) => {
                            details = format!(
                                "lambdify({expr}) (pure Rust), eval error: {e}"
                            );
                        }
                    }
                } else {
                    details = format!(
                        "lambdify({expr}) (pure Rust), variable count mismatch"
                    );
                }
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

/// Expand a symbolic expression.
pub fn expand_expression(expr: &str) -> VerificationResult {
    match symbolic::parse(expr) {
        Ok(parsed) => {
            let expanded = symbolic::expand(&parsed);
            let result = symbolic::display(&expanded);
            VerificationResult {
                check_name: "math_sympy_expand".into(),
                status: VerificationStatus::Pass,
                details: format!("expand({expr}) → {result} (pure Rust)"),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_expand".into(),
            status: VerificationStatus::Fail,
            details: format!("expand({expr}) failed: parse error: {e}"),
            evidence_path: None,
        },
    }
}

/// Factor a symbolic expression.
pub fn factor_expression(expr: &str) -> VerificationResult {
    match symbolic::parse(expr) {
        Ok(parsed) => {
            let factored = symbolic::factor(&parsed);
            let result = symbolic::display(&factored);
            VerificationResult {
                check_name: "math_sympy_factor".into(),
                status: VerificationStatus::Pass,
                details: format!("factor({expr}) → {result} (pure Rust)"),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_factor".into(),
            status: VerificationStatus::Fail,
            details: format!("factor({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Compute series expansion of an expression.
pub fn series_expression(expr: &str, variable: &str, point: f64, order: u32) -> VerificationResult {
    match symbolic::parse(expr) {
        Ok(parsed) => {
            let series_expr = symbolic::series(&parsed, variable, point, order);
            let result_str = symbolic::display(&series_expr);
            VerificationResult {
                check_name: "math_sympy_series".into(),
                status: VerificationStatus::Pass,
                details: format!(
                    "series({expr}, {variable}, {point}, order={order}) → {result_str} (pure Rust)"
                ),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_series".into(),
            status: VerificationStatus::Fail,
            details: format!("series({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Differentiate a symbolic expression.
pub fn differentiate_expression(expr: &str, variable: &str, order: u32) -> VerificationResult {
    match symbolic::parse(expr) {
        Ok(parsed) => {
            let mut result = parsed;
            for _ in 0..order {
                result = symbolic::differentiate(&result, variable);
            }
            let diffed = symbolic::display(&result);
            VerificationResult {
                check_name: "math_sympy_differentiate".into(),
                status: VerificationStatus::Pass,
                details: format!(
                    "differentiate({expr}, {variable}, order={order}) → {diffed} (pure Rust)"
                ),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_differentiate".into(),
            status: VerificationStatus::Fail,
            details: format!("differentiate({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Integrate a symbolic expression.
pub fn integrate_expression(
    expr: &str,
    variable: &str,
    lower: Option<f64>,
    upper: Option<f64>,
) -> VerificationResult {
    match symbolic::parse(expr) {
        Ok(parsed) => {
            let integrated = symbolic::integrate(&parsed, variable);
            let result_str = symbolic::display(&integrated);

            match (lower, upper) {
                (Some(l), Some(u)) => {
                    let mut upper_vars = std::collections::HashMap::new();
                    upper_vars.insert(variable.to_string(), u);
                    let mut lower_vars = std::collections::HashMap::new();
                    lower_vars.insert(variable.to_string(), l);
                    match (
                        symbolic::eval(&integrated, &upper_vars),
                        symbolic::eval(&integrated, &lower_vars),
                    ) {
                        (Ok(f_upper), Ok(f_lower)) => {
                            let value = f_upper - f_lower;
                            VerificationResult {
                                check_name: "math_sympy_integrate".into(),
                                status: VerificationStatus::Pass,
                                details: format!(
                                    "integrate({expr}, {variable} [{l}, {u}]) → {result_str} = {value} (pure Rust)"
                                ),
                                evidence_path: None,
                            }
                        }
                        _ => VerificationResult {
                            check_name: "math_sympy_integrate".into(),
                            status: VerificationStatus::Fail,
                            details: format!(
                                "integrate({expr}, {variable}) → {result_str} (could not evaluate definite integral) (pure Rust)"
                            ),
                            evidence_path: None,
                        },
                    }
                }
                _ => VerificationResult {
                    check_name: "math_sympy_integrate".into(),
                    status: VerificationStatus::Pass,
                    details: format!(
                        "integrate({expr}, {variable}) → {result_str} (pure Rust)"
                    ),
                    evidence_path: None,
                },
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_integrate".into(),
            status: VerificationStatus::Fail,
            details: format!("integrate({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Solve an equation or system of equations.
pub fn solve_equation(equation: &str, variable: &str) -> VerificationResult {
    match symbolic::solve_equation(equation, variable) {
        Ok(solutions) => {
            let count = solutions.len();
            let sol_str = if count == 0 {
                "no solutions".to_string()
            } else {
                solutions.join("; ")
            };
            VerificationResult {
                check_name: "math_sympy_solve".into(),
                status: VerificationStatus::Pass,
                details: format!(
                    "solve({equation}) → {sol_str} ({count} solutions, pure Rust)"
                ),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_sympy_solve".into(),
            status: VerificationStatus::Fail,
            details: format!("solve({equation}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Propagate physical dimensions through an equation using the pure Rust AST engine.
pub fn dimension_propagate(equation: &str, dimensions: &serde_json::Value) -> VerificationResult {
    let mut dim_map = std::collections::HashMap::new();
    if let Some(obj) = dimensions.as_object() {
        for (key, val) in obj {
            if let Some(s) = val.as_str() {
                dim_map.insert(key.clone(), s.to_string());
            }
        }
    }

    let result = symbolic::propagate_dimensions_ast(equation, &dim_map);
    let status = if result.consistent {
        VerificationStatus::Pass
    } else {
        VerificationStatus::Fail
    };
    VerificationResult {
        check_name: "math_sympy_dimension_propagate".into(),
        status,
        details: format!(
            "dimension_propagate({equation}) → LHS={}, RHS={}, consistent={} (pure Rust)",
            result.lhs_dim, result.rhs_dim, result.consistent
        ),
        evidence_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sympy_available() {
        assert!(sympy_available(), "pure Rust engine is always available");
    }

    #[test]
    fn test_verify_trivial() {
        let vr = verify_identity("x", "x");
        assert_eq!(vr.status, VerificationStatus::Pass);
        assert!(vr.details.contains("pure Rust"));
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
    fn test_expand() {
        let vr = expand_expression("(x+1)^2");
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_factor() {
        let vr = factor_expression("x^2 + 2*x + 1");
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_series() {
        let vr = series_expression("sin(x)", "x", 0.0, 6);
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_differentiate() {
        let vr = differentiate_expression("x^2", "x", 1);
        assert_eq!(vr.status, VerificationStatus::Pass);
        assert!(vr.details.contains("2*x"));
    }

    #[test]
    fn test_integrate() {
        let vr = integrate_expression("x^2", "x", None, None);
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_solve() {
        let vr = solve_equation("x^2 - 4 = 0", "x");
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_dimension_propagate() {
        let dims = serde_json::json!({"F": "L*M*T^-2", "m": "M", "a": "L*T^-2"});
        let vr = dimension_propagate("F = m*a", &dims);
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_trig_simplify() {
        let vr = trig_simplify_expression("sin(x)^2 + cos(x)^2");
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_subs() {
        let subs = serde_json::json!({"x": 2});
        let vr = subs_expression("x^2 + 1", &subs);
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_limit() {
        let vr = limit_expression("1/x", "x", "oo", None);
        assert_eq!(vr.status, VerificationStatus::Pass);
    }

    #[test]
    fn test_lambdify() {
        let vr = lambdify_expression("x^2 + 1", &["x".to_string()], Some(&[2.0]));
        assert_eq!(vr.status, VerificationStatus::Pass);
        assert!(vr.details.contains("evaluated"));
    }
}
