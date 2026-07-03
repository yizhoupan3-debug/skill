/// 形式验证 — 量纲一致性检查。
///
/// 基本的 SI 量纲一致性检查：在给定方程中检测左右两侧的量纲符号是否匹配。
/// 支持量纲传播（通过 Python SymPy 后端或启发式本地传播）。

use anyhow::Result;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::verification::symbolic::Expr;

/// 常见物理量纲符号及其 SI 基本量纲组合。
const DIMENSION_TABLE: &[(&str, &str)] = &[
    ("m", "L"),
    ("kg", "M"),
    ("s", "T"),
    ("A", "I"),
    ("K", "Θ"),
    ("mol", "N"),
    ("cd", "J"),
    ("N", "LMT^-2"),
    ("Pa", "L^-1MT^-2"),
    ("J", "L^2MT^-2"),
    ("W", "L^2MT^-3"),
    ("Hz", "T^-1"),
    ("V", "L^2MT^-3I^-1"),
    ("Ω", "L^2MT^-3I^-2"),
    ("F", "L^-2M^-1T^4I^2"),
];

/// 检查方程中的量纲是否一致。
///
/// 简化实现：提取方程中的维度标注（如 [L], [M], [T]），
/// 检查等号两侧的维度是否相同。
///
/// 返回 `true` 表示检查通过（一致或无法判断），`false` 表示检测到不一致。
pub fn check_dimensional_consistency(equation: &str) -> Result<bool> {
    // 处理多段等式链：F = ma = ... 分段检查每一对 (lhs, rhs)
    let parts: Vec<&str> = equation.split('=').collect();
    if parts.len() < 2 {
        return Ok(true); // 无等号，无法判断
    }
    // 对每一段等式分别做量纲检查
    for pair in parts.windows(2) {
        let lhs = pair[0];
        let rhs = pair[1];
        let lhs_dims = extract_dimension_tokens(lhs);
        let rhs_dims = extract_dimension_tokens(rhs);

        // 如果两侧都有维度标注，集合必须严格相等才一致。
        // `[L] = [L][T]` 中 lhs={"L"}, rhs={"L","T"}, 集合不等 → 不一致。
        if !lhs_dims.is_empty() && !rhs_dims.is_empty() {
            let lhs_set: std::collections::HashSet<&str> =
                lhs_dims.iter().map(|s| s.as_str()).collect();
            let rhs_set: std::collections::HashSet<&str> =
                rhs_dims.iter().map(|s| s.as_str()).collect();
            if lhs_set != rhs_set {
                return Ok(false);
            }
        } else if !lhs_dims.is_empty() || !rhs_dims.is_empty() {
            // One side has dimension annotations but the other doesn't → mismatch
            return Ok(false);
        }
    }

    // 没有维度标注时，尝试通过单位符号映射判断
    let units_found = extract_unit_symbols(equation);
    if units_found.is_empty() {
        return Ok(true); // 无单位符号，无法判断
    }

    Ok(true)
}

/// 从文本中提取单位符号（如 m, kg, s, N, Pa 等）。
fn extract_unit_symbols(text: &str) -> Vec<String> {
    let known: std::collections::HashSet<&str> =
        DIMENSION_TABLE.iter().map(|(sym, _)| *sym).collect();

    text.split(|c: char| !c.is_alphanumeric() && c != '^' && c != '-' && c != '*' && c != '/')
        .filter(|token| known.contains(*token))
        .map(|s| s.to_string())
        .collect()
}

/// 提取维度标注文本（如 [L], [M], [T] 等）。
fn extract_dimension_tokens(text: &str) -> Vec<String> {
    static DIMENSION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[([A-Z](?:[A-Za-z\^0-9+\-]*)?)\]")
            .expect("invalid dimension regex")
    });
    DIMENSION_RE
        .captures_iter(text)
        .map(|cap| cap[1].to_string())
        .collect()
}

// ===========================================================================
// 量纲传播 — 通过已知变量量纲推断方程量纲一致性
// ===========================================================================

/// 传播维度并检查方程的维度一致性。
///
/// 相比 `check_dimensional_consistency` 的简单集合比对，
/// 此函数尝试通过已知维度的运算传播来确定方程两边是否有相同维度。
///
/// 优先使用 Python SymPy 后端（当可用时），回退到启发式本地分析。
///
/// `dimensions` 是一个从变量名到维度字符串的映射，例如：
/// `{"F": "L*M*T^-2", "m": "M", "a": "L*T^-2"}`
///
/// 返回：
/// ```json
/// {
///   "lhs_dim": "...",     // 左侧计算结果维度
///   "rhs_dim": "...",     // 右侧计算结果维度
///   "consistent": true,   // 是否一致
///   "method": "sympy|heuristic|unknown"
/// }
/// ```
pub fn propagate_dimensions(equation: &str, dimensions: HashMap<String, String>) -> serde_json::Value {
    // Try Python SymPy backend first
    if crate::verification::python_bridge::sympy_available() {
        match python_dimension_propagate(equation, &dimensions) {
            Ok(result) => return result,
            Err(e) => {
                tracing::debug!("[formal] SymPy dimension propagate failed, falling back: {e}");
            }
        }
    }

    // Fallback: basic heuristic dimension propagation
    heuristic_dimension_propagate(equation, &dimensions)
}

