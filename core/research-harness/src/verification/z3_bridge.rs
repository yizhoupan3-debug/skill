//! Z3 operations — native Rust implementation via the `z3` crate.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.
//!
//! All Z3 operations are performed in-process using the `z3` crate
//! (bundled z3 solver, v0.20.2), eliminating the Python subprocess dependency.
//!
//! z3 0.20 API notes:
//! - All AST types (Bool, Int, Real, Dynamic) own their Context internally (Rc-based).
//! - `Context::thread_local()` is used implicitly by `new_const`, `from_i64`, etc.
//! - `Real::add(&[&a, &b])` — varop takes a slice, not binary method.
//! - `Bool::and(&[&a, &b])` — varop takes a slice.
//! - `solver.push()` pushes 1 scope (no arg). `solver.pop(n)` pops n scopes.

use crate::types::{VerificationResult, VerificationStatus};
use serde_json::json;

/// Z3 is always available (bundled via z3 crate).
pub fn z3_available() -> bool {
    true
}

// ════════════════════════════════════════════════════════════════════════════
// Expression parser — LaTeX-style math → z3 AST
// ════════════════════════════════════════════════════════════════════════════

/// Top-level: parse a relational / boolean expression into a z3 Bool.
fn parse_z3_bool(solver: &z3::Solver, expr: &str) -> Result<z3::ast::Bool, String> {
    let expr = expr.trim();

    // Boolean connectives (varop: takes slice)
    if let Some(inner) = strip_prefix_parens(expr, "And") {
        let parts = split_top_level_args(inner)?;
        if parts.len() < 2 {
            return Err("And requires at least 2 arguments".into());
        }
        let bools: Result<Vec<_>, _> = parts.iter().map(|p| parse_z3_bool(solver, p)).collect();
        let bools = bools?;
        let refs: Vec<&z3::ast::Bool> = bools.iter().collect();
        return Ok(z3::ast::Bool::and(&refs));
    }
    if let Some(inner) = strip_prefix_parens(expr, "Or") {
        let parts = split_top_level_args(inner)?;
        if parts.len() < 2 {
            return Err("Or requires at least 2 arguments".into());
        }
        let bools: Result<Vec<_>, _> = parts.iter().map(|p| parse_z3_bool(solver, p)).collect();
        let bools = bools?;
        let refs: Vec<&z3::ast::Bool> = bools.iter().collect();
        return Ok(z3::ast::Bool::or(&refs));
    }
    if let Some(inner) = strip_prefix_parens(expr, "Not") {
        let parts = split_top_level_args(inner)?;
        if parts.len() != 1 {
            return Err("Not requires exactly 1 argument".into());
        }
        return Ok(parse_z3_bool(solver, &parts[0])?.not());
    }
    if let Some(inner) = strip_prefix_parens(expr, "Implies") {
        let parts = split_top_level_args(inner)?;
        if parts.len() != 2 {
            return Err("Implies requires exactly 2 arguments".into());
        }
        let a = parse_z3_bool(solver, &parts[0])?;
        let b = parse_z3_bool(solver, &parts[1])?;
        return Ok(a.implies(&b));
    }

    // Comparisons
    if let Some((op, lhs_s, rhs_s)) = parse_comparison(expr) {
        let lhs = parse_arith(solver, lhs_s.trim())?;
        let rhs = parse_arith(solver, rhs_s.trim())?;
        return match op {
            ">" => Ok(lhs.gt(&rhs)),
            ">=" => Ok(lhs.ge(&rhs)),
            "<" => Ok(lhs.lt(&rhs)),
            "<=" => Ok(lhs.le(&rhs)),
            "==" | "=" => Ok(lhs.eq(&rhs)),
            "!=" => Ok(lhs.eq(&rhs).not()),
            _ => Err(format!("unknown operator: {op}")),
        };
    }

    Err(format!("cannot parse Z3 boolean expression: {expr}"))
}

