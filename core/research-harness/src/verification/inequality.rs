//! Inequality verification — pure Rust implementation.
//!
//! Pure business logic: types, LaTeX parsing (regex), minilp solver.
//! No Python/Z3/SymPy subprocess dependencies.
//!
//! # Layer boundary
//!
//! This module is FEATURE layer only. JSON argument extraction, result
//! formatting, and MCP tool dispatch belong in `mcp_tools.rs`.

use crate::types::{VerificationResult, VerificationStatus};
use core_errors::FrameworkError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ===========================================================================
// Core types
// ===========================================================================

/// Strictness of an inequality constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InequalitySense {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

/// A single linear inequality: `∑(coeffs[i] * vars[i]) sense rhs`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inequality {
    pub coefficients: Vec<f64>,
    pub vars: Vec<String>,
    pub sense: InequalitySense,
    pub rhs: f64,
}

impl Inequality {
    pub fn new(
        coefficients: Vec<f64>,
        vars: Vec<String>,
        sense: InequalitySense,
        rhs: f64,
    ) -> Self {
        Self {
            coefficients,
            vars,
            sense,
            rhs,
        }
    }
}

/// A system of linear inequalities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InequalitySystem {
    pub constraints: Vec<Inequality>,
}

impl InequalitySystem {
    pub fn new(constraints: Vec<Inequality>) -> Self {
        Self { constraints }
    }
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }
    pub fn len(&self) -> usize {
        self.constraints.len()
    }
}

/// Result of a feasibility check (serializable for Python bridge).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeasibilityResult {
    Feasible { model: HashMap<String, f64> },
    Infeasible { proof_certificate: String },
    Timeout { timeout_ms: u64 },
    Error { message: String },
    Warn { message: String },
}

impl FeasibilityResult {
    pub fn is_feasible(&self) -> bool {
        matches!(self, FeasibilityResult::Feasible { .. })
    }
    pub fn is_infeasible(&self) -> bool {
        matches!(self, FeasibilityResult::Infeasible { .. })
    }
}

// ===========================================================================
// LaTeX inequality string parsing (regex)
// ===========================================================================

pub fn parse_inequality_latex(expr: &str) -> Result<Inequality, FrameworkError> {
    parse_via_regex(expr)
}

fn parse_via_regex(expr: &str) -> Result<Inequality, FrameworkError> {
    let cleaned = expr
        .replace("\\leq", "<=")
        .replace("\\le", "<=")
        .replace("\\geq", ">=")
        .replace("\\ge", ">=")
        .replace("\\lt", "<")
        .replace("\\gt", ">")
        .replace("\\cdot", "*")
        .replace(' ', "");

    let re = regex::Regex::new(r"^(.+?)\s*(<=|>=|==|=|<|>)\s*(.+)$")
        .map_err(|e| FrameworkError::validation(format!("regex: {e}")))?;
    let caps = re
        .captures(&cleaned)
        .ok_or_else(|| FrameworkError::validation(format!("cannot parse: {expr}")))?;

    let lhs_str = caps
        .get(1)
        .ok_or_else(|| FrameworkError::validation("regex missing LHS group"))?
        .as_str();
    let sense_str = caps
        .get(2)
        .ok_or_else(|| FrameworkError::validation("regex missing sense group"))?
        .as_str();
    let rhs_str = caps
        .get(3)
        .ok_or_else(|| FrameworkError::validation("regex missing RHS group"))?
        .as_str();

    let sense = match sense_str {
        "<" => InequalitySense::Lt,
        "<=" => InequalitySense::Le,
        "==" | "=" => InequalitySense::Eq,
        ">=" => InequalitySense::Ge,
        ">" => InequalitySense::Gt,
        _ => {
            return Err(FrameworkError::validation(format!(
                "unknown sense: {sense_str}"
            )));
        }
    };

    // Extract terms from both sides. Variables go to LHS, constants to RHS.
    let (l_coeffs, l_vars, l_const) = extract_terms(lhs_str);
    let (r_coeffs, r_vars, r_const) = extract_terms(rhs_str);

    // Merge variable lists: coefficient = l_coeff - r_coeff (bring RHS vars to LHS)
    let mut all_vars: Vec<String> = l_vars.clone();
    let mut net_coeffs: Vec<f64> = l_coeffs.clone();
    for (rc, rv) in r_coeffs.iter().zip(r_vars.iter()) {
        match all_vars.iter().position(|v| v == rv) {
            Some(idx) => net_coeffs[idx] -= rc,
            None => {
                all_vars.push(rv.clone());
                net_coeffs.push(-rc);
            }
        }
    }

    // RHS net = r_const - l_const (bring LHS constants to RHS)
    let rhs_net = r_const - l_const;

    Ok(Inequality::new(net_coeffs, all_vars, sense, rhs_net))
}