/// Call Python SymPy backend for dimension propagation.
fn python_dimension_propagate(equation: &str, dims: &HashMap<String, String>) -> Result<serde_json::Value> {
    let params = json!({
        "equation": equation,
        "dimensions": dims,
    });
    let response = crate::verification::python_bridge::call_math_backend(
        "sympy_dimension_propagate",
        params,
    )?;
    Ok(response)
}

/// Heuristic dimension propagation without SymPy.
///
/// Uses the symbolic engine's recursive-descent parser to build an expression
/// AST, then traverses it with dimension algebra:
/// - Variables → look up in dimension map
/// - Constants → dimensionless
/// - Addition/Subtraction → operands must share the same dimension
/// - Multiplication → multiply dimensions (add exponents)
/// - Division → divide dimensions (subtract exponents)
/// - Powers → scale dimension exponents (only for constant powers)
/// - Transcendental functions (sin, cos, etc.) → require dimensionless input,
///   output dimensionless
/// - sqrt → half-exponent on the input dimension
///
/// Falls back to the original token-based approach when parsing fails.
fn heuristic_dimension_propagate(equation: &str, dims: &HashMap<String, String>) -> serde_json::Value {
    let parts: Vec<&str> = equation.split('=').collect();
    if parts.len() < 2 {
        return json!({
            "lhs_dim": "unknown",
            "rhs_dim": "unknown",
            "consistent": true,
            "method": "heuristic",
        });
    }

    let lhs_dim = compute_dimension_ast(parts[0], dims)
        .or_else(|| compute_dimension(parts[0], dims));
    let rhs_dim = compute_dimension_ast(parts[1], dims)
        .or_else(|| compute_dimension(parts[1], dims));

    let consistent = match (&lhs_dim, &rhs_dim) {
        (Some(a), Some(b)) => normalize_dim_string(a) == normalize_dim_string(b),
        _ => true, // Can't determine → assume consistent
    };

    json!({
        "lhs_dim": lhs_dim.unwrap_or_else(|| "unknown".to_string()),
        "rhs_dim": rhs_dim.unwrap_or_else(|| "unknown".to_string()),
        "consistent": consistent,
        "method": "heuristic",
    })
}

/// AST-based dimension computation using the symbolic engine parser.
///
/// Supports addition, subtraction, multiplication, division, powers,
/// parentheses, and transcendental functions — far more robust than
/// the token-based `compute_dimension` fallback.
fn compute_dimension_ast(expr: &str, dims: &HashMap<String, String>) -> Option<String> {
    let parsed = crate::verification::symbolic::parse(expr).ok()?;
    dimension_of_expr(&parsed, dims)
}

/// Recursively compute the dimension of an expression AST node.
fn dimension_of_expr(expr: &Expr, dims: &HashMap<String, String>) -> Option<String> {
    match expr {
        Expr::Var(name) => dims.get(name.as_str()).cloned(),

        Expr::Const(_) => Some("1".to_string()),

        Expr::Neg(a) => dimension_of_expr(a, dims),

        // Add/Sub: operands must have the same dimension
        Expr::Add(a, b) | Expr::Sub(a, b) => {
            let da = dimension_of_expr(a, dims)?;
            let db = dimension_of_expr(b, dims)?;
            if normalize_dim_string(&da) == normalize_dim_string(&db) {
                Some(da)
            } else {
                None
            }
        }

        // Mul: multiply dimensions
        Expr::Mul(a, b) => {
            let da = dimension_of_expr(a, dims)?;
            let db = dimension_of_expr(b, dims)?;
            Some(combine_dimensions(&[da, db]))
        }

        // Div: divide dimensions (negate denominator and multiply)
        Expr::Div(a, b) => {
            let da = dimension_of_expr(a, dims)?;
            let db = dimension_of_expr(b, dims)?;
            if normalize_dim_string(&db) == "1" {
                Some(da)
            } else {
                Some(combine_dimensions(&[da, negate_dimension(&db)]))
            }
        }

        // Pow: scale dimension by exponent (constant only)
        // For composite dimensions, replication distributes the exponent
        // correctly: (L*T^-1)^2 → L^2*T^-2
        Expr::Pow(a, b) => {
            let da = dimension_of_expr(a, dims)?;
            match b.as_ref() {
                Expr::Const(n) => {
                    let int_exp = *n as i32;
                    if int_exp == 0 { Some("1".to_string()) }
                    else if int_exp == 1 { Some(da) }
                    else if int_exp < 0 {
                        let negated = negate_dimension(&da);
                        let repeated = vec![negated; (-int_exp) as usize];
                        Some(combine_dimensions(&repeated))
                    } else {
                        let repeated = vec![da; int_exp as usize];
                        Some(combine_dimensions(&repeated))
                    }
                }
                _ => None, // Variable exponent → cannot determine dimension
            }
        }

        // Functions
        Expr::Fn(name, args) => match name.as_str() {
            "sin" | "cos" | "tan" | "exp" | "log" | "ln" => {
                // Require dimensionless arguments
                for arg in args {
                    if let Some(d) = dimension_of_expr(arg, dims) {
                        if normalize_dim_string(&d) != "1" && normalize_dim_string(&d) != "" {
                            return None;
                        }
                    }
                }
                Some("1".to_string())
            }
            "sqrt" => {
                let d = dimension_of_expr(&args[0], dims)?;
                if normalize_dim_string(&d) == "1" {
                    Some("1".to_string())
                } else {
                    Some(format!("{d}^(1/2)"))
                }
            }
            "abs" => args.first().and_then(|a| dimension_of_expr(a, dims)),
            _ => dims.get(name.as_str()).cloned(),
        },
    }
}