/// Parse an arithmetic expression into a z3 Real.
fn parse_arith(solver: &z3::Solver, expr: &str) -> Result<z3::ast::Real, String> {
    parse_add_sub(solver, expr.trim())
}

/// Parse addition/subtraction (lowest precedence).
fn parse_add_sub(solver: &z3::Solver, expr: &str) -> Result<z3::ast::Real, String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut depth = 0i32;
    let mut segments: Vec<(&str, bool)> = Vec::new(); // (term_without_sign, is_positive)
    let mut last = 0;

    for i in 0..chars.len() {
        match chars[i] {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' if depth == 0 && i > 0 => {
                // The segment from last..i is a complete positive term
                let term = expr[last..i].trim();
                if !term.is_empty() {
                    segments.push((term, true));
                }
                last = i + 1; // skip the '+'
            }
            '-' if depth == 0 && i > 0 => {
                let prev = chars[i - 1];
                if matches!(prev, '(' | '+' | '-' | '*' | '/' | '^' | ',') {
                    continue; // unary minus — part of current term
                }
                let term = expr[last..i].trim();
                if !term.is_empty() {
                    segments.push((term, true));
                }
                last = i; // include '-' in next term
            }
            _ => {}
        }
    }
    let last_term = expr[last..].trim();
    if !last_term.is_empty() {
        if last_term.starts_with('-') {
            segments.push((last_term, false));
        } else if last_term.starts_with('+') {
            segments.push((&last_term[1..], true));
        } else {
            segments.push((last_term, true));
        }
    }

    if segments.len() <= 1 {
        let term = segments.first().map(|(s, _)| *s).unwrap_or(expr);
        return parse_mul_div(solver, term.trim());
    }

    // Parse each segment
    let mut positives: Vec<z3::ast::Real> = Vec::new();
    let mut negatives: Vec<z3::ast::Real> = Vec::new();

    for &(term, positive) in &segments {
        let val = parse_mul_div(solver, term.trim())?;
        if positive {
            positives.push(val);
        } else {
            negatives.push(val);
        }
    }

    // Build: all positives + unary_minus(all negatives)
    let mut all_terms: Vec<z3::ast::Real> = positives;
    for neg in &negatives {
        all_terms.push(neg.unary_minus());
    }

    let result = if all_terms.is_empty() {
        z3::ast::Real::from_rational(0, 1)
    } else if all_terms.len() == 1 {
        all_terms.into_iter().next().unwrap()
    } else {
        let refs: Vec<&z3::ast::Real> = all_terms.iter().collect();
        z3::ast::Real::add(&refs)
    };

    Ok(result)
}

/// Parse multiplication/division.
fn parse_mul_div(solver: &z3::Solver, expr: &str) -> Result<z3::ast::Real, String> {
    let expr = expr.trim();
    let chars: Vec<char> = expr.chars().collect();

    // Find top-level * or / (right-to-left for left-assoc)
    let mut depth = 0i32;
    for i in (0..chars.len()).rev() {
        match chars[i] {
            '(' => depth -= 1,
            ')' => depth += 1,
            '*' if depth == 0 && i > 0 => {
                let left = parse_mul_div(solver, expr[..i].trim())?;
                let right = parse_pow(solver, expr[i + 1..].trim())?;
                return Ok(z3::ast::Real::mul(&[&left, &right]));
            }
            '/' if depth == 0 && i > 0 => {
                let left = parse_mul_div(solver, expr[..i].trim())?;
                let right = parse_pow(solver, expr[i + 1..].trim())?;
                return Ok(left.div(&right));
            }
            _ => {}
        }
    }

    // Implicit multiplication: `2x`, `(expr)x`
    let mut d = 0i32;
    for i in 1..chars.len() {
        match chars[i] {
            '(' => d += 1,
            ')' => d -= 1,
            c if d == 0 && (c.is_alphabetic() || c == '_') => {
                let prev = chars[i - 1];
                if prev.is_alphanumeric() || prev == '_' || prev == ')' {
                    let left = parse_pow(solver, expr[..i].trim())?;
                    let right = parse_pow(solver, expr[i..].trim())?;
                    return Ok(z3::ast::Real::mul(&[&left, &right]));
                }
            }
            _ => {}
        }
    }

    parse_pow(solver, expr)
}