fn parse_number(s: &str) -> Result<f64, FrameworkError> {
    if let Some(pos) = s.find('/') {
        let n: f64 = s[..pos]
            .parse()
            .map_err(|_| FrameworkError::validation(format!("bad num: {}", &s[..pos])))?;
        let d: f64 = s[pos + 1..]
            .parse()
            .map_err(|_| FrameworkError::validation(format!("bad den: {}", &s[pos + 1..])))?;
        if d == 0.0 {
            return Err(FrameworkError::validation("div by zero"));
        }
        return Ok(n / d);
    }
    s.parse::<f64>()
        .map_err(|_| FrameworkError::validation(format!("bad number: {s}")))
}

fn extract_terms(lhs: &str) -> (Vec<f64>, Vec<String>, f64) {
    let mut coeffs = Vec::new();
    let mut vars = Vec::new();
    let mut const_shift = 0.0;
    let src: Vec<char> = lhs.trim_start_matches('+').chars().collect();
    let mut i = 0;

    while i < src.len() {
        // Skip '+' separators (not consumed by the term body loop)
        if src[i] == '+' {
            i += 1;
            continue;
        }

        let mut term = String::new();
        if src[i] == '-' {
            term.push('-');
            i += 1;
        }
        while i < src.len() && src[i] != '+' && src[i] != '-' {
            term.push(src[i]);
            i += 1;
        }
        if let Some((c, v_opt)) = parse_one_term(&term) {
            if let Some(v) = v_opt {
                match vars.iter().position(|x: &String| x == &v) {
                    Some(idx) => coeffs[idx] += c,
                    None => {
                        vars.push(v);
                        coeffs.push(c);
                    }
                }
            } else {
                const_shift += c;
            }
        }
    }
    (coeffs, vars, const_shift)
}

fn parse_one_term(t: &str) -> Option<(f64, Option<String>)> {
    let t = t.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(n) = t.parse::<f64>() {
        return Some((n, None));
    }

    let mut num = String::new();
    let mut var = String::new();
    let mut in_var = false;
    for ch in t.chars() {
        if (ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '/') && !in_var {
            num.push(ch);
        } else {
            in_var = true;
            var.push(ch);
        }
    }
    if var.is_empty() {
        return None;
    }
    // Strip leading '*' from coefficient-variable separator
    let var = var.trim_start_matches('*').to_string();
    if var.is_empty() {
        return None;
    }
    let c = if num.is_empty() || num == "+" {
        1.0
    } else if num == "-" {
        -1.0
    } else {
        parse_number(&num).ok()?
    };
    Some((c, Some(var)))
}

// ===========================================================================
// minilp solver (pure Rust LP feasibility)
// ===========================================================================

/// Solve a system of linear inequalities using minilp.
pub fn solve_system(system: &InequalitySystem, timeout_ms: Option<u64>) -> FeasibilityResult {
    let _timeout = timeout_ms.unwrap_or(5000);
    if system.is_empty() {
        return FeasibilityResult::Feasible {
            model: HashMap::new(),
        };
    }

    let result = solve_via_minilp(system);
    match result {
        Ok(model) => FeasibilityResult::Feasible { model },
        Err(cert) => FeasibilityResult::Infeasible {
            proof_certificate: cert.to_string(),
        },
    }
}