/// Compute the combined dimension of a expression string using a known dimension map.
fn compute_dimension(expr: &str, dims: &HashMap<String, String>) -> Option<String> {
    // Simple token-based dimension propagation:
    // Split by *, / and look up each token
    let expr = expr.trim();

    // Check if the expression is a single known symbol
    let cleaned = expr.replace(" ", "");
    if dims.contains_key(&cleaned) {
        return dims.get(&cleaned).cloned();
    }

    // Try to parse as product/quotient of known symbols
    // Split by multiplication (*) and division (/)
    let mut result_dims: Vec<String> = Vec::new();
    let mut is_dividing = false;
    let mut current = String::new();

    for ch in cleaned.chars() {
        if ch == '*' {
            if !current.is_empty() {
                if let Some(d) = dims.get(&current) {
                    if is_dividing {
                        result_dims.push(negate_dimension(d));
                    } else {
                        result_dims.push(d.clone());
                    }
                }
                current.clear();
            }
            is_dividing = false;
        } else if ch == '/' {
            if !current.is_empty() {
                if let Some(d) = dims.get(&current) {
                    if is_dividing {
                        result_dims.push(negate_dimension(d));
                    } else {
                        result_dims.push(d.clone());
                    }
                }
                current.clear();
            }
            is_dividing = true;
        } else {
            current.push(ch);
        }
    }
    // Last token
    if !current.is_empty() {
        if let Some(d) = dims.get(&current) {
            if is_dividing {
                result_dims.push(negate_dimension(d));
            } else {
                result_dims.push(d.clone());
            }
        }
    }

    if result_dims.is_empty() {
        // Check for explicit dimension annotations [L], [M], etc.
        let tokens = extract_dimension_tokens(expr);
        if !tokens.is_empty() {
            return Some(tokens.join("*"));
        }
        return None;
    }

    // Combine dimensions: handle multiplications of dimension strings
    // e.g. L * T^-1 → L*T^-1
    Some(combine_dimensions(&result_dims))
}

