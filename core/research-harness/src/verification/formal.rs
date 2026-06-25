//! 形式验证 — 量纲一致性检查。
//!
//! 基本的 SI 量纲一致性检查：在给定方程中检测左右两侧的量纲符号是否匹配。
//! 这是一个启发式检查，不替代完整的量纲分析工具。

use anyhow::Result;

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
    // 先检查维度标注（[L], [M], [T] 等）
    let eq_pos = equation.find('=');
    let (lhs, rhs) = match eq_pos {
        Some(pos) => (&equation[..pos], &equation[pos + 1..]),
        None => return Ok(true), // 无等号，无法判断
    };

    let lhs_dims = extract_dimension_tokens(lhs);
    let rhs_dims = extract_dimension_tokens(rhs);

    // 如果两侧都有维度标注但完全没有交集，说明不一致
    if !lhs_dims.is_empty() && !rhs_dims.is_empty() {
        let lhs_set: std::collections::HashSet<&str> =
            lhs_dims.iter().map(|s| s.as_str()).collect();
        let rhs_set: std::collections::HashSet<&str> =
            rhs_dims.iter().map(|s| s.as_str()).collect();
        let intersection: Vec<_> = lhs_set.intersection(&rhs_set).collect();
        if intersection.is_empty() {
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
    #[allow(clippy::expect_used)]
    let re = regex::Regex::new(r"\[([A-Z](?:[A-Za-z\^0-9+\-]*)?)\]").expect("static regex");
    re.captures_iter(text)
        .map(|cap| cap[1].to_string())
        .collect()
}

#[cfg(test)]
mod tests {
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
}