fn solve_via_minilp(system: &InequalitySystem) -> Result<HashMap<String, f64>, FrameworkError> {
    use minilp::{ComparisonOp, OptimizationDirection, Problem};

    let mut prob = Problem::new(OptimizationDirection::Maximize);

    // Collect unique variable names across all constraints
    let mut all_vars: Vec<String> = Vec::new();
    for c in &system.constraints {
        for v in &c.vars {
            if !all_vars.contains(v) {
                all_vars.push(v.clone());
            }
        }
    }

    if all_vars.is_empty() {
        // No variables → check constant constraints
        for c in &system.constraints {
            let ok = match c.sense {
                InequalitySense::Lt => 0.0 < c.rhs,
                InequalitySense::Le => 0.0 <= c.rhs,
                InequalitySense::Eq => (0.0 - c.rhs).abs() < 1e-12,
                InequalitySense::Ge => 0.0 >= c.rhs,
                InequalitySense::Gt => 0.0 > c.rhs,
            };
            if !ok {
                return Err(FrameworkError::validation(format!(
                    "constant constraint violated: {:?} {} {}",
                    c.coefficients, c.rhs, c.rhs
                )));
            }
        }
        return Ok(HashMap::new());
    }

    // Bounded variables help the simplex converge
    let large = 1e10;

    // Create minilp variables (dummy objective = 0, wide bounds)
    let vars: Vec<minilp::Variable> = all_vars
        .iter()
        .map(|_| prob.add_var(0.0, (-large, large)))
        .collect();

    // Map from var name to index
    let var_indices: HashMap<&str, usize> = all_vars
        .iter()
        .enumerate()
        .map(|(i, v)| (v.as_str(), i))
        .collect();

    // Add each constraint
    for c in &system.constraints {
        let mut expr = minilp::LinearExpr::empty();
        for (coeff, vname) in c.coefficients.iter().zip(c.vars.iter()) {
            if let Some(&idx) = var_indices.get(vname.as_str()) {
                expr.add(vars[idx], *coeff);
            }
        }

        let op = match c.sense {
            InequalitySense::Lt => ComparisonOp::Le, // use ≤ with epsilon margin
            InequalitySense::Le => ComparisonOp::Le,
            InequalitySense::Eq => ComparisonOp::Eq,
            InequalitySense::Ge => ComparisonOp::Ge,
            InequalitySense::Gt => ComparisonOp::Ge, // use ≥ with epsilon margin
        };

        // For strict inequalities, use relative epsilon-adjusted bound
        let adjusted_rhs = match c.sense {
            InequalitySense::Lt => {
                let epsilon = c.rhs.abs() * 1e-7 + 1e-10;
                c.rhs - epsilon
            }
            InequalitySense::Gt => {
                let epsilon = c.rhs.abs() * 1e-7 + 1e-10;
                c.rhs + epsilon
            }
            _ => c.rhs,
        };

        prob.add_constraint(expr, op, adjusted_rhs);
    }

    // Solve — minilp 0.2 returns Result<Solution, Error>
    match prob.solve() {
        Ok(solution) => {
            let model: HashMap<String, f64> = all_vars
                .iter()
                .zip(solution.iter())
                .map(|(name, (_var, val))| (name.clone(), *val))
                .collect();
            Ok(model)
        }
        Err(e) => Err(FrameworkError::validation(format!("minilp: {e}"))),
    }
}

// ===========================================================================
// Verification pipeline integration
// ===========================================================================

pub fn check_inequality(expr: &str, timeout_ms: Option<u64>) -> VerificationResult {
    check_inequality_with_name(expr, timeout_ms, "math_prove_inequality")
}