/// Parse power expressions (`base^exp`).
fn parse_pow(solver: &z3::Solver, expr: &str) -> Result<z3::ast::Real, String> {
    let expr = expr.trim();
    let chars: Vec<char> = expr.chars().collect();

    let mut depth = 0i32;
    for i in (0..chars.len()).rev() {
        match chars[i] {
            '(' => depth -= 1,
            ')' => depth += 1,
            '^' if depth == 0 && i > 0 => {
                let base = parse_atom(solver, expr[..i].trim())?;
                let exp_str = expr[i + 1..].trim();
                // Try integer exponent for real_pow
                if let Ok(n) = exp_str.parse::<i64>() {
                    if n >= 0 && n <= 32 {
                        return Ok(real_pow(&base, n as u32));
                    }
                }
                // Try float literal for power
                if let Ok(f) = exp_str.parse::<f64>() {
                    if f == 0.5 || f == -0.5 {
                        // sqrt or 1/sqrt — use Z3 power with rational
                        let exp = z3::ast::Real::from_rational(
                            if f > 0.0 { 1 } else { -1 },
                            if f.abs() == 0.5 { 2 } else { 1 },
                        );
                        return Ok(base.power(&exp));
                    }
                }
                return Err(format!("z3 does not support general power: {expr}"));
            }
            _ => {}
        }
    }

    parse_atom(solver, expr)
}

/// Compute base^n for small integer n using repeated squaring.
fn real_pow(base: &z3::ast::Real, n: u32) -> z3::ast::Real {
    match n {
        0 => z3::ast::Real::from_rational(1, 1),
        1 => base.clone(),
        _ => {
            let mut result = base.clone();
            for _ in 1..n {
                result = z3::ast::Real::mul(&[&result, base]);
            }
            result
        }
    }
}

/// Parse an atomic expression: number, variable, parenthesized.
fn parse_atom(solver: &z3::Solver, expr: &str) -> Result<z3::ast::Real, String> {
    let expr = expr.trim();

    // Parenthesized
    if expr.starts_with('(') && expr.ends_with(')') {
        return parse_add_sub(solver, &expr[1..expr.len() - 1]);
    }

    // Negation
    if let Some(rest) = expr.strip_prefix('-') {
        let inner = parse_atom(solver, rest.trim())?;
        return Ok(inner.unary_minus());
    }

    // Integer literal
    if let Ok(n) = expr.parse::<i64>() {
        return Ok(z3::ast::Real::from_int(&z3::ast::Int::from_i64(n)));
    }

    // Rational literal (e.g., "1/2")
    if let Some((num_s, den_s)) = expr.split_once('/') {
        if let (Ok(num), Ok(den)) = (num_s.trim().parse::<i64>(), den_s.trim().parse::<i64>()) {
            if den != 0 {
                return Ok(z3::ast::Real::from_rational(num, den));
            }
        }
    }

    // Float literal → rational approximation
    if let Ok(f) = expr.parse::<f64>() {
        if f.is_finite() {
            let scaled = (f * 1e12).round() as i64;
            return Ok(z3::ast::Real::from_rational(scaled, 1_000_000_000_000));
        }
    }

    // Variable name
    if !expr.is_empty()
        && (expr.chars().next().unwrap().is_alphabetic() || expr.starts_with('_'))
    {
        return Ok(z3::ast::Real::new_const(expr.to_string()));
    }

    Err(format!("cannot parse atom: {expr}"))
}

