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
    if system.is_empty() {
        return FeasibilityResult::Feasible {
            model: HashMap::new(),
        };
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let system_clone = system.clone();
    std::thread::spawn(move || {
        let _ = tx.send(solve_via_minilp(&system_clone));
    });

    let result = match timeout_ms {
        Some(ms) => match rx.recv_timeout(std::time::Duration::from_millis(ms)) {
            Ok(r) => r,
            Err(_) => {
                return FeasibilityResult::Timeout {
                    timeout_ms: ms,
                }
            }
        },
        None => rx.recv().unwrap_or_else(|_| {
            Err(FrameworkError::validation(
                "inequality solver channel closed unexpectedly",
            ))
        }),
    };

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

/// Check if an expression is potentially nonlinear.
///
/// Uses a conservative heuristic: function calls (sin, cos, etc.),
/// power operators (`^`, `**`), `pi`, and variable×variable products
/// (i.e. `*` where BOTH sides are non-constant tokens).
///
/// A `*` between a constant and a variable (e.g. `2*x`, `x*3`) is LINEAR
/// and can be solved by minilp — only when `*` connects two non-constant
/// tokens (e.g. `x*y`, `(x+1)*y`, `x*sin(x)`) do we route to Z3.
fn is_nonlinear(expr: &str) -> bool {
    let lower = expr.to_lowercase();

    // Function calls and power operators → nonlinear
    if lower.contains('^')
        || lower.contains("**")
        || lower.contains("sin(")
        || lower.contains("cos(")
        || lower.contains("tan(")
        || lower.contains("sqrt(")
        || lower.contains("abs(")
        || lower.contains("exp(")
        || lower.contains("log(")
        || lower.contains("ln(")
        || word_boundary_match(&lower, "pi")
    {
        return true;
    }

    // Variable×variable product detection: `*` where both adjacent
    // non-whitespace characters are letters (identifiers) or parens.
    // Constant×variable (e.g. `2*x`, `x*2`) is LINEAR and NOT flagged.
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'*' {
            // Find the first non-whitespace char before *
            let mut j = if i > 0 { i - 1 } else { return false; };
            while j > 0 && bytes[j] == b' ' { j -= 1; }
            let before = bytes[j] as char;

            // Find the first non-whitespace char after *
            let mut k = i + 1;
            while k < bytes.len() && bytes[k] == b' ' { k += 1; }
            if k >= bytes.len() { return false; }
            let after = bytes[k] as char;

            // If both sides are letters or parens → variable × variable or (expr) × variable
            if (before.is_ascii_alphabetic() || before == ')')
                && (after.is_ascii_alphabetic() || after == '(')
            {
                return true;
            }
        }
        i += 1;
    }

    false
}

/// Check if `keyword` appears as a whole word (not substring) in `text`.
///
/// Used to avoid false positives like `"pivot"` matching keyword `"pi"`.
fn word_boundary_match(text: &str, keyword: &str) -> bool {
    let kw_len = keyword.len();
    if kw_len > text.len() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(pos) = text[start..].find(keyword) {
        let abs_pos = start + pos;
        // Check char before keyword
        let before_ok = if abs_pos == 0 {
            true
        } else {
            let c = bytes[abs_pos - 1] as char;
            !c.is_ascii_alphanumeric() && c != '_'
        };
        // Check char after keyword
        let after_pos = abs_pos + kw_len;
        let after_ok = if after_pos >= bytes.len() {
            true
        } else {
            let c = bytes[after_pos] as char;
            !c.is_ascii_alphanumeric() && c != '_'
        };
        if before_ok && after_ok {
            return true;
        }
        // Advance past this match to continue scanning
        start = abs_pos + kw_len;
        if start >= bytes.len() {
            break;
        }
    }
    false
}