/// Like `check_inequality` but with an explicit check name (for tool-layer reuse).
pub fn check_inequality_with_name(
    expr: &str,
    timeout_ms: Option<u64>,
    check_name: &str,
) -> VerificationResult {
    let ineq = match parse_inequality_latex(expr) {
        Ok(i) => i,
        Err(e) => {
            return VerificationResult {
                check_name: check_name.to_string(),
                status: VerificationStatus::Fail,
                details: format!("parse failed: {e}"),
                evidence_path: None,
            };
        }
    };
    let system = InequalitySystem::new(vec![ineq]);
    match solve_system(&system, timeout_ms) {
        FeasibilityResult::Feasible { model } => {
            let ms: Vec<String> = model.iter().map(|(k, v)| format!("{k}={v}")).collect();
            VerificationResult {
                check_name: check_name.to_string(),
                status: VerificationStatus::Pass,
                details: format!("Consistent. Model: {}", ms.join(", ")),
                evidence_path: None,
            }
        }
        FeasibilityResult::Infeasible { proof_certificate } => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Fail,
            details: format!("Inconsistent: {proof_certificate}"),
            evidence_path: None,
        },
        FeasibilityResult::Timeout { timeout_ms: t } => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Warn,
            details: format!("Timeout ({t}ms)"),
            evidence_path: None,
        },
        FeasibilityResult::Error { message } => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Warn,
            details: format!("Error: {message}"),
            evidence_path: None,
        },
        FeasibilityResult::Warn { message } => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Warn,
            details: message,
            evidence_path: None,
        },
    }
}

// ===========================================================================
// Backend probe
// ===========================================================================

/// minilp is always available (pure Rust, no external dependencies).
pub fn solver_available() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_parse_lt() {
        let ineq = parse_via_regex("x < 5").unwrap();
        assert_eq!(ineq.sense, InequalitySense::Lt);
        assert!((ineq.rhs - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_parse_le() {
        let ineq = parse_via_regex("x + y <= 10").unwrap();
        assert_eq!(ineq.sense, InequalitySense::Le);
    }

    #[test]
    fn test_parse_shift() {
        let ineq = parse_via_regex("2*x + 5 <= 3*y").unwrap();
        assert!((ineq.rhs - (-5.0)).abs() < 1e-10);
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_parse_laTeX() {
        let ineq = parse_via_regex("x \\leq 2*y").unwrap();
        assert_eq!(ineq.sense, InequalitySense::Le);
    }

    #[test]
    fn test_parse_negative_coeff() {
        let ineq = parse_via_regex("-x + 2*y < 5").unwrap();
        assert!((ineq.coefficients[0] - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_feasibility_roundtrip() {
        let r = FeasibilityResult::Feasible {
            model: HashMap::from([("x".into(), 1.0)]),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: FeasibilityResult = serde_json::from_str(&json).unwrap();
        assert!(back.is_feasible());
    }

    #[test]
    fn test_empty_system() {
        let system = InequalitySystem::new(vec![]);
        let result = solve_system(&system, Some(1000));
        assert!(result.is_feasible());
    }

    #[test]
    fn test_simple_feasible() {
        // x > 0
        let ineq = Inequality::new(vec![1.0], vec!["x".into()], InequalitySense::Gt, 0.0);
        let system = InequalitySystem::new(vec![ineq]);
        let result = solve_system(&system, Some(1000));
        assert!(result.is_feasible(), "x > 0 should be feasible");
    }

    #[test]
    fn test_contradiction() {
        // x > 5 AND x < 0
        let ineq1 = Inequality::new(vec![1.0], vec!["x".into()], InequalitySense::Gt, 5.0);
        let ineq2 = Inequality::new(vec![1.0], vec!["x".into()], InequalitySense::Lt, 0.0);
        let system = InequalitySystem::new(vec![ineq1, ineq2]);
        let result = solve_system(&system, Some(1000));
        assert!(
            result.is_infeasible(),
            "x > 5 AND x < 0 should be infeasible"
        );
    }

    #[test]
    fn test_probe_always_true() {
        assert!(solver_available());
    }
}