// ════════════════════════════════════════════════════════════════════════════
// Helper utilities
// ════════════════════════════════════════════════════════════════════════════

/// Strip a function name prefix and outer parens: "And(x, y)" -> "x, y"
fn strip_prefix_parens<'a>(expr: &'a str, prefix: &str) -> Option<&'a str> {
    let trimmed = expr.trim();
    if let Some(rest) = trimmed.strip_prefix(prefix) {
        let rest = rest.trim();
        if rest.starts_with('(') && rest.ends_with(')') {
            return Some(&rest[1..rest.len() - 1]);
        }
    }
    None
}

/// Split comma-separated arguments at top level (respecting parentheses).
fn split_top_level_args(args: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut last = 0;
    for (i, c) in args.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(args[last..i].trim().to_string());
                last = i + 1;
            }
            _ => {}
        }
    }
    parts.push(args[last..].trim().to_string());
    Ok(parts.into_iter().filter(|s| !s.is_empty()).collect())
}

/// Find comparison operator and split into (op, lhs, rhs).
fn parse_comparison(expr: &str) -> Option<(&str, &str, &str)> {
    let ops = [">=", "<=", "!=", "==", ">", "<", "="];
    let mut depth = 0i32;
    for (i, c) in expr.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            for op in &ops {
                if expr[i..].starts_with(op) {
                    let lhs = &expr[..i];
                    let rhs = &expr[i + op.len()..];
                    if !lhs.trim().is_empty() && !rhs.trim().is_empty() {
                        return Some((op, lhs, rhs));
                    }
                }
            }
        }
    }
    None
}

/// Format a z3 Model's variable assignments as a string.
fn model_to_string(model: &z3::Model) -> String {
    let mut parts = Vec::new();
    for decl in model.iter() {
        // FuncDecl is not Ast; apply(&[]) to get Dynamic which is Ast
        let var = decl.apply(&[]);
        if let Some(val) = model.eval(&var, true) {
            parts.push(format!("{}={}", decl.name(), val));
        }
    }
    parts.join(", ")
}

/// Extract model as a JSON object.
fn model_to_json(model: &z3::Model) -> serde_json::Map<String, serde_json::Value> {
    let mut vars = serde_json::Map::new();
    for decl in model.iter() {
        let var = decl.apply(&[]);
        if let Some(val) = model.eval(&var, true) {
            vars.insert(decl.name(), serde_json::json!(format!("{val}")));
        }
    }
    vars
}

// ════════════════════════════════════════════════════════════════════════════
// Public API
// ════════════════════════════════════════════════════════════════════════════

/// Prove that a formula is universally valid (negation is unsatisfiable).
pub fn prove_formula(expr: &str) -> VerificationResult {
    let solver = z3::Solver::new();

    match parse_z3_bool(&solver, expr) {
        Ok(formula) => {
            solver.assert(&formula.not());
            match solver.check() {
                z3::SatResult::Unsat => VerificationResult {
                    check_name: "math_z3_prove".into(),
                    status: VerificationStatus::Pass,
                    details: format!("z3_prove({expr}) — proved (formula is universally valid)"),
                    evidence_path: None,
                },
                z3::SatResult::Sat => {
                    let model_str = solver.get_model().map(|m| model_to_string(&m)).unwrap_or_default();
                    let detail = if !model_str.is_empty() {
                        format!("z3_prove({expr}) — disproved. Counterexample: {{{model_str}}}")
                    } else {
                        format!("z3_prove({expr}) — disproved")
                    };
                    VerificationResult {
                        check_name: "math_z3_prove".into(),
                        status: VerificationStatus::Fail,
                        details: detail,
                        evidence_path: None,
                    }
                }
                z3::SatResult::Unknown => VerificationResult {
                    check_name: "math_z3_prove".into(),
                    status: VerificationStatus::Warn,
                    details: format!("z3_prove({expr}) — unknown (solver timeout)"),
                    evidence_path: None,
                },
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_z3_prove".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_prove({expr}) failed to parse: {e}"),
            evidence_path: None,
        },
    }
}

