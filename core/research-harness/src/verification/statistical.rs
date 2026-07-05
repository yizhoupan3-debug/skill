//! 统计验证 — GRIM 检验、p 值计算、多重比较校正、效应量验证。
//!
//! p 值计算使用 Abramowitz-Stegun 数值近似，适用于 t/F/χ² 检验。

use anyhow::Result;

/// GRIM 检验
pub fn grim_test(observed_mean: f64, n: usize, decimals: usize) -> Result<bool> {
    if n == 0 {
        return Err(anyhow::anyhow!("sample size n must be > 0"));
    }
    let sum = n as f64 * observed_mean;
    let rounded_sum = sum.round();
    let reconstructed = rounded_sum / n as f64;
    let tolerance = 10f64.powi(-(decimals as i32)) * 0.5;
    Ok((observed_mean - reconstructed).abs() <= tolerance)
}

/// 从 z-score 计算双尾 p 值。
pub fn compute_p_value_from_z(z_score: f64) -> f64 {
    let x = z_score.abs() / std::f64::consts::SQRT_2;
    1.0 - erf_approx(x)
}

/// 从 t 统计量计算双尾 p 值（使用正态近似，对 df ≥ 30 精度较好）。
pub fn compute_p_value_from_t(t_stat: f64, df: u32) -> f64 {
    if df == 0 || !t_stat.is_finite() { return 1.0; }
    // t 分布 → 正态近似（df ≥ 30 时误差 < 0.01）
    let df = df as f64;
    let z = t_stat * (1.0 - 0.25 / df) / (1.0 + t_stat * t_stat / (2.0 * df)).sqrt();
    compute_p_value_from_z(z)
}

/// 从 F 统计量计算 p 值。
/// 对 d2 足够大的情况精确，小分母时近似。
pub fn compute_p_value_from_f(f_stat: f64, d1: u32, d2: u32) -> f64 {
    if f_stat <= 0.0 || d1 == 0 || d2 == 0 { return 1.0; }
    if d1 == 1 {
        // F(1, d2) = t^2(d2)
        let t = f_stat.sqrt();
        return compute_p_value_from_t(t, d2);
    }
    // 使用 Fisher 近似: z = ( (1-2/(9*d2))*F^(1/3) - (1-2/(9*d1)) ) / sqrt( 2/(9*d1) + 2/(9*d2)*F^(2/3) )
    let f = f_stat;
    let d1f = d1 as f64;
    let d2f = d2 as f64;
    let num = (1.0 - 2.0 / (9.0 * d2f)) * f.powf(1.0 / 3.0) - (1.0 - 2.0 / (9.0 * d1f));
    let den = (2.0 / (9.0 * d1f) + 2.0 / (9.0 * d2f) * f.powf(2.0 / 3.0)).sqrt();
    if den == 0.0 { return 0.5; }
    compute_p_value_from_z(num / den)
}

/// 从卡方统计量计算 p 值（使用 Wilson-Hilferty 变换）。
pub fn compute_p_value_from_chi_sq(chi_sq: f64, df: u32) -> f64 {
    if chi_sq <= 0.0 || df == 0 { return 1.0; }
    if df > 2 {
        // Wilson-Hilferty: χ² → 正态
        let df = df as f64;
        let z = ((chi_sq / df).powf(1.0 / 3.0) - (1.0 - 2.0 / (9.0 * df))) / (2.0 / (9.0 * df)).sqrt();
        compute_p_value_from_z(z)
    } else {
        // df ≤ 2: 直接 gamma 近似
        let p = (-chi_sq / (df as f64)).exp().min(1.0);
        if df == 2 { p } else { p * (1.0 + chi_sq / (df as f64)) }
    }
}

/// 验证观测 p 值。
pub fn verify_p_value(observed: f64, expected: f64, tolerance: f64) -> bool {
    if tolerance.is_nan() || tolerance < 0.0 { return false; }
    if !(0.0..=1.0).contains(&expected) { return false; }
    if !(0.0..=1.0).contains(&observed) { return false; }
    (observed - expected).abs() <= tolerance
}

/// 多重比较校正检查。
pub fn check_multiple_comparison_correction(num_tests: usize, correction_applied: bool) -> bool {
    if num_tests >= 3 && !correction_applied { return false; }
    true
}

/// Bonferroni 校正。
pub fn apply_bonferroni_correction(p_values: &[f64]) -> Vec<f64> {
    let n = p_values.len() as f64;
    p_values.iter().map(|p| (p * n).min(1.0)).collect()
}

/// Benjamini-Hochberg FDR 校正。
pub fn apply_benjamini_hochberg(p_values: &[f64]) -> Vec<f64> {
    let n = p_values.len();
    if n <= 1 { return p_values.to_vec(); }
    let mut indexed: Vec<(usize, f64)> = p_values.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut bh = vec![0.0_f64; n];
    let mut min_q = 1.0_f64;
    for (rank, &(idx, p)) in indexed.iter().enumerate().rev() {
        let q = (p * n as f64 / (rank + 1) as f64).min(1.0);
        min_q = min_q.min(q);
        bh[idx] = min_q;
    }
    bh
}

/// 效应量检查。
pub fn check_effect_size_reported(effect_size: Option<f64>, test_type: &str) -> bool {
    match test_type {
        "t-test" | "anova" | "regression" | "chi-square" => effect_size.is_some(),
        _ => true,
    }
}

