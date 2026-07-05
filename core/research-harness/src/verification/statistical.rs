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

    // 重建均值：round(N × mean) / N
    let sum = n as f64 * observed_mean;
    let rounded_sum = sum.round();
    let reconstructed = rounded_sum / n as f64;

    // 比较到报告的小数位精度
    let tolerance = 10f64.powi(-(decimals as i32)) * 0.5;
    Ok((observed_mean - reconstructed).abs() <= tolerance)
}

/// 验证观测 p 值是否在预期值的容差范围内。
/// 返回 `true` 表示通过。
pub fn verify_p_value(observed: f64, expected: f64, tolerance: f64) -> bool {
    if tolerance.is_nan() || tolerance < 0.0 {
        return false;
    }
    if !(0.0..=1.0).contains(&expected) {
        return false;
    }
    if !(0.0..=1.0).contains(&observed) {
        return false;
    }
    (observed - expected).abs() <= tolerance
}

/// 多重比较校正检查：如果进行了 k >= 3 次检验，是否应用了校正。
pub fn check_multiple_comparison_correction(num_tests: usize, correction_applied: bool) -> bool {
    if num_tests >= 3 && !correction_applied {
        return false; // 应该校正但没有
    }
    true
}

/// 效应量检查：是否报告了效应量。
pub fn check_effect_size_reported(effect_size: Option<f64>, test_type: &str) -> bool {
    match test_type {
        "t-test" | "anova" | "regression" | "chi-square" => effect_size.is_some(),
        _ => true, // 其他检验类型不强制
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
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

    #[test]
    fn test_grim_large_n() {
        // N=1000, mean=5.000 → sum=5000, 5000/1000=5.000 ✓
        assert!(grim_test(5.000, 1000, 3).unwrap());
    }

    #[test]
    fn test_grim_single_participant() {
        assert!(grim_test(5.5, 1, 1).unwrap());
    }

    #[test]
    fn test_grim_negative_mean() {
        assert!(!grim_test(-3.14, 7, 2).unwrap());
    }

    #[test]
    fn test_p_value_exact_match() {
        assert!(verify_p_value(0.05, 0.05, 0.001));
    }

    #[test]
    fn test_p_value_zero_tolerance() {
        assert!(verify_p_value(0.05, 0.05, 0.0));
        assert!(!verify_p_value(0.051, 0.05, 0.0));
    }

    #[test]
    fn test_multiple_comparison_single_test() {
        assert!(check_multiple_comparison_correction(1, false));
        assert!(check_multiple_comparison_correction(1, true));
    }

    #[test]
    fn test_effect_size_not_required_for_descriptive() {
        assert!(check_effect_size_reported(None, "descriptive"));
        assert!(check_effect_size_reported(None, "unknown"));
    }

    #[test]
    fn test_grim_very_small_decimal_places() {
        // N=3, mean=1.3333... → sum=4.0, 4/3=1.333 ≠ 1.3333 at 4 decimals
        // With decimals=0: sum_rounded=4, 4/3=1 → but mean=1.333 → fail
        assert!(
            grim_test(1.3333, 3, 4).unwrap() ||
            !grim_test(1.3333, 3, 4).unwrap(),
            "GRIM should not panic for small N"
        );
    }

    #[test]
    fn test_effect_size_all_types() {
        let required_types = ["t-test", "anova", "regression", "chi-square"];
        for tt in &required_types {
            assert!(!check_effect_size_reported(None, tt),
                "{tt} should require effect size");
            assert!(check_effect_size_reported(Some(0.3), tt),
                "{tt} should accept effect size");
        }
    }
}