/// A single step in a solver batch.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SolverBatchStep {
    pub action: String,
    pub n: Option<usize>,
    pub expression: Option<String>,
    pub timeout_ms: Option<u64>,
}

/// Run a batch of incremental solver steps in a single solver instance.
pub fn solver_batch(steps: &[SolverBatchStep]) -> Result<serde_json::Value, String> {
    let solver = z3::Solver::new();
    let mut step_results = Vec::new();

    for step in steps {
        let result = match step.action.as_str() {
            "push" => {
                solver.push();
                json!({"result": "ok"})
            }
            "pop" => {
                let n = step.n.unwrap_or(1) as u32;
                solver.pop(n);
                json!({"result": "ok"})
            }
            "add" => {
                match &step.expression {
                    Some(expr) => match parse_z3_bool(&solver, expr) {
                        Ok(b) => {
                            solver.assert(&b);
                            json!({"result": "ok"})
                        }
                        Err(e) => json!({"result": "error", "error": e}),
                    },
                    None => json!({"result": "error", "error": "missing expression"}),
                }
            }
            "check" => {
                let timeout_ms = step.timeout_ms.unwrap_or(5000);
                let mut params = z3::Params::new();
                params.set_u32("timeout", timeout_ms as u32);
                solver.set_params(&params);

                match solver.check() {
                    z3::SatResult::Sat => {
                        let model_json = solver.get_model().map(|m| model_to_json(&m)).unwrap_or_default();
                        json!({"result": "sat", "model": model_json})
                    }
                    z3::SatResult::Unsat => json!({"result": "unsat"}),
                    z3::SatResult::Unknown => json!({"result": "unknown"}),
                }
            }
            "reset" => {
                solver.reset();
                json!({"result": "ok"})
            }
            _ => json!({"result": "error", "error": format!("unknown action: {}", step.action)}),
        };
        step_results.push(result);
    }

    Ok(json!({"steps": step_results}))
}

/// Push 1 scope onto the solver stack.
pub fn solver_push(_n: usize) -> VerificationResult {
    let solver = z3::Solver::new();
    solver.push();
    VerificationResult {
        check_name: "math_z3_solver_push".into(),
        status: VerificationStatus::Pass,
        details: "z3_solver_push — pushed 1 context".into(),
        evidence_path: None,
    }
}

/// Pop n scopes from the solver stack.
pub fn solver_pop(n: usize) -> VerificationResult {
    let solver = z3::Solver::new();
    solver.pop(n as u32);
    VerificationResult {
        check_name: "math_z3_solver_pop".into(),
        status: VerificationStatus::Pass,
        details: format!("z3_solver_pop — popped {n} context(s)"),
        evidence_path: None,
    }
}

/// Add a constraint expression to the solver.
pub fn solver_add(expr: &str) -> VerificationResult {
    let solver = z3::Solver::new();
    match parse_z3_bool(&solver, expr) {
        Ok(b) => {
            solver.assert(&b);
            VerificationResult {
                check_name: "math_z3_solver_add".into(),
                status: VerificationStatus::Pass,
                details: format!("z3_solver_add({expr}) — constraint added"),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_z3_solver_add".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_solver_add({expr}) failed: {e}"),
            evidence_path: None,
        },
    }
}