/// Route to Z3 backend for nonlinear inequalities.
fn check_inequality_z3(expr: &str) -> VerificationResult {
    use crate::verification::python_bridge;
    use serde_json::json;

    if !python_bridge::z3_available() {
        return VerificationResult {
            check_name: "math_prove_inequality".to_string(),
            status: VerificationStatus::Warn,
            details: format!(
                "Z3 not available for nonlinear inequality: {expr} — \
                 install z3-solver: uv pip install z3-solver"
            ),
            evidence_path: None,
        };
    }

    // Extract variable names from the expression
    let re_vars = regex::Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*")
        .expect("valid regex");
    let known_keywords = [
        "sin", "cos", "tan", "sqrt", "abs", "exp", "log", "ln",
        "And", "Or", "Not", "Implies", "True", "False",
        "pi", "e",
    ];
    let mut vars: Vec<String> = re_vars
        .find_iter(expr)
        .map(|m| m.as_str().to_string())
        .filter(|v| !known_keywords.contains(&v.as_str()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    vars.sort();

    let params = json!({
        "expression": expr,
        "variables": vars,
        "timeout_ms": 10000,
    });

    match python_bridge::call_math_backend("z3_check", params) {
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
                        check_name: "math_prove_inequality".to_string(),
                        status: VerificationStatus::Pass,
                        details: format!("Consistent (Z3 sat). Model: {model}"),
                        evidence_path: None,
                    }
                }
                "unsat" => VerificationResult {
                    check_name: "math_prove_inequality".to_string(),
                    status: VerificationStatus::Fail,
                    details: format!("Inconsistent (Z3 unsat): {expr}"),
                    evidence_path: None,
                },
                _ => VerificationResult {
                    check_name: "math_prove_inequality".to_string(),
                    status: VerificationStatus::Warn,
                    details: format!("Z3 result: {status_str}"),
                    evidence_path: None,
                },
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_prove_inequality".to_string(),
            status: VerificationStatus::Warn,
            details: format!("Z3 error: {e}"),
            evidence_path: None,
        },
    }
}

pub fn check_inequality(expr: &str, timeout_ms: Option<u64>) -> VerificationResult {
    check_inequality_with_name(expr, timeout_ms, "math_prove_inequality")
}