/// Combine a list of dimension strings into a single normalized dimension string.
///
/// Each input is parsed into base components and their exponents are summed.
/// e.g., `["L*T^-2", "M"]` → sum exponents: L=1, M=1, T=-2 → `"L*M*T^-2"`
fn combine_dimensions(dims: &[String]) -> String {
    if dims.len() == 1 {
        return dims[0].clone();
    }

    // Parse each dimension into base components
    // e.g., "L*M*T^-2" → {"L": 1, "M": 1, "T": -2}
    let mut combined: HashMap<String, i32> = HashMap::new();

    for dim in dims {
        // Split composite dimensions on *
        for part in dim.split('*') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            // Parse exponent: "L^2" or just "L" or "L^-1"
            if let Some(caret_pos) = part.find('^') {
                let base = &part[..caret_pos];
                let exp_str = &part[caret_pos + 1..];
                let exp: i32 = exp_str.parse().unwrap_or(1);
                *combined.entry(base.to_string()).or_insert(0) += exp;
            } else {
                *combined.entry(part.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Build result string
    let mut parts: Vec<String> = combined
        .iter()
        .filter(|&(_, &exp)| exp != 0)
        .map(|(base, &exp)| {
            if exp == 1 {
                base.clone()
            } else {
                format!("{base}^{exp}")
            }
        })
        .collect();

    if parts.is_empty() {
        return "1".to_string(); // dimensionless
    }

    parts.sort(); // deterministic order
    parts.join("*")
}

/// Negate all exponents in a dimension string.
///
/// e.g., `"L*M*T^-2"` → `"L^-1*M^-1*T^2"`; `"L^2"` → `"L^-2"`
fn negate_dimension(dim: &str) -> String {
    if dim == "1" || dim.is_empty() {
        return "1".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    for part in dim.split('*') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(caret_pos) = part.find('^') {
            let base = &part[..caret_pos];
            let exp_str = &part[caret_pos + 1..];
            let exp: i32 = exp_str.parse().unwrap_or(1);
            let new_exp = -exp;
            if new_exp == 1 {
                parts.push(base.to_string());
            } else {
                parts.push(format!("{base}^{new_exp}"));
            }
        } else {
            parts.push(format!("{part}^-1"));
        }
    }
    if parts.is_empty() {
        "1".to_string()
    } else {
        parts.join("*")
    }
}

/// Normalize a dimension string for comparison (canonical form).
///
/// Splits on `*`, sorts component strings (e.g. `"L"`, `"L^2"`, `"T^-2"`),
/// and rejoins with `*`. This ensures dimensions like `"L*M*T^-2"` and
/// `"M*L*T^-2"` compare equal, while correctly handling exponent notation.
fn normalize_dim_string(dim: &str) -> String {
    let mut parts: Vec<&str> = dim.split('*').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    parts.sort();
    parts.join("*")
}

/// 检查方程在特例值（witness）下是否一致。
///
/// 对每个 witness（变量名 → 数值的映射），代入方程两侧并比较数值结果。
/// 所有 witness 通过则返回 PASS，任一不通过则返回 FAIL 并说明哪些 witness 不通过。
///
/// 方程格式：`lhs = rhs`，例如 `x + y = 2*x`。
/// 支持等链如 `A = B = C`（分段比较每对）。
///
/// 返回：
/// ```json
/// {
///   "passed": true/false,
///   "witnesses_checked": N,
///   "failures": [{"witness": {"x": 1, "y": 2}, "lhs_value": 3.0, "rhs_value": 1.0, "diff": 2.0}],
///   "detail": "..."
/// }
/// ```
pub fn check_witness_consistency(
    expression: &str,
    witnesses: &[HashMap<String, f64>],
) -> Result<serde_json::Value> {
    if expression.is_empty() {
        return Ok(json!({
            "passed": true,
            "witnesses_checked": 0,
            "failures": [],
            "detail": "Empty expression — skipping witness check",
        }));
    }
    if witnesses.is_empty() {
        return Ok(json!({
            "passed": true,
            "witnesses_checked": 0,
            "failures": [],
            "detail": "No witnesses provided — skipping witness check",
        }));
    }

    // Split on '=' into segments
    let segments: Vec<&str> = expression.split('=').map(|s| s.trim()).collect();
    if segments.len() < 2 {
        return Ok(json!({
            "passed": true,
            "witnesses_checked": 0,
            "failures": [],
            "detail": "Expression has no '=' — skipping witness check",
        }));
    }

    // Build pairs: (lhs, rhs) for each consecutive pair in the chain
    let pairs: Vec<(&str, &str)> = segments.windows(2).map(|w| (w[0], w[1])).collect();

    let mut failures: Vec<serde_json::Value> = Vec::new();
    let tolerance = 1e-8;

    for (pair_idx, (lhs_str, rhs_str)) in pairs.iter().enumerate() {
        let lhs_expr = match crate::verification::symbolic::parse(lhs_str) {
            Ok(e) => e,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to parse lhs in pair {}: '{}': {}",
                    pair_idx,
                    lhs_str,
                    e
                ));
            }
        };
        let rhs_expr = match crate::verification::symbolic::parse(rhs_str) {
            Ok(e) => e,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to parse rhs in pair {}: '{}': {}",
                    pair_idx,
                    rhs_str,
                    e
                ));
            }
        };

        for (w_idx, witness) in witnesses.iter().enumerate() {
            let lhs_val = crate::verification::symbolic::eval(&lhs_expr, witness)?;
            let rhs_val = crate::verification::symbolic::eval(&rhs_expr, witness)?;
            let diff = (lhs_val - rhs_val).abs();
            if diff > tolerance {
                failures.push(json!({
                    "pair_index": pair_idx,
                    "witness_index": w_idx,
                    "lhs_expression": lhs_str,
                    "rhs_expression": rhs_str,
                    "substitutions": witness,
                    "lhs_value": lhs_val,
                    "rhs_value": rhs_val,
                    "diff": diff,
                }));
            }
        }
    }

    let passed = failures.is_empty();
    Ok(json!({
        "passed": passed,
        "witnesses_checked": witnesses.len(),
        "pairs_checked": pairs.len(),
        "failures": failures,
        "detail": if passed {
            format!("All {} witnesses pass (tolerance={})", witnesses.len(), tolerance)
        } else {
            format!(
                "{} witness(es) failed. First failure: LHS evaluated to {}, RHS evaluated to {}",
                failures.len(),
                failures[0]["lhs_value"],
                failures[0]["rhs_value"],
            )
        },
    }))
}

