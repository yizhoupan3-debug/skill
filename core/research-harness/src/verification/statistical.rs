//! 统计验证 — GRIM 检验与 p 值校验。
//!
//! GRIM (Granularity-Related Inconsistency of Means) test: 检验报告的均值
//! 在给定样本量下是否可以从整数响应中重建。
//! 参考：Brown & Heathers (2017), "The GRIM Test"

use anyhow::Result;

/// GRIM 检验：检验报告的均值在给定样本量下是否合理。
///
/// 原理：如果数据是 N 个整数，均值的分母必须能整除 N × mean。
/// 具体检查：round(N × mean) / N 是否约等于报告的 mean（到小数位精度）。
///
/// `decimals`：报告均值的小数位数（如报告 "3.50" 则 decimals=2）。
/// 返回 `true` 表示通过（合理），`false` 表示存在可疑。
pub fn grim_test(observed_mean: f64, n: usize, decimals: usize) -> Result<bool> {
    if n == 0 {
        return Err(anyhow::anyhow!("sample size n must be > 0"));
    }
    if observed_mean < 0.0 {
        return Err(anyhow::anyhow!("observed_mean must be >= 0"));
    }

    // 重建均值：round(N × mean) / N
    let sum = n as f64 * observed_mean;
    let rounded_sum = sum.round();
    let reconstructed = rounded_sum / n as f64;

    // 比较到报告的小数位精度
    let tolerance = 10f64.powi(-(decimals as i32)) * 0.5;
    Ok((observed_mean - reconstructed).abs() <= tolerance)
}

/// GRIM 检验（显式指定小数位数版本）。
///
/// 与 `grim_test` 等价，提供一致的参数命名。调用方必须显式传入报告值的
/// 小数位数（如报告 "3.50" 则 decimals=2），避免依赖 f64 Display 而丢失尾零。
pub fn grim_test_auto(observed_mean: f64, n: usize, decimals: usize) -> Result<bool> {
    grim_test(observed_mean, n, decimals)
}

/// 验证观测 p 值是否在预期值的容差范围内。
/// 返回 `true` 表示通过。
pub fn verify_p_value(observed: f64, expected: f64, tolerance: f64) -> bool {
    if !(0.0..=1.0).contains(&expected) {
        return false;
    }
    if !(0.0..=1.0).contains(&observed) {
        return false;
    }
    (observed - expected).abs() <= tolerance
}

/// 多重比较校正检查：如果进行了 k >= 3 次检验，是否应用了校正。
pub fn check_multiple_comparison_correction(
    num_tests: usize,
    correction_applied: bool,
) -> bool {
    if num_tests >= 3 && !correction_applied {
        return false; // 应该校正但没有
    }
    true
}

/// 效应量检查：是否报告了效应量。
pub fn check_effect_size_reported(
    effect_size: Option<f64>,
    test_type: &str,
) -> bool {
    match test_type {
        "t-test" | "anova" | "regression" | "chi-square" => effect_size.is_some(),
        _ => true, // 其他检验类型不强制
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grim_pass_integer_responses() {
        // N=20, mean=3.5 → sum=70, 70/20=3.5 ✓
        assert!(grim_test(3.50, 20, 2).unwrap());
    }

    #[test]
    fn test_grim_fail_suspicious_mean() {
        // N=20, mean=3.47 → sum=69.4, round=69, 69/20=3.45 ≠ 3.47
        assert!(!grim_test(3.47, 20, 2).unwrap());
    }

    #[test]
    fn test_grim_edge_case_zero() {
        assert!(grim_test(0.0, 10, 1).unwrap());
    }

    #[test]
    fn test_p_value_within_tolerance() {
        assert!(verify_p_value(0.048, 0.05, 0.01));
    }

    #[test]
    fn test_p_value_outside_tolerance() {
        assert!(!verify_p_value(0.03, 0.05, 0.01));
    }

    #[test]
    fn test_multiple_comparison_no_correction() {
        assert!(!check_multiple_comparison_correction(5, false));
        assert!(check_multiple_comparison_correction(2, false));
        assert!(check_multiple_comparison_correction(5, true));
    }

    #[test]
    fn test_effect_size_required_for_ttest() {
        assert!(!check_effect_size_reported(None, "t-test"));
        assert!(check_effect_size_reported(Some(0.5), "t-test"));
    }
}
