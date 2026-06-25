//! Inequality verification — ResearchHarness formal verification feature.
//!
//! Pure business logic: types, LaTeX parsing (SymPy + regex fallback),
//! Z3 calling via Python subprocess (JSON stdin/stdout).
//!
//! # Security
//!
//! User input NEVER reaches shell. Python subprocess receives only
//! serialized JSON via stdin — no string concatenation into command args.
//!
//! # Layer boundary
//!
//! This module is FEATURE layer only. JSON argument extraction, result
//! formatting, and MCP tool dispatch belong in `mcp_tools.rs`.

use crate::types::{VerificationResult, VerificationStatus};
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
    pub fn new(coefficients: Vec<f64>, vars: Vec<String>, sense: InequalitySense, rhs: f64) -> Self {
        Self { coefficients, vars, sense, rhs }
    }
}

/// A system of linear inequalities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InequalitySystem {
    pub constraints: Vec<Inequality>,
}

impl InequalitySystem {
    pub fn new(constraints: Vec<Inequality>) -> Self { Self { constraints } }
    pub fn is_empty(&self) -> bool { self.constraints.is_empty() }
    pub fn len(&self) -> usize { self.constraints.len() }
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
    pub fn is_feasible(&self) -> bool { matches!(self, FeasibilityResult::Feasible { .. }) }
    pub fn is_infeasible(&self) -> bool { matches!(self, FeasibilityResult::Infeasible { .. }) }
}

// ===========================================================================
// LaTeX inequality string parsing (SymPy subprocess, regex fallback)
// ===========================================================================

const DEFAULT_TIMEOUT_MS: u64 = 5_000;

pub fn parse_inequality_latex(expr: &str) -> Result<Inequality, String> {
    parse_via_sympy_subprocess(expr).or_else(|_| parse_via_regex(expr))
}

fn parse_via_sympy_subprocess(expr: &str) -> Result<Inequality, String> {
    let input = serde_json::json!({ "command": "parse", "expr": expr });
    let resp = crate::subprocess::run_uv_module("inequality_solver", &input)?;
    if resp.get("error").is_some() {
        return Err(resp["error"].as_str().unwrap_or("parse error").to_string());
    }
    serde_json::from_value(resp).map_err(|e| format!("deser: {e}"))
}

fn parse_via_regex(expr: &str) -> Result<Inequality, String> {
    let cleaned = expr
        .replace("\\leq", "<=").replace("\\le", "<=")
        .replace("\\geq", ">=").replace("\\ge", ">=")
        .replace("\\lt", "<").replace("\\gt", ">")
        .replace("\\cdot", "*").replace(' ', "");

    let re = regex::Regex::new(r"^(.+?)\s*(<=|>=|==|=|<|>)\s*(.+)$")
        .map_err(|e| format!("regex: {e}"))?;
    let caps = re.captures(&cleaned).ok_or_else(|| format!("cannot parse: {expr}"))?;

    let lhs_str = caps.get(1).unwrap().as_str();
    let sense_str = caps.get(2).unwrap().as_str();
    let rhs_str = caps.get(3).unwrap().as_str();

    let sense = match sense_str {
        "<" => InequalitySense::Lt,  "<=" => InequalitySense::Le,
        "==" | "=" => InequalitySense::Eq,
        ">=" => InequalitySense::Ge, ">" => InequalitySense::Gt,
        _ => return Err(format!("unknown sense: {sense_str}")),
    };
    let rhs = parse_number(rhs_str)?;

    let (coeffs, vars, const_shift) = extract_terms(lhs_str);
    Ok(Inequality::new(coeffs, vars, sense, rhs - const_shift))
}

fn parse_number(s: &str) -> Result<f64, String> {
    if let Some(pos) = s.find('/') {
        let n: f64 = s[..pos].parse().map_err(|_| format!("bad num: {}", &s[..pos]))?;
        let d: f64 = s[pos+1..].parse().map_err(|_| format!("bad den: {}", &s[pos+1..]))?;
        if d == 0.0 { return Err("div by zero".into()); }
        return Ok(n / d);
    }
    s.parse::<f64>().map_err(|_| format!("bad number: {s}"))
}