/// Like `check_inequality` but with an explicit check name (for tool-layer reuse).
pub fn check_inequality_with_name(
    expr: &str,
    timeout_ms: Option<u64>,
    check_name: &str,
) -> VerificationResult {
    // Route to Z3 for nonlinear expressions
    if is_nonlinear(expr) {
        tracing::debug!("[inequality] nonlinear expression, routing to Z3: {expr}");
        return check_inequality_z3(expr);
    }

    // Linear case: use existing minilp solver
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
    use crate::verification::python_bridge;
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

    #[test]
    fn test_solve_system_timeout() {
        // timeout_ms=0 should trigger instant timeout since recv_timeout(0ms)
        // returns immediately if the channel is empty. The spawned thread
        // cannot have sent a result before recv_timeout checks.
        let ineq = Inequality::new(vec![1.0], vec!["x".into()], InequalitySense::Gt, 0.0);
        let system = InequalitySystem::new(vec![ineq]);
        let result = solve_system(&system, Some(0));
        match result {
            FeasibilityResult::Timeout { timeout_ms } => {
                assert_eq!(timeout_ms, 0);
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[test]
    fn test_solve_via_minilp_strict_lt() {
        // x < 10 — strict less-than with epsilon margin.
        // minilp with zero objective may return any feasible value, so we
        // check approximately: the solver's epsilon-adjusted constraint is
        // x <= 10 - eps, so x(solver) should be close to or less than 10.
        let ineq = Inequality::new(vec![1.0], vec!["x".into()], InequalitySense::Lt, 10.0);
        let system = InequalitySystem::new(vec![ineq]);
        let result = solve_system(&system, Some(1000));
        match &result {
            FeasibilityResult::Feasible { model } => {
                let x = model.get("x").copied().unwrap_or(f64::NAN);
                assert!(
                    x <= 10.0 + 1e-9,
                    "expected x approximately <= 10, got x={x}"
                );
            }
            other => panic!("expected Feasible, got: {other:?}"),
        }
    }

    #[test]
    fn test_solve_via_minilp_strict_gt() {
        // x > 5 — strict greater-than with epsilon margin.
        // minilp with zero objective may return any feasible value, so we
        // check approximately: the solver's epsilon-adjusted constraint is
        // x >= 5 + eps, so x(solver) should be close to or above 5.
        let ineq = Inequality::new(vec![1.0], vec!["x".into()], InequalitySense::Gt, 5.0);
        let system = InequalitySystem::new(vec![ineq]);
        let result = solve_system(&system, Some(1000));
        match &result {
            FeasibilityResult::Feasible { model } => {
                let x = model.get("x").copied().unwrap_or(f64::NAN);
                assert!(
                    x >= 5.0 - 1e-9,
                    "expected x approximately >= 5, got x={x}"
                );
            }
            other => panic!("expected Feasible, got: {other:?}"),
        }
    }

    #[test]
    fn test_solve_via_minilp_constant_only() {
        // Constants only — no variables. This exercises the all_vars.is_empty() path.
        // (1) 0 < 5 (satisfied, constant-only Lt)
        let sys1 = InequalitySystem::new(vec![Inequality::new(
            vec![],
            vec![],
            InequalitySense::Lt,
            5.0,
        )]);
        let r1 = solve_system(&sys1, Some(1000));
        assert!(r1.is_feasible(), "0 < 5 should be feasible");

        // (2) 0 > 5 (violated, constant-only Gt)
        let sys2 = InequalitySystem::new(vec![Inequality::new(
            vec![],
            vec![],
            InequalitySense::Gt,
            5.0,
        )]);
        let r2 = solve_system(&sys2, Some(1000));
        assert!(r2.is_infeasible(), "0 > 5 should be infeasible");

        // (3) 0 <= 0 (satisfied, constant-only Le)
        let sys3 = InequalitySystem::new(vec![Inequality::new(
            vec![],
            vec![],
            InequalitySense::Le,
            0.0,
        )]);
        let r3 = solve_system(&sys3, Some(1000));
        assert!(r3.is_feasible(), "0 <= 0 should be feasible");

        // (4) 0 == 1 (violated, constant-only Eq)
        let sys4 = InequalitySystem::new(vec![Inequality::new(
            vec![],
            vec![],
            InequalitySense::Eq,
            1.0,
        )]);
        let r4 = solve_system(&sys4, Some(1000));
        assert!(r4.is_infeasible(), "0 == 1 should be infeasible");
    }

    #[test]
    fn test_solve_via_minilp_constant_violation() {
        // Unsatisfiable constant constraints (no variables).
        // (1) 0 < 0 (violated Lt)
        let sys1 = InequalitySystem::new(vec![Inequality::new(
            vec![], vec![], InequalitySense::Lt, 0.0,
        )]);
        let r1 = solve_system(&sys1, Some(1000));
        assert!(r1.is_infeasible(), "0 < 0 should be infeasible");

        // (2) 0 <= -1 (violated Le)
        let sys2 = InequalitySystem::new(vec![Inequality::new(
            vec![], vec![], InequalitySense::Le, -1.0,
        )]);
        let r2 = solve_system(&sys2, Some(1000));
        assert!(r2.is_infeasible(), "0 <= -1 should be infeasible");

        // (3) 0 >= 5 (violated Ge)
        let sys3 = InequalitySystem::new(vec![Inequality::new(
            vec![], vec![], InequalitySense::Ge, 5.0,
        )]);
        let r3 = solve_system(&sys3, Some(1000));
        assert!(r3.is_infeasible(), "0 >= 5 should be infeasible");

        // (4) 0 == 0 (satisfied Eq)
        let sys4 = InequalitySystem::new(vec![Inequality::new(
            vec![], vec![], InequalitySense::Eq, 0.0,
        )]);
        let r4 = solve_system(&sys4, Some(1000));
        assert!(r4.is_feasible(), "0 == 0 should be feasible");

        // (5) 0 >= -1 (satisfied Ge)
        let sys5 = InequalitySystem::new(vec![Inequality::new(
            vec![], vec![], InequalitySense::Ge, -1.0,
        )]);
        let r5 = solve_system(&sys5, Some(1000));
        assert!(r5.is_feasible(), "0 >= -1 should be feasible");
    }

    // ── is_nonlinear heuristic tests ──

    #[test]
    fn test_is_nonlinear_positive() {
        // Expressions that SHOULD be detected as nonlinear
        assert!(is_nonlinear("x^2 >= 0"), "x^2 should be nonlinear");
        assert!(is_nonlinear("sin(x) > 0"), "sin( should be nonlinear");
        assert!(is_nonlinear("cos(x) >= -1"), "cos( should be nonlinear");
        assert!(is_nonlinear("sqrt(x) < 5"), "sqrt( should be nonlinear");
        assert!(is_nonlinear("abs(x) <= 10"), "abs( should be nonlinear");
        assert!(is_nonlinear("exp(x) > 0"), "exp( should be nonlinear");
        assert!(is_nonlinear("log(x) <= 1"), "log( should be nonlinear");
        assert!(is_nonlinear("ln(x) > 0"), "ln( should be nonlinear");
        assert!(is_nonlinear("tan(x) <= 2"), "tan( should be nonlinear");
        assert!(is_nonlinear("x + pi >= 3"), "pi should be nonlinear");
        assert!(is_nonlinear("x*y <= 0"), "product via * should be nonlinear");
        assert!(is_nonlinear("x ** 2 > 1"), "** should be nonlinear");
    }

    #[test]
    fn test_is_nonlinear_negative() {
        // Linear expressions that should NOT be detected as nonlinear.
        // Note: the heuristic treats `*` as a nonlinear marker (potential
        // product of variables), so scalar multiplication like `2*y` or `3*x`
        // produces a false positive. We avoid such expressions here.

        // Simple variable inequalities
        assert!(!is_nonlinear("x > 0"), "x > 0 should be linear");
        assert!(!is_nonlinear("x < 0"), "x < 0 should be linear");
        assert!(!is_nonlinear("x == 10"), "x == 10 should be linear");
        assert!(!is_nonlinear("x >= 0"), "x >= 0 should be linear");
        assert!(!is_nonlinear("x <= 0"), "x <= 0 should be linear");

        // Sums of variables (no explicit coefficient multiplier)
        assert!(!is_nonlinear("x + y <= 5"), "x + y <= 5 should be linear");
        assert!(!is_nonlinear("a + b + c <= 3"), "a + b + c <= 3 should be linear");
        assert!(!is_nonlinear("x - y >= 1"), "x - y >= 1 should be linear");

        // Multiple variables, no *
        assert!(!is_nonlinear("x + y + z == 0"), "x + y + z == 0 should be linear");
    }

    #[test]
    fn test_is_nonlinear_constant_times_variable_no_longer_false_positive() {
        // Fixed: constant × variable (e.g. `2*x`, `3*x - y`) is LINEAR
        // and is no longer incorrectly flagged as nonlinear.
        assert!(!is_nonlinear("2*x <= 5"), "2*x is linear (constant × variable)");
        assert!(!is_nonlinear("x + 2*y <= 5"), "x + 2*y is linear");
        assert!(!is_nonlinear("3*x - y >= 1"), "3*x - y is linear");
    }

    #[test]
    fn test_is_nonlinear_pi_no_longer_false_positive() {
        // Fixed: `pi` is now recognized as a whole-word boundary check,
        // so `pivot` (contains "pi" as substring) is correctly linear.
        assert!(
            !is_nonlinear("pivot > 0"),
            "'pivot' contains 'pi' substring but should NOT be nonlinear"
        );
        assert!(
            !is_nonlinear("spiral >= 0"),
            "'spiral' contains 'pi' but should NOT be nonlinear"
        );

        // Actual uses of `pi` constant ARE nonlinear
        assert!(
            is_nonlinear("x + pi <= 10"),
            "x + pi should be nonlinear"
        );
        assert!(
            is_nonlinear("sin(x) + pi"),
            "sin(x) + pi should be nonlinear"
        );
    }

    // ── Z3 nonlinear inequality path tests ──

    #[test]
    fn test_check_inequality_nonlinear_z3_path() {
        // x^2 >= 0 is a nonlinear inequality that should route to the Z3 backend.
        // When Z3 is available, it should return Pass (x^2 >= 0 is always true).
        if !python_bridge::z3_available() {
            tracing::info!("Z3 not available — skipping nonlinear Z3 path test");
            return;
        }
        let result = check_inequality("x^2 >= 0", Some(10000));
        assert_eq!(
            result.status,
            VerificationStatus::Pass,
            "x^2 >= 0 should be Pass when Z3 available, got: {}",
            result.details
        );
        assert_eq!(result.check_name, "math_prove_inequality");
    }

    #[test]
    fn test_check_inequality_nonlinear_z3_unavailable() {
        // When Z3 is not available, a nonlinear inequality should return Warn
        // with a helpful installation message.
        if python_bridge::z3_available() {
            tracing::info!("Z3 is available — skipping unavailable-path test");
            return;
        }
        let result = check_inequality("x^2 >= 0", Some(5000));
        assert_eq!(
            result.status,
            VerificationStatus::Warn,
            "expected Warn when Z3 unavailable, got: {:?}",
            result.status
        );
        assert!(
            result.details.contains("Z3 not available"),
            "details should mention Z3 unavailability, got: {}",
            result.details
        );
        assert!(
            result.details.contains("z3-solver"),
            "details should mention installing z3-solver, got: {}",
            result.details
        );
        assert_eq!(result.check_name, "math_prove_inequality");
    }

    // ── is_nonlinear tests ──

    #[test]
    fn test_is_nonlinear_power() {
        assert!(is_nonlinear("x^2 + y <= 10"), "power operator (^)");
        assert!(is_nonlinear("x**2"), "power operator (**)");
    }

    #[test]
    fn test_is_nonlinear_functions() {
        assert!(is_nonlinear("sin(x) <= 1"), "sin function");
        assert!(is_nonlinear("cos(x) >= -1"), "cos function");
        assert!(is_nonlinear("sqrt(x) >= 0"), "sqrt function");
        assert!(is_nonlinear("exp(x) <= 10"), "exp function");
        assert!(is_nonlinear("log(x) > 0"), "log function");
        assert!(is_nonlinear("abs(x) >= 0"), "abs function");
    }

    #[test]
    fn test_is_nonlinear_variable_product() {
        // Variable × Variable → nonlinear
        assert!(is_nonlinear("x*y <= 10"), "variable × variable");
        assert!(is_nonlinear("x*y*z >= 0"), "three variables product");
    }

    #[test]
    fn test_is_nonlinear_paren_product() {
        // (expr) × variable → nonlinear
        assert!(is_nonlinear("(x+1)*y <= 10"), "paren × variable");
    }

    #[test]
    fn test_is_linear_constant_times_variable() {
        // Constant × Variable → LINEAR
        assert!(!is_nonlinear("2*x <= 10"), "constant × variable (2*x)");
        assert!(!is_nonlinear("x*2 <= 10"), "variable × constant (x*2)");
        assert!(!is_nonlinear("3*x + 5*y <= 10"), "multiple constant×variable");
    }

    #[test]
    fn test_is_linear_simple() {
        assert!(!is_nonlinear("x + y <= 10"), "linear: x + y <= 10");
        assert!(!is_nonlinear("x - y > 0"), "linear: x - y > 0");
        assert!(!is_nonlinear("x >= 0"), "linear: x >= 0");
        assert!(!is_nonlinear("x + 2*y - 3*z <= 5"), "linear multi-var");
    }

    #[test]
    fn test_is_nonlinear_with_pi() {
        assert!(is_nonlinear("x + pi <= 10"), "pi constant");
        assert!(is_nonlinear("sin(x) + pi"), "pi in expression");
    }

    #[test]
    fn test_is_linear_non_standard_operators() {
        // Division by constant → still linear if no other nonlinearity
        assert!(!is_nonlinear("x/2 <= 10"), "x/2 is linear");
    }
}