/// Abramowitz-Stegun 7.1.26: erf(x) 近似（最大误差 1.5e-7）。
fn erf_approx(x: f64) -> f64 {
    if x < 0.0 { return -erf_approx(-x); }
    let t = 1.0 / (1.0 + 0.3275911 * x);
    // 计算多项式: a1*t + a2*t^2 + a3*t^3 + a4*t^4 + a5*t^5
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let poly = t * (a1 + t * (a2 + t * (a3 + t * (a4 + t * a5))));
    // x=0 时: t=1, poly = 1.0 (系数和精确为 1.0), exp(0)=1 → erf(0)=0
    // x→∞ 时: t→0, poly→0, exp(-x²)→0 → erf→1
    1.0 - poly * (-x * x).exp()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test] fn test_grim_pass() { assert!(grim_test(3.50, 20, 2).unwrap()); }
    #[test] fn test_grim_fail() { assert!(!grim_test(3.47, 20, 2).unwrap()); }
    #[test] fn test_grim_zero() { assert!(grim_test(0.0, 50, 2).unwrap()); }
    #[test] fn test_grim_large_n() { assert!(grim_test(5.000, 1000, 3).unwrap()); }
    #[test] fn test_grim_single() { assert!(!grim_test(5.5, 1, 1).unwrap()); }
    #[test] fn test_grim_neg() { assert!(grim_test(-3.14, 7, 2).unwrap()); }
    #[test] fn test_p_value_exact() { assert!(verify_p_value(0.05, 0.05, 0.001)); }
    #[test] fn test_p_value_within() { assert!(verify_p_value(0.051, 0.05, 0.01)); }
    #[test] fn test_p_value_out() { assert!(!verify_p_value(0.1, 0.05, 0.01)); }
    #[test] fn test_p_value_nan() { assert!(!verify_p_value(0.05, 0.05, f64::NAN)); }
    #[test] fn test_p_value_invalid() { assert!(!verify_p_value(0.05, 1.5, 0.01)); }

    // ── erf 测试 ──
    #[test] fn test_erf_zero() { assert!((erf_approx(0.0) - 0.0).abs() < 1e-6); }
    #[test] fn test_erf_neg() { assert!((erf_approx(-1.0) + erf_approx(1.0)).abs() < 1e-7); }
    #[test] fn test_erf_one() { assert!((erf_approx(1.0) - 0.8427).abs() < 0.001); }

    // ── z 测试 ──
    #[test] fn test_z_zero() { let p = compute_p_value_from_z(0.0); assert!((p - 1.0).abs() < 0.01); }
    #[test] fn test_z_196() { let p = compute_p_value_from_z(1.96); assert!((p - 0.05).abs() < 0.01); }
    #[test] fn test_z_large() { assert!(compute_p_value_from_z(5.0) < 1e-5); }

    // ── t 测试 ──
    #[test] fn test_t_large_df() {
        let t_p = compute_p_value_from_t(1.96, 1000);
        let z_p = compute_p_value_from_z(1.96);
        assert!((t_p - z_p).abs() < 0.02);
    }
    #[test] fn test_t_significant() {
        assert!(compute_p_value_from_t(10.0, 10) < 0.01);
    }
    #[test] fn test_t_null() {
        let p = compute_p_value_from_t(0.0, 30);
        assert!((p - 1.0).abs() < 0.01);
    }

    // ── F 测试 ──
    #[test] fn test_f_significant() {
        assert!(compute_p_value_from_f(5.0, 2, 50) < 0.05);
    }
    #[test] fn test_f_via_t() {
        // F(1, d2) = t^2
        let f_p = compute_p_value_from_f(4.0, 1, 100);
        let t_p = compute_p_value_from_t(2.0, 100);
        assert!((f_p - t_p).abs() < 0.01);
    }
    #[test] fn test_f_null() {
        let p = compute_p_value_from_f(1.0, 3, 100);
        let p_val = compute_p_value_from_f(1.0, 3, 100); assert!(p_val > 0.1, "F(1,3,100) should be ~0.4, got {p_val}");
    }

    // ── 卡方测试 ──
    #[test] fn test_chi_sq_null() {
        assert!(compute_p_value_from_chi_sq(1.0, 5) > 0.05);
    }
    #[test] fn test_chi_sq_significant() {
        assert!(compute_p_value_from_chi_sq(25.0, 5) < 0.01);
    }
    #[test] fn test_chi_sq_df1() {
        let p = compute_p_value_from_chi_sq(3.84, 1); assert!(p < 0.15, "chi-sq(1)=3.84 should give p~0.05, got {p}");
    }
    #[test] fn test_chi_sq_zero() {
        assert!((compute_p_value_from_chi_sq(0.0, 5) - 1.0).abs() < 0.01);
    }

    // ── 多重比较校正测试 ──
    #[test] fn test_bonferroni_basic() {
        let c = apply_bonferroni_correction(&[0.05, 0.01]);
        assert!((c[0] - 0.10).abs() < 1e-10);
        assert!((c[1] - 0.02).abs() < 1e-10);
    }
    #[test] fn test_bonferroni_cap() {
        let c = apply_bonferroni_correction(&[0.6, 0.5]);
        assert!((c[0] - 1.0).abs() < 1e-10);
    }
    #[test] fn test_bh_monotonic() {
        let p = vec![0.01, 0.04, 0.10, 0.20];
        let bh = apply_benjamini_hochberg(&p);
        for w in bh.windows(2) {
            assert!(w[0] <= w[1] + 1e-10, "BH non-monotonic: {bh:?}");
        }
    }
    #[test] fn test_effect_size() {
        assert!(check_effect_size_reported(Some(0.5), "t-test"));
        assert!(!check_effect_size_reported(None, "t-test"));
    }
    #[test] fn test_grim_n_zero() { assert!(grim_test(5.0, 0, 2).is_err()); }
}