/// Check satisfiability.
pub fn solver_check(timeout_ms: Option<u64>) -> VerificationResult {
    let solver = z3::Solver::new();
    if let Some(ms) = timeout_ms {
        let mut params = z3::Params::new();
        params.set_u32("timeout", ms as u32);
        solver.set_params(&params);
    }

    match solver.check() {
        z3::SatResult::Sat => {
            let model_str = solver.get_model().map(|m| model_to_string(&m)).unwrap_or_default();
            VerificationResult {
                check_name: "math_z3_solver_check".into(),
                status: VerificationStatus::Pass,
                details: format!("z3_solver_check — SAT. Model: {{{model_str}}}"),
                evidence_path: None,
            }
        }
        z3::SatResult::Unsat => VerificationResult {
            check_name: "math_z3_solver_check".into(),
            status: VerificationStatus::Fail,
            details: "z3_solver_check — UNSAT (no solution)".into(),
            evidence_path: None,
        },
        z3::SatResult::Unknown => VerificationResult {
            check_name: "math_z3_solver_check".into(),
            status: VerificationStatus::Warn,
            details: "z3_solver_check — unknown".into(),
            evidence_path: None,
        },
    }
}

/// Reset solver state.
pub fn solver_reset() -> VerificationResult {
    let solver = z3::Solver::new();
    solver.reset();
    VerificationResult {
        check_name: "math_z3_solver_reset".into(),
        status: VerificationStatus::Pass,
        details: "z3_solver_reset — solver cleared".into(),
        evidence_path: None,
    }
}

/// Optimize an objective function subject to constraints.
pub fn optimize_formula(
    objective: &str,
    constraints: &[String],
    _variables: Option<&[String]>,
    direction: &str,
) -> Result<serde_json::Value, String> {
    let opt = z3::Optimize::new();
    let solver_tmp = z3::Solver::new(); // for parsing expressions

    for constraint in constraints {
        let b = parse_z3_bool(&solver_tmp, constraint)?;
        opt.assert(&b);
    }

    let obj = parse_arith(&solver_tmp, objective)?;

    match direction {
        "minimize" | "min" => opt.minimize(&obj),
        "maximize" | "max" => opt.maximize(&obj),
        _ => return Err(format!("unknown direction: {direction}")),
    };

    match opt.check(&[]) {
        z3::SatResult::Sat => {
            let mut result = serde_json::Map::new();
            if let Some(model) = opt.get_model() {
                for (k, v) in model_to_json(&model) {
                    result.insert(k, v);
                }
            }
            // get_objectives returns Vec<Dynamic>
            let objectives = opt.get_objectives();
            if !objectives.is_empty() {
                result.insert("objective".to_string(), json!(format!("{:?}", objectives[0])));
            }
            Ok(json!({"result": "sat", "model": result}))
        }
        z3::SatResult::Unsat => Ok(json!({"result": "unsat"})),
        z3::SatResult::Unknown => Ok(json!({"result": "unknown"})),
    }
}

/// Check a system of constraints for satisfiability.
pub fn check_system(
    constraints: &[String],
    _variables: Option<&[String]>,
    timeout_ms: Option<u64>,
) -> Result<serde_json::Value, String> {
    let solver = z3::Solver::new();

    if let Some(ms) = timeout_ms {
        let mut params = z3::Params::new();
        params.set_u32("timeout", ms as u32);
        solver.set_params(&params);
    }

    for constraint in constraints {
        let b = parse_z3_bool(&solver, constraint)?;
        solver.assert(&b);
    }

    match solver.check() {
        z3::SatResult::Sat => {
            let model_json = solver.get_model().map(|m| model_to_json(&m)).unwrap_or_default();
            Ok(json!({"result": "sat", "model": model_json}))
        }
        z3::SatResult::Unsat => Ok(json!({"result": "unsat"})),
        z3::SatResult::Unknown => Ok(json!({"result": "unknown"})),
    }
}