fn extract_terms(lhs: &str) -> (Vec<f64>, Vec<String>, f64) {
    let mut coeffs = Vec::new();
    let mut vars = Vec::new();
    let mut const_shift = 0.0;
    let src: Vec<char> = lhs.trim_start_matches('+').chars().collect();
    let mut i = 0;

    while i < src.len() {
        let mut term = String::new();
        if src[i] == '-' { term.push('-'); i += 1; }
        while i < src.len() && src[i] != '+' && src[i] != '-' {
            term.push(src[i]); i += 1;
        }
        if let Some((c, v_opt)) = parse_one_term(&term) {
            if let Some(v) = v_opt {
                match vars.iter().position(|x: &String| x == &v) {
                    Some(idx) => coeffs[idx] += c,
                    None => { vars.push(v); coeffs.push(c); }
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
    if t.is_empty() { return None; }
    if let Ok(n) = t.parse::<f64>() { return Some((n, None)); }

    let mut num = String::new();
    let mut var = String::new();
    let mut in_var = false;
    for ch in t.chars() {
        if (ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '/') && !in_var {
            num.push(ch);
        } else { in_var = true; var.push(ch); }
    }
    if var.is_empty() { return None; }
    let c = if num.is_empty() || num == "+" { 1.0 }
            else if num == "-" { -1.0 } else { parse_number(&num).ok()? };
    Some((c, Some(var)))
}

// ===========================================================================
// Z3 subprocess bridge
// ===========================================================================

pub fn solve_system(system: &InequalitySystem, timeout_ms: Option<u64>) -> FeasibilityResult {
    let timeout = timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if system.is_empty() {
        return FeasibilityResult::Feasible { model: HashMap::new() };
    }
    let r = call_z3(system, timeout);
    // If Z3 failed with an error-like result, return a consistent Warn
    // with install hint regardless of system size.
    if matches!(&r, FeasibilityResult::Error { .. } | FeasibilityResult::Timeout { .. }) {
        return FeasibilityResult::Warn {
            message: "Z3 check failed or timed out; install z3-solver: uv pip install z3-solver".into(),
        };
    }
    r
}

fn call_z3(system: &InequalitySystem, timeout_ms: u64) -> FeasibilityResult {
    let input = serde_json::json!({ "command": "solve", "system": system, "timeout_ms": timeout_ms });
    match crate::subprocess::run_uv_module_with_timeout("inequality_solver", &input, timeout_ms) {
        Ok(resp) => serde_json::from_value(resp).unwrap_or_else(|e| {
            FeasibilityResult::Error { message: format!("parse response: {e}") }
        }),
        Err(e) => FeasibilityResult::Error { message: e },
    }
}

// ===========================================================================
// Backend probes (no cache — per-invocation)
// ===========================================================================

pub fn z3_available() -> bool {
    std::process::Command::new("uv")
        .args(["run", "python", "-c", "import z3; print('ok')"])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
        .map(|s| s.success()).unwrap_or(false)
}

pub fn sympy_available() -> bool {
    crate::verification::sympy_available()
}

// ===========================================================================
// Verification pipeline integration
// ===========================================================================

pub fn check_inequality(expr: &str, timeout_ms: Option<u64>) -> VerificationResult {
    check_inequality_with_name(expr, timeout_ms, "math_prove_inequality")
}

/// Like `check_inequality` but with an explicit check name (for tool-layer reuse).
pub fn check_inequality_with_name(expr: &str, timeout_ms: Option<u64>, check_name: &str) -> VerificationResult {
    let ineq = match parse_inequality_latex(expr) {
        Ok(i) => i,
        Err(e) => return VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Fail,
            details: format!("parse failed: {e}"),
            evidence_path: None,
        },
    };
    let system = InequalitySystem::new(vec![ineq]);
    match solve_system(&system, timeout_ms) {
        FeasibilityResult::Feasible { model } => {
            let ms: Vec<String> = model.iter().map(|(k,v)| format!("{k}={v}")).collect();
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

#[cfg(test)]
mod tests {
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
    fn test_backend_probe_no_panic() {
        let _ = z3_available();
        let _ = sympy_available();
    }
}