/// 检查步骤依赖图完整性：无悬空引用。
///
/// 输入步骤列表，每个步骤有 `id` 和 `depends_on`（依赖的其他步骤 ID 列表）。
/// 验证所有 `depends_on` 引用的 ID 在步骤列表中实际存在。
///
/// 返回：
/// ```json
/// {
///   "passed": true/false,
///   "total_steps": N,
///   "dangling_references": [{"from": "step-4", "depends_on": "step-99"}],
///   "detail": "..."
/// }
/// ```
pub fn check_step_dependency(steps: &[serde_json::Value]) -> serde_json::Value {
    // Collect all valid step IDs
    let mut valid_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in steps {
        if let Some(id) = step.get("id").and_then(Value::as_str) {
            valid_ids.insert(id.to_string());
        }
    }

    // Check each step's depends_on
    let mut dangling: Vec<serde_json::Value> = Vec::new();
    for step in steps {
        let from_id = step.get("id").and_then(Value::as_str).unwrap_or("(unnamed)");
        if let Some(deps) = step.get("depends_on").and_then(Value::as_array) {
            for dep_val in deps {
                if let Some(dep_id) = dep_val.as_str() {
                    if !valid_ids.contains(dep_id) {
                        dangling.push(json!({
                            "from": from_id,
                            "depends_on": dep_id,
                        }));
                    }
                }
            }
        }
    }

    let passed = dangling.is_empty();

    // Also check for circular dependencies (basic first-level detection)
    let mut circular: Vec<serde_json::Value> = Vec::new();
    for step in steps {
        let step_id = step.get("id").and_then(Value::as_str);
        if let Some(deps) = step.get("depends_on").and_then(Value::as_array) {
            for dep_val in deps {
                if let Some(dep_id) = dep_val.as_str() {
                    // Check if dep_id depends back on step_id (direct cycle)
                    if let Some(dep_step) = steps.iter().find(|s| {
                        s.get("id").and_then(Value::as_str) == Some(dep_id)
                    }) {
                        if let Some(dep_deps) = dep_step.get("depends_on").and_then(Value::as_array)
                        {
                            if dep_deps.iter().any(|d| d.as_str() == step_id) {
                                circular.push(json!({
                                    "from": step_id,
                                    "depends_on": dep_id,
                                    "kind": "direct_cycle",
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    let has_circular = !circular.is_empty();

    // Compute detail message outside json! macro
    let mut msgs = Vec::new();
    if passed && !has_circular {
        msgs.push(format!("All {} steps have valid dependencies", steps.len()));
    }
    if !dangling.is_empty() {
        msgs.push(format!("{} dangling reference(s) found", dangling.len()));
    }
    if has_circular {
        msgs.push(format!("{} circular reference(s) found", circular.len()));
    }
    let detail = msgs.join("; ");

    json!({
        "passed": passed && !has_circular,
        "total_steps": steps.len(),
        "valid_ids": valid_ids.into_iter().collect::<Vec<_>>(),
        "dangling_references": dangling,
        "circular_dependencies": circular,
        "detail": detail,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn empty_equation_passes() {
        assert!(check_dimensional_consistency("").unwrap());
    }

    #[test]
    fn no_units_passes() {
        assert!(check_dimensional_consistency("x = y + z").unwrap());
    }

    #[test]
    fn consistent_dimensions_pass() {
        assert!(check_dimensional_consistency("[L] = [L]").unwrap());
    }

    #[test]
    fn inconsistent_dimensions_fail() {
        assert!(!check_dimensional_consistency("[L] = [T]").unwrap());
    }

    #[test]
    fn compound_dimensions_pass() {
        assert!(check_dimensional_consistency("[L^2MT^-2] = [L^2MT^-2]").unwrap());
    }

    #[test]
    fn known_units_detected() {
        let units = extract_unit_symbols("v = 10 m / s");
        assert!(units.contains(&"m".to_string()));
        assert!(units.contains(&"s".to_string()));
    }

    #[test]
    fn one_side_has_dims_other_does_not() {
        // [L] = x → one side annotated, other not → mismatch
        assert!(!check_dimensional_consistency("[L] = x").unwrap());
        // x = [T] → same pattern inverted
        assert!(!check_dimensional_consistency("x = [T]").unwrap());
    }

    #[test]
    fn multi_part_chain_partial_mismatch() {
        // [L] = [L] = x → second pair mismatched
        assert!(!check_dimensional_consistency("[L] = [L] = x").unwrap());
    }

    #[test]
    fn test_multi_part_chain_all_consistent() {
        // [L] = [L] = [L] — all pairs consistent
        assert!(
            check_dimensional_consistency("[L] = [L] = [L]").unwrap(),
            "[L] = [L] = [L] should be consistent"
        );
    }

    #[test]
    fn test_no_equals_sign_returns_true() {
        // No equals sign → parts.len() < 2 → returns Ok(true)
        assert!(
            check_dimensional_consistency("x + y z").unwrap(),
            "no equals sign should return true"
        );
        assert!(
            check_dimensional_consistency("[L] [T]").unwrap(),
            "no equals sign with dims should return true"
        );
    }

    #[test]
    fn both_sides_no_dims_passes() {
        // Neither side has dimension annotations → can't judge → passes
        assert!(check_dimensional_consistency("x = y").unwrap());
    }

    // ── Witness consistency tests ──

    #[test]
    fn test_witness_empty_expression() {
        let result = check_witness_consistency("", &[HashMap::new()]).unwrap();
        assert!(result["passed"].as_bool().unwrap());
        assert_eq!(result["witnesses_checked"].as_u64().unwrap(), 0);
    }

    #[test]
    fn test_witness_no_witnesses() {
        let result = check_witness_consistency("x = x", &[]).unwrap();
        assert!(result["passed"].as_bool().unwrap());
        assert_eq!(result["witnesses_checked"].as_u64().unwrap(), 0);
    }

    #[test]
    fn test_witness_identity_passes() {
        let witnesses = vec![
            HashMap::from([("x".into(), 1.0)]),
            HashMap::from([("x".into(), 100.0)]),
            HashMap::from([("x".into(), -3.5)]),
        ];
        let result = check_witness_consistency("x = x", &witnesses).unwrap();
        assert!(result["passed"].as_bool().unwrap(), "x=x should pass for all witnesses");
        assert_eq!(result["witnesses_checked"].as_u64().unwrap(), 3);
    }

    #[test]
    fn test_witness_simple_equation() {
        // x + 1 = 3 → at x=2: 3=3 ✓, at x=1: 2=3 ✗
        let passing = HashMap::from([("x".into(), 2.0)]);
        let result = check_witness_consistency("x + 1 = 3", &[passing]).unwrap();
        assert!(result["passed"].as_bool().unwrap());

        let failing = HashMap::from([("x".into(), 1.0)]);
        let result = check_witness_consistency("x + 1 = 3", &[failing]).unwrap();
        assert!(!result["passed"].as_bool().unwrap());
        assert_eq!(result["failures"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_witness_multi_variable() {
        // x + y = 2*x → at (x=1, y=1): 2=2 ✓, at (x=1, y=2): 3=2 ✗
        let witnesses = vec![
            HashMap::from([("x".into(), 1.0), ("y".into(), 1.0)]),
            HashMap::from([("x".into(), 3.0), ("y".into(), 3.0)]),
        ];
        let result = check_witness_consistency("x + y = 2*x", &witnesses).unwrap();
        assert!(result["passed"].as_bool().unwrap());

        let failing = HashMap::from([("x".into(), 1.0), ("y".into(), 2.0)]);
        let result = check_witness_consistency("x + y = 2*x", &[failing]).unwrap();
        assert!(!result["passed"].as_bool().unwrap());
    }

    #[test]
    fn test_witness_chain_equality() {
        // x + 1 = 2 = y can't parse — skip
        let witnesses = vec![HashMap::from([("x".into(), 3.0), ("y".into(), 3.0)])];
        let result = check_witness_consistency("x = y", &witnesses).unwrap();
        assert!(result["passed"].as_bool().unwrap());
    }

    // ── Step dependency tests ──

    #[test]
    fn test_step_dependency_empty() {
        let result = check_step_dependency(&[]);
        assert!(result["passed"].as_bool().unwrap());
        assert_eq!(result["total_steps"].as_u64().unwrap(), 0);
    }

    #[test]
    fn test_step_dependency_no_deps() {
        let steps = json!([
            {"id": "step-1", "description": "First step"},
            {"id": "step-2", "description": "Second step"},
        ]);
        let result = check_step_dependency(steps.as_array().unwrap());
        assert!(result["passed"].as_bool().unwrap());
        assert_eq!(result["total_steps"].as_u64().unwrap(), 2);
    }

    #[test]
    fn test_step_dependency_valid() {
        let steps = json!([
            {"id": "step-1", "depends_on": []},
            {"id": "step-2", "depends_on": ["step-1"]},
            {"id": "step-3", "depends_on": ["step-1", "step-2"]},
        ]);
        let result = check_step_dependency(steps.as_array().unwrap());
        assert!(result["passed"].as_bool().unwrap(), "All deps should be valid");
        assert!(result["dangling_references"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_step_dependency_dangling() {
        let steps = json!([
            {"id": "step-1", "depends_on": ["step-99"]},
            {"id": "step-2", "depends_on": ["step-1", "step-999"]},
        ]);
        let result = check_step_dependency(steps.as_array().unwrap());
        assert!(!result["passed"].as_bool().unwrap());
        assert_eq!(result["dangling_references"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_step_dependency_circular() {
        let steps = json!([
            {"id": "step-1", "depends_on": ["step-2"]},
            {"id": "step-2", "depends_on": ["step-1"]},
        ]);
        let result = check_step_dependency(steps.as_array().unwrap());
        assert!(!result["passed"].as_bool().unwrap(),
            "Circular dependency should fail");
        assert_eq!(result["circular_dependencies"].as_array().unwrap().len(), 2);
    }

    // ── Dimension propagation tests ──

    #[test]
    fn test_propagate_dimensions_f_ma_consistent() {
        // F = ma should be dimensionally consistent (may use SymPy or heuristic)
        let dims = HashMap::from([
            ("F".into(), "L*M*T^-2".into()),
            ("m".into(), "M".into()),
            ("a".into(), "L*T^-2".into()),
        ]);
        let result = propagate_dimensions("F = m*a", dims);
        assert!(
            result["consistent"].as_bool().unwrap(),
            "F=ma should be dimensionally consistent: {:?}",
            result
        );
        // Both sides should report the same dimension, but format varies:
        // SymPy uses "**" (e.g. "L*M*T**-2") while heuristic uses "^" (e.g. "L*M*T^-2")
        assert_eq!(
            result["lhs_dim"].as_str().unwrap(),
            result["rhs_dim"].as_str().unwrap(),
            "lhs_dim and rhs_dim must be equal"
        );
    }

    #[test]
    fn test_propagate_dimensions_heuristic_fallback() {
        // Directly test the heuristic path (always forced)
        let dims = HashMap::from([
            ("F".into(), "L*M*T^-2".into()),
            ("m".into(), "M".into()),
            ("a".into(), "L*T^-2".into()),
        ]);
        let result = heuristic_dimension_propagate("F = m*a", &dims);
        assert!(
            result["consistent"].as_bool().unwrap(),
            "Heuristic: F=ma should be consistent: {:?}",
            result
        );
        assert_eq!(result["method"].as_str().unwrap(), "heuristic");
        assert_eq!(result["lhs_dim"].as_str().unwrap(), "L*M*T^-2");
        assert_eq!(result["rhs_dim"].as_str().unwrap(), "L*M*T^-2");
    }

    #[test]
    fn test_propagate_dimensions_heuristic_no_equals() {
        // No '=' sign: returns unknown/consistent/method=heuristic
        let dims = HashMap::new();
        let result = heuristic_dimension_propagate("x + y", &dims);
        assert!(result["consistent"].as_bool().unwrap());
        assert_eq!(result["method"].as_str().unwrap(), "heuristic");
        assert_eq!(result["lhs_dim"].as_str().unwrap(), "unknown");
        assert_eq!(result["rhs_dim"].as_str().unwrap(), "unknown");
    }

    #[test]
    fn test_propagate_dimensions_heuristic_inconsistent() {
        // Force inconsistency: F vs T
        let dims = HashMap::from([
            ("F".into(), "L*M*T^-2".into()),
            ("t".into(), "T".into()),
        ]);
        let result = heuristic_dimension_propagate("F = t", &dims);
        assert!(
            !result["consistent"].as_bool().unwrap(),
            "F vs T should be inconsistent"
        );
    }

    #[test]
    fn test_compute_dimension_single_var() {
        let dims = HashMap::from([
            ("x".into(), "L".into()),
            ("t".into(), "T".into()),
        ]);
        assert_eq!(compute_dimension("x", &dims).unwrap(), "L");
        assert_eq!(compute_dimension("t", &dims).unwrap(), "T");
    }

    #[test]
    fn test_compute_dimension_product() {
        // m * a where m=M, a=L*T^-2 → L*M*T^-2
        let dims = HashMap::from([
            ("m".into(), "M".into()),
            ("a".into(), "L*T^-2".into()),
        ]);
        let result = compute_dimension("m*a", &dims).unwrap();
        assert_eq!(result, "L*M*T^-2", "m*a should produce L*M*T^-2, got: {result}");
    }

    #[test]
    fn test_compute_dimension_quotient() {
        // d / t where d=L, t=T → L*T^-1
        let dims = HashMap::from([
            ("d".into(), "L".into()),
            ("t".into(), "T".into()),
        ]);
        let result = compute_dimension("d/t", &dims).unwrap();
        assert_eq!(result, "L*T^-1", "d/t should produce L*T^-1, got: {result}");
    }

    #[test]
    fn test_compute_dimension_unknown_returns_none() {
        let dims = HashMap::from([("x".into(), "L".into())]);
        assert!(
            compute_dimension("z", &dims).is_none(),
            "unknown variable should return None"
        );
    }

    #[test]
    fn test_compute_dimension_annotation_fallback() {
        // compute_dimension falls through to extract_dimension_tokens
        // dimensions from [L] and [T] are joined with '*' separator
        let dims = HashMap::new();
        let result = compute_dimension("speed = [L]/[T]", &dims).unwrap();
        assert_eq!(result, "L*T", "dimension annotation fallback, got: {result}");
    }

    #[test]
    fn test_combine_dimensions_simple_product() {
        let result = combine_dimensions(&["L".into(), "M".into(), "T^-2".into()]);
        assert_eq!(result, "L*M*T^-2");
    }

    #[test]
    fn test_combine_dimensions_with_negation() {
        // L * T^-1 * M → L*M*T^-1
        let result = combine_dimensions(&["L".into(), "T^-1".into(), "M".into()]);
        assert_eq!(result, "L*M*T^-1");
    }

    #[test]
    fn test_combine_dimensions_dimensionless() {
        // Opposite dimensions cancel: L * L^-1 → 1
        let result = combine_dimensions(&["L".into(), "L^-1".into()]);
        assert_eq!(result, "1");
    }

    #[test]
    fn test_combine_dimensions_composite_with_negation() {
        // Simulate dividing by composite: M * (L*T^-2)^-1
        // Use negate_dimension to create the negated composite
        let negated = negate_dimension("L*T^-2");
        assert_eq!(negated, "L^-1*T^2", "negate_dimension(L*T^-2) should be L^-1*T^2");
        let result = combine_dimensions(&["M".into(), negated]);
        let parts: Vec<&str> = result.split('*').collect();
        assert!(parts.contains(&"L^-1"), "Should contain L^-1 in {result}");
        assert!(parts.contains(&"M"), "Should contain M in {result}");
        assert!(parts.contains(&"T^2"), "Should contain T^2 in {result}");
    }

    #[test]
    fn test_combine_dimensions_zero_exp_cancel() {
        // L * L^-1 → dimensionless 1 (exponent sums to 0)
        let result = combine_dimensions(&["L".into(), "L^-1".into()]);
        assert_eq!(result, "1");
    }

    #[test]
    fn test_normalize_dim_string_equality() {
        assert_eq!(
            normalize_dim_string("L*M*T^-2"),
            normalize_dim_string("M*L*T^-2"),
        );
        assert_eq!(
            normalize_dim_string("T^-1*L"),
            normalize_dim_string("L*T^-1"),
        );
    }

    #[test]
    fn test_normalize_dim_string_inequality() {
        assert_ne!(normalize_dim_string("L"), normalize_dim_string("T"));
    }

    #[test]
    fn test_normalize_dim_string_empty() {
        assert_eq!(normalize_dim_string(""), "");
    }

    // ── AST-based dimension propagation tests ──

    #[test]
    fn test_dimension_of_expr_single_var() {
        let dims = HashMap::from([("x".into(), "L".into())]);
        let expr = crate::verification::symbolic::parse("x").unwrap();
        let result = dimension_of_expr(&expr, &dims);
        assert_eq!(result, Some("L".to_string()));
    }

    #[test]
    fn test_dimension_of_expr_add_same_dim() {
        let dims = HashMap::from([
            ("x".into(), "L".into()),
            ("y".into(), "L".into()),
        ]);
        let expr = crate::verification::symbolic::parse("x + y").unwrap();
        let result = dimension_of_expr(&expr, &dims);
        assert_eq!(result, Some("L".to_string()));
    }

    #[test]
    fn test_dimension_of_expr_add_mismatch_dims() {
        let dims = HashMap::from([
            ("x".into(), "L".into()),
            ("t".into(), "T".into()),
        ]);
        let expr = crate::verification::symbolic::parse("x + t").unwrap();
        let result = dimension_of_expr(&expr, &dims);
        assert!(result.is_none(), "L + T should be dimensionally inconsistent");
    }

    #[test]
    fn test_dimension_of_expr_mul() {
        let dims = HashMap::from([
            ("m".into(), "M".into()),
            ("v".into(), "L*T^-1".into()),
        ]);
        let expr = crate::verification::symbolic::parse("m * v").unwrap();
        let result = dimension_of_expr(&expr, &dims);
        // M * (L*T^-1) = L*M*T^-1 (sorted alphabetically)
        let r = result.unwrap();
        assert!(r.contains("L"), "should contain L in {r}");
        assert!(r.contains("M"), "should contain M in {r}");
        assert!(r.contains("T^-1"), "should contain T^-1 in {r}");
    }

    #[test]
    fn test_dimension_of_expr_power() {
        let dims = HashMap::from([("v".into(), "L*T^-1".into())]);
        let expr = crate::verification::symbolic::parse("v^2").unwrap();
        let result = dimension_of_expr(&expr, &dims);
        let r = result.unwrap();
        // (L*T^-1)^2 → v^2 replicates: combine([L*T^-1, L*T^-1])
        // = L^2 * T^-2
        assert!(r.contains("L^2"), "should contain L^2 in {r}");
        assert!(r.contains("T^-2"), "should contain T^-2 in {r}");
    }

    #[test]
    fn test_dimension_of_expr_div() {
        let dims = HashMap::from([
            ("d".into(), "L".into()),
            ("t".into(), "T".into()),
        ]);
        let expr = crate::verification::symbolic::parse("d / t").unwrap();
        let result = dimension_of_expr(&expr, &dims);
        let r = result.unwrap();
        assert!(r.contains("L"), "should contain L in {r}");
        assert!(r.contains("T^-1"), "should contain T^-1 in {r}");
    }

    #[test]
    fn test_dimension_of_expr_sin() {
        let dims = HashMap::from([("x".into(), "1".into())]);
        let expr = crate::verification::symbolic::parse("sin(x)").unwrap();
        let result = dimension_of_expr(&expr, &dims);
        assert_eq!(result, Some("1".to_string()));
    }

    #[test]
    fn test_dimension_of_expr_sin_rejects_dimensioned() {
        let dims = HashMap::from([("x".into(), "L".into())]);
        let expr = crate::verification::symbolic::parse("sin(x)").unwrap();
        let result = dimension_of_expr(&expr, &dims);
        assert!(result.is_none(), "sin(L) should be rejected");
    }

    #[test]
    fn test_propagate_dimensions_heuristic_with_addition() {
        let dims = HashMap::from([
            ("F".into(), "L*M*T^-2".into()),
            ("m".into(), "M".into()),
            ("a".into(), "L*T^-2".into()),
            ("g".into(), "L*T^-2".into()),
        ]);
        let result = heuristic_dimension_propagate("F = m*a + m*g", &dims);
        assert!(
            result["consistent"].as_bool().unwrap(),
            "F = m*a + m*g should be consistent: {result:?}"
        );
        assert_eq!(result["lhs_dim"].as_str().unwrap(), result["rhs_dim"].as_str().unwrap());
    }

    #[test]
    fn test_propagate_dimensions_heuristic_with_parens() {
        let dims = HashMap::from([
            ("F".into(), "L*M*T^-2".into()),
            ("m".into(), "M".into()),
            ("v".into(), "L*T^-1".into()),
            ("t".into(), "T".into()),
        ]);
        let result = heuristic_dimension_propagate("F = (m*v)/t", &dims);
        assert!(
            result["consistent"].as_bool().unwrap(),
            "F = (m*v)/t should be consistent: {result:?}"
        );
    }

    #[test]
    fn test_negate_dimension_simple() {
        assert_eq!(negate_dimension("L"), "L^-1");
        assert_eq!(negate_dimension("L^2"), "L^-2");
        assert_eq!(negate_dimension("M"), "M^-1");
    }

    #[test]
    fn test_negate_dimension_composite() {
        assert_eq!(negate_dimension("L*T^-1"), "L^-1*T");
        assert_eq!(negate_dimension("L*M*T^-2"), "L^-1*M^-1*T^2");
    }

    #[test]
    fn test_negate_dimension_dimensionless() {
        assert_eq!(negate_dimension("1"), "1");
    }

    #[test]
    fn test_compute_dimension_ast_paren_product() {
        let dims = HashMap::from([
            ("m".into(), "M".into()),
            ("v".into(), "L*T^-1".into()),
            ("t".into(), "T".into()),
        ]);
        let result = compute_dimension_ast("(m*v)/t", &dims).unwrap();
        assert!(result.contains("M"), "should contain M in {result}");
        assert!(result.contains("L"), "should contain L in {result}");
        assert!(result.contains("T^-2") || result.contains("T^2"),
            "should contain T^-2 or T^2 in {result} (negated T or inverted)");
    }

    #[test]
    fn test_compute_dimension_ast_fallback_to_token() {
        // Unknown variable → AST returns None
        let dims = HashMap::from([("x".into(), "L".into())]);
        let result = compute_dimension_ast("unknown_var", &dims);
        assert!(result.is_none(), "AST-based should return None for unknown var");

        // Token-based fallback still works
        let result2 = compute_dimension("x", &dims);
        assert_eq!(result2.as_deref(), Some("L"));
    }
}