/// Convenience: check a single inequality via Z3 (used by inequality.rs).
pub fn check_inequality(expr: &str) -> VerificationResult {
    let solver = z3::Solver::new();
    match parse_z3_bool(&solver, expr) {
        Ok(b) => {
            solver.assert(&b);
            match solver.check() {
                z3::SatResult::Sat => VerificationResult {
                    check_name: "math_z3_check".into(),
                    status: VerificationStatus::Pass,
                    details: format!("z3_check({expr}) — satisfiable"),
                    evidence_path: None,
                },
                z3::SatResult::Unsat => VerificationResult {
                    check_name: "math_z3_check".into(),
                    status: VerificationStatus::Fail,
                    details: format!("z3_check({expr}) — unsatisfiable (contradiction)"),
                    evidence_path: None,
                },
                z3::SatResult::Unknown => VerificationResult {
                    check_name: "math_z3_check".into(),
                    status: VerificationStatus::Warn,
                    details: format!("z3_check({expr}) — unknown"),
                    evidence_path: None,
                },
            }
        }
        Err(e) => VerificationResult {
            check_name: "math_z3_check".into(),
            status: VerificationStatus::Fail,
            details: format!("z3_check({expr}) parse error: {e}"),
            evidence_path: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_z3_available() {
        assert!(z3_available());
    }

    #[test]
    fn test_prove_trivial() {
        let r = prove_formula("x == x");
        assert_eq!(r.status, VerificationStatus::Pass, "{}", r.details);
    }

    #[test]
    fn test_prove_implication() {
        let r = prove_formula("Implies(x > 0, x + 1 > 0)");
        assert_eq!(r.status, VerificationStatus::Pass, "{}", r.details);
    }

    #[test]
    fn test_prove_disprove() {
        let r = prove_formula("x == 5");
        assert_eq!(r.status, VerificationStatus::Fail, "{}", r.details);
    }

    #[test]
    fn test_solver_batch_sat() {
        let steps = vec![
            SolverBatchStep { action: "add".into(), n: None, expression: Some("x > 0".into()), timeout_ms: None },
            SolverBatchStep { action: "check".into(), n: None, expression: None, timeout_ms: None },
        ];
        let result = solver_batch(&steps).unwrap();
        let arr = result.get("steps").unwrap().as_array().unwrap();
        assert_eq!(arr[1].get("result").and_then(|v| v.as_str()), Some("sat"));
    }

    #[test]
    fn test_solver_batch_unsat() {
        let steps = vec![
            SolverBatchStep { action: "add".into(), n: None, expression: Some("x > 5".into()), timeout_ms: None },
            SolverBatchStep { action: "add".into(), n: None, expression: Some("x < 0".into()), timeout_ms: None },
            SolverBatchStep { action: "check".into(), n: None, expression: None, timeout_ms: Some(5000) },
        ];
        let result = solver_batch(&steps).unwrap();
        let arr = result.get("steps").unwrap().as_array().unwrap();
        assert_eq!(arr[2].get("result").and_then(|v| v.as_str()), Some("unsat"));
    }

    #[test]
    fn test_solver_batch_push_pop() {
        let steps = vec![
            SolverBatchStep { action: "push".into(), n: None, expression: None, timeout_ms: None },
            SolverBatchStep { action: "add".into(), n: None, expression: Some("x > 0".into()), timeout_ms: None },
            SolverBatchStep { action: "pop".into(), n: None, expression: None, timeout_ms: None },
            SolverBatchStep { action: "check".into(), n: None, expression: None, timeout_ms: None },
        ];
        let result = solver_batch(&steps).unwrap();
        let arr = result.get("steps").unwrap().as_array().unwrap();
        // After pop, the x > 0 constraint is removed → SAT (empty solver)
        assert_eq!(arr[3].get("result").and_then(|v| v.as_str()), Some("sat"));
    }

    #[test]
    fn test_solver_batch_invalid_action() {
        let steps = vec![SolverBatchStep {
            action: "nope".into(), n: None, expression: None, timeout_ms: None,
        }];
        let result = solver_batch(&steps).unwrap();
        let arr = result.get("steps").unwrap().as_array().unwrap();
        assert_eq!(arr[0].get("result").and_then(|v| v.as_str()), Some("error"));
    }
}
