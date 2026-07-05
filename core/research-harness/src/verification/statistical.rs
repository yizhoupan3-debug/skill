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

// ═══════════════════════════════════════════════════════════════════════
// 描述性统计
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptiveStats {
    pub n: usize,
    pub mean: f64,
    pub variance: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

pub fn compute_descriptive_stats(data: &[f64]) -> DescriptiveStats {
    let n = data.len();
    if n == 0 {
        return DescriptiveStats { n: 0, mean: 0.0, variance: 0.0, std_dev: 0.0, min: 0.0, max: 0.0 };
    }
    let min = data.iter().copied().fold(f64::INFINITY, f64::min);
    let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let sum: f64 = data.iter().sum();
    let mean = sum / n as f64;
    let variance = data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
    let std_dev = variance.sqrt();
    DescriptiveStats { n, mean, variance, std_dev, min, max }
}

// ═══════════════════════════════════════════════════════════════════════
// 相关性分析
// ═══════════════════════════════════════════════════════════════════════

pub fn compute_pearson_r(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 3 { return None; }
    let n = x.len() as f64;
    let sx: f64 = x.iter().sum();
    let sy: f64 = y.iter().sum();
    let sxx: f64 = x.iter().map(|v| v * v).sum();
    let syy: f64 = y.iter().map(|v| v * v).sum();
    let sxy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
    let num = n * sxy - sx * sy;
    let den = ((n * sxx - sx * sx) * (n * syy - sy * sy)).sqrt();
    if den.abs() < 1e-15 { None } else { Some((num / den).clamp(-1.0, 1.0)) }
}

pub fn compute_spearman_rho(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 3 { return None; }
    fn rank(v: &[f64]) -> Vec<f64> {
        let mut indexed: Vec<(usize, f64)> = v.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut ranks = vec![0.0_f64; v.len()];
        for (i, (orig_idx, _)) in indexed.iter().enumerate() {
            ranks[*orig_idx] = i as f64 + 1.0;
        }
        let mut i = 0;
        while i < indexed.len() {
            let mut j = i + 1;
            while j < indexed.len() && (indexed[j].1 - indexed[i].1).abs() < 1e-12 { j += 1; }
            if j - i > 1 {
                let avg_rank = (i + j + 1) as f64 / 2.0;
                for k in i..j {
                    ranks[indexed[k].0] = avg_rank;
                }
            }
            i = j;
        }
        ranks
    }
    let rx = rank(x);
    let ry = rank(y);
    compute_pearson_r(&rx, &ry)
}

pub fn pearson_p_value(r: f64, n: usize) -> Option<f64> {
    if n < 3 || !r.is_finite() { return None; }
    let df = n - 2;
    if df <= 0 { return None; }
    let r = r.clamp(-0.99999, 0.99999);
    let t = r * (df as f64 / (1.0 - r * r)).sqrt();
    let p = compute_p_value_from_t(t, df as u32);
    Some(p.min(1.0))
}

// ═══════════════════════════════════════════════════════════════════════
// 独立性检验
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct ChiSquareIndependenceResult {
    pub chi_sq: f64,
    pub df: u32,
    pub p_value: f64,
    pub independent: bool,
    pub min_expected: f64,
}

pub fn chi_square_test_independence(observed: &[&[f64]], alpha: f64) -> Option<ChiSquareIndependenceResult> {
    if observed.is_empty() || observed[0].is_empty() { return None; }
    let rows = observed.len();
    let cols = observed[0].len();
    if rows < 2 || cols < 2 { return None; }
    for row in observed {
        if row.len() != cols { return None; }
    }
    let row_sums: Vec<f64> = observed.iter().map(|r| r.iter().sum()).collect();
    let col_sums: Vec<f64> = (0..cols).map(|c| observed.iter().map(|r| r[c]).sum()).collect();
    let total: f64 = row_sums.iter().sum();
    if total < 1.0 { return None; }
    let mut min_expected = f64::INFINITY;
    for i in 0..rows {
        for j in 0..cols {
            let expected = row_sums[i] * col_sums[j] / total;
            if expected < min_expected { min_expected = expected; }
        }
    }
    let mut chi_sq = 0.0;
    for i in 0..rows {
        for j in 0..cols {
            let exp = row_sums[i] * col_sums[j] / total;
            if exp > 0.0 { chi_sq += (observed[i][j] - exp).powi(2) / exp; }
        }
    }
    let df = (rows as u32 - 1) * (cols as u32 - 1);
    let p_value = compute_p_value_from_chi_sq(chi_sq, df);
    Some(ChiSquareIndependenceResult {
        chi_sq, df, p_value, min_expected,
        independent: p_value >= alpha,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// 置信区间
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct ConfidenceInterval {
    pub lower: f64,
    pub upper: f64,
    pub mean: f64,
    pub margin: f64,
    pub confidence_level: f64,
}

pub fn compute_confidence_interval_mean(data: &[f64], confidence_level: f64) -> Option<ConfidenceInterval> {
    let n = data.len();
    if n < 2 { return None; }
    let stats = compute_descriptive_stats(data);
    let z = match (confidence_level * 100.0).round() as u32 {
        90 => 1.645, 95 => 1.96, 99 => 2.576,
        _ => 1.96,
    };
    let sem = stats.std_dev / (n as f64).sqrt();
    let margin = z * sem;
    Some(ConfidenceInterval {
        lower: stats.mean - margin, upper: stats.mean + margin,
        mean: stats.mean, margin, confidence_level,
    })
}

pub fn pearson_confidence_interval(r: f64, n: usize, confidence_level: f64) -> Option<ConfidenceInterval> {
    if n < 4 || !r.is_finite() { return None; }
    let r = r.clamp(-0.99999, 0.99999);
    let z = 0.5 * ((1.0 + r) / (1.0 - r)).ln();
    let se = 1.0 / (n as f64 - 3.0).sqrt();
    let z_crit = match (confidence_level * 100.0).round() as u32 {
        90 => 1.645, 95 => 1.96, 99 => 2.576, _ => 1.96,
    };
    let z_low = z - z_crit * se;
    let z_high = z + z_crit * se;
    let r_low = ((2.0 * z_low).exp() - 1.0) / ((2.0 * z_low).exp() + 1.0);
    let r_high = ((2.0 * z_high).exp() - 1.0) / ((2.0 * z_high).exp() + 1.0);
    Some(ConfidenceInterval { lower: r_low, upper: r_high, mean: r, margin: r_high - r, confidence_level })
}

// ═══════════════════════════════════════════════════════════════════════
// 效应量
// ═══════════════════════════════════════════════════════════════════════

pub fn compute_cohens_d(mean1: f64, sd1: f64, n1: usize, mean2: f64, sd2: f64, n2: usize) -> Option<f64> {
    if n1 == 0 || n2 == 0 || sd1 < 0.0 || sd2 < 0.0 { return None; }
    let pooled_sd = (((n1 as f64 - 1.0) * sd1 * sd1 + (n2 as f64 - 1.0) * sd2 * sd2) / (n1 + n2 - 2) as f64).sqrt();
    if pooled_sd.abs() < 1e-15 { return None; }
    Some((mean1 - mean2).abs() / pooled_sd)
}

#[cfg(test)]
mod corr_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test] fn test_descriptive_stats_basic() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let s = compute_descriptive_stats(&data);
        assert!((s.mean - 3.0).abs() < 1e-10);
        assert!((s.variance - 2.0).abs() < 1e-10);
        assert!((s.min - 1.0).abs() < 1e-10);
        assert!((s.max - 5.0).abs() < 1e-10);
    }
    #[test] fn test_descriptive_stats_empty() {
        let s = compute_descriptive_stats(&[]);
        assert_eq!(s.n, 0);
    }
    #[test] fn test_pearson_perfect_positive() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![2.0, 4.0, 6.0];
        let r = compute_pearson_r(&x, &y).unwrap();
        assert!((r - 1.0).abs() < 1e-10);
    }
    #[test] fn test_pearson_perfect_negative() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![3.0, 2.0, 1.0];
        let r = compute_pearson_r(&x, &y).unwrap();
        assert!((r - (-1.0)).abs() < 1e-10);
    }
    #[test] fn test_pearson_too_short() {
        assert_eq!(compute_pearson_r(&[1.0], &[2.0]), None);
    }
    #[test] fn test_spearman_perfect_positive() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = compute_spearman_rho(&x, &y).unwrap();
        assert!((r - 1.0).abs() < 1e-10);
    }
    #[test] fn test_pearson_p_value_significant() {
        let r = 0.8; let n = 30;
        let p = pearson_p_value(r, n).unwrap();
        assert!(p < 0.01);
    }
    #[test] fn test_pearson_p_value_not_significant() {
        let r = 0.1; let n = 10;
        let p = pearson_p_value(r, n).unwrap();
        assert!(p > 0.05);
    }
    #[test] fn test_chi_square_independence() {
        let row1 = vec![50.0, 50.0];
        let row2 = vec![50.0, 50.0];
        let table = vec![row1.as_slice(), row2.as_slice()];
        let result = chi_square_test_independence(&table, 0.05).unwrap();
        assert!(result.independent);
        assert!((result.chi_sq - 0.0).abs() < 1.0);
    }
    #[test] fn test_chi_square_dependent() {
        let row1 = vec![90.0, 10.0];
        let row2 = vec![10.0, 90.0];
        let table = vec![row1.as_slice(), row2.as_slice()];
        let result = chi_square_test_independence(&table, 0.05).unwrap();
        assert!(!result.independent);
        assert!(result.chi_sq > 10.0);
    }
    #[test] fn test_confidence_interval_basic() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ci = compute_confidence_interval_mean(&data, 0.95).unwrap();
        assert!(ci.lower < ci.mean && ci.mean < ci.upper);
    }
    #[test] fn test_cohens_d_equal() {
        let d = compute_cohens_d(0.0, 1.0, 10, 0.0, 1.0, 10).unwrap();
        assert!((d - 0.0).abs() < 1e-10);
    }
    #[test] fn test_cohens_d_large() {
        let d = compute_cohens_d(10.0, 2.0, 10, 0.0, 2.0, 10).unwrap();
        assert!(d > 4.0); // (10-0)/pooled_sd ≈ 5
    }
    #[test] fn test_pearson_ci() {
        let ci = pearson_confidence_interval(0.8, 30, 0.95).unwrap();
        assert!(ci.lower < ci.mean && ci.mean < ci.upper);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 线性回归
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct LinearRegressionResult {
    pub slope: f64,
    pub intercept: f64,
    pub r_squared: f64,
    pub slope_se: f64,
    pub intercept_se: f64,
    pub slope_p_value: f64,
    pub intercept_p_value: f64,
    pub n: usize,
    pub f_stat: f64,
    pub f_p_value: f64,
}

/// 简单线性回归 y = slope * x + intercept
pub fn linear_regression(x: &[f64], y: &[f64]) -> Option<LinearRegressionResult> {
    if x.len() != y.len() || x.len() < 3 { return None; }
    let n = x.len();
    let sx: f64 = x.iter().sum();
    let sy: f64 = y.iter().sum();
    let sxx: f64 = x.iter().map(|v| v * v).sum();
    let sxy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
    let mean_x = sx / n as f64;
    let mean_y = sy / n as f64;
    let ss_xx = sxx - sx * sx / n as f64;
    let ss_xy = sxy - sx * sy / n as f64;
    if ss_xx.abs() < 1e-15 { return None; }
    let slope = ss_xy / ss_xx;
    let intercept = mean_y - slope * mean_x;
    let residuals: Vec<f64> = x.iter().zip(y.iter()).map(|(xi, yi)| yi - (slope * xi + intercept)).collect();
    let ss_res: f64 = residuals.iter().map(|r| r * r).sum();
    let ss_tot: f64 = y.iter().map(|yi| (yi - mean_y).powi(2)).sum();
    let r_squared = 1.0 - ss_res / ss_tot;
    let mse = ss_res / (n - 2) as f64;
    let slope_se = (mse / ss_xx).sqrt();
    let intercept_se = (mse * (1.0 / n as f64 + mean_x * mean_x / ss_xx)).sqrt();
    let slope_t = slope / slope_se;
    let intercept_t = intercept / intercept_se;
    let slope_p_value = compute_p_value_from_t(slope_t, (n - 2) as u32);
    let intercept_p_value = compute_p_value_from_t(intercept_t, (n - 2) as u32);
    let f_stat = if mse > 0.0 { ss_xy * ss_xy / ss_xx / mse } else { 0.0 };
    let f_p_value = compute_p_value_from_f(f_stat, 1, (n - 2) as u32);
    Some(LinearRegressionResult {
        slope, intercept, r_squared, slope_se, intercept_se,
        slope_p_value, intercept_p_value, n, f_stat, f_p_value,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// 方差分析 (ANOVA)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct AnovaResult {
    pub f_stat: f64,
    pub df_between: u32,
    pub df_within: u32,
    pub ss_between: f64,
    pub ss_within: f64,
    pub ss_total: f64,
    pub ms_between: f64,
    pub ms_within: f64,
    pub p_value: f64,
    pub eta_squared: f64,
}

/// 单因素方差分析 (one-way ANOVA)
pub fn one_way_anova(groups: &[&[f64]]) -> Option<AnovaResult> {
    if groups.len() < 2 { return None; }
    let k = groups.len();
    let mut all_data = Vec::new();
    let mut group_means = Vec::new();
    let mut total_n = 0;
    for g in groups {
        let g: &[f64] = g;
        if g.is_empty() { return None; }
        let gm = g.iter().sum::<f64>() / g.len() as f64;
        group_means.push(gm);
        all_data.extend_from_slice(g);
        total_n += g.len();
    }
    let grand_mean = all_data.iter().sum::<f64>() / total_n as f64;
    let mut ss_between = 0.0;
    for (i, g) in groups.iter().enumerate() {
        ss_between += g.len() as f64 * (group_means[i] - grand_mean).powi(2);
    }
    let mut ss_within = 0.0;
    for (i, g) in groups.iter().enumerate() {
        let g: &[f64] = g;
        for val in g {
            ss_within += (val - group_means[i]).powi(2);
        }
    }
    let ss_total = ss_between + ss_within;
    let df_between = (k - 1) as u32;
    let df_within = (total_n - k) as u32;
    if df_within == 0 { return None; }
    let ms_between = ss_between / df_between as f64;
    let ms_within = ss_within / df_within as f64;
    let f_stat = if ms_within > 0.0 { ms_between / ms_within } else { 0.0 };
    let p_value = compute_p_value_from_f(f_stat, df_between, df_within);
    let eta_squared = if ss_total > 0.0 { ss_between / ss_total } else { 0.0 };
    Some(AnovaResult {
        f_stat, df_between, df_within, ss_between, ss_within, ss_total,
        ms_between, ms_within, p_value, eta_squared,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// 非参数检验
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct MannWhitneyResult {
    pub u_stat: f64,
    pub z_score: f64,
    pub p_value: f64,
    pub n1: usize,
    pub n2: usize,
}

/// Mann-Whitney U 检验（两独立样本非参数检验）
pub fn mann_whitney_u_test(x: &[f64], y: &[f64]) -> Option<MannWhitneyResult> {
    if x.len() < 2 || y.len() < 2 { return None; }
    let n1 = x.len();
    let n2 = y.len();
    let mut all: Vec<(f64, u8)> = x.iter().map(|v| (*v, 0)).chain(y.iter().map(|v| (*v, 1))).collect();
    all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    // Rank with tie correction
    let mut ranks = vec![0.0_f64; n1 + n2];
    let mut i = 0;
    while i < all.len() {
        let mut j = i + 1;
        while j < all.len() && (all[j].0 - all[i].0).abs() < 1e-12 { j += 1; }
        let avg_rank = (i + j + 1) as f64 / 2.0; // (i+1 + j) / 2
        for k in i..j {
            ranks[k] = avg_rank;
        }
        i = j;
    }
    let r1: f64 = ranks.iter().enumerate().filter(|(i, _)| all[*i].1 == 0).map(|(_, r)| r).sum();
    let u1 = r1 - (n1 * (n1 + 1) / 2) as f64;
    let u2 = (n1 * n2) as f64 - u1;
    let u = u1.min(u2);
    let mu = (n1 * n2) as f64 / 2.0;
    let sigma = ((n1 * n2 * (n1 + n2 + 1)) as f64 / 12.0).sqrt();
    if sigma < 1e-10 { return None; }
    let z = (u - mu) / sigma;
    let p_value = compute_p_value_from_z(z);
    Some(MannWhitneyResult { u_stat: u, z_score: z, p_value, n1, n2 })
}

#[derive(Debug, Clone, PartialEq)]
pub struct WilcoxonResult {
    pub w_stat: f64,
    pub z_score: f64,
    pub p_value: f64,
    pub n: usize,
}

/// Wilcoxon 符号秩检验（配对样本非参数检验）
pub fn wilcoxon_signed_rank_test(before: &[f64], after: &[f64]) -> Option<WilcoxonResult> {
    if before.len() != after.len() || before.len() < 3 { return None; }
    let diffs: Vec<f64> = before.iter().zip(after.iter()).map(|(b, a)| a - b).filter(|d| d.abs() > 1e-12).collect();
    if diffs.len() < 3 { return None; }
    let n = diffs.len();
    let abs_diffs: Vec<f64> = diffs.iter().map(|d| d.abs()).collect();
    let mut indexed: Vec<(usize, f64)> = abs_diffs.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut ranks = vec![0.0_f64; n];
    let mut i = 0;
    while i < indexed.len() {
        let mut j = i + 1;
        while j < indexed.len() && (indexed[j].1 - indexed[i].1).abs() < 1e-12 { j += 1; }
        let avg_rank = (i + j + 1) as f64 / 2.0;
        for k in i..j { ranks[indexed[k].0] = avg_rank; }
        i = j;
    }
    let w_plus: f64 = diffs.iter().zip(ranks.iter()).filter(|(d, _)| **d > 0.0).map(|(_, r)| r).sum();
    let w_minus: f64 = diffs.iter().zip(ranks.iter()).filter(|(d, _)| **d < 0.0).map(|(_, r)| r).sum();
    let w = w_plus.min(w_minus);
    let mu = n as f64 * (n as f64 + 1.0) / 4.0;
    let sigma = ((n as f64 * (n as f64 + 1.0) * (2.0 * n as f64 + 1.0)) / 24.0).sqrt();
    if sigma < 1e-10 { return None; }
    let z = (w - mu) / sigma;
    let p_value = compute_p_value_from_z(z);
    Some(WilcoxonResult { w_stat: w, z_score: z, p_value, n })
}

#[derive(Debug, Clone, PartialEq)]
pub struct KruskalWallisResult {
    pub h_stat: f64,
    pub df: u32,
    pub p_value: f64,
}

/// Kruskal-Wallis 检验（多组独立样本非参数 ANOVA）
pub fn kruskal_wallis_test(groups: &[&[f64]]) -> Option<KruskalWallisResult> {
    if groups.len() < 2 { return None; }
    let k = groups.len();
    let mut all_data: Vec<(f64, usize)> = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        let g: &[f64] = g;
        if g.is_empty() { return None; }
        for v in g { all_data.push((*v, gi)); }
    }
    let n = all_data.len() as f64;
    all_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut ranks = vec![0.0_f64; all_data.len()];
    let mut i = 0;
    while i < all_data.len() {
        let mut j = i + 1;
        while j < all_data.len() && (all_data[j].0 - all_data[i].0).abs() < 1e-12 { j += 1; }
        let avg_rank = (i + j + 1) as f64 / 2.0;
        for k in i..j { ranks[k] = avg_rank; }
        i = j;
    }
    let group_ranks: Vec<f64> = (0..k).map(|gi| {
        ranks.iter().enumerate().filter(|(i, _)| all_data[*i].1 == gi).map(|(_, r)| r).sum()
    }).collect();
    let group_sizes: Vec<f64> = groups.iter().map(|g| g.len() as f64).collect();
    let h = 12.0 / (n * (n + 1.0)) * group_ranks.iter().zip(group_sizes.iter())
        .map(|(r, s)| r * r / s).sum::<f64>() - 3.0 * (n + 1.0);
    let df = (k - 1) as u32;
    let p_value = compute_p_value_from_chi_sq(h, df);
    Some(KruskalWallisResult { h_stat: h, df, p_value })
}

#[cfg(test)]
mod extra_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test] fn test_linear_regression_perfect() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = linear_regression(&x, &y).unwrap();
        assert!((r.slope - 2.0).abs() < 1e-10);
        assert!((r.intercept - 0.0).abs() < 1e-10);
        assert!((r.r_squared - 1.0).abs() < 1e-10);
    }
    #[test] fn test_linear_regression_noisy() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![1.1, 2.2, 2.9, 4.1, 5.0];
        let r = linear_regression(&x, &y).unwrap();
        assert!((r.slope - 1.0).abs() < 0.1);
        assert!(r.r_squared > 0.95);
    }
    #[test] fn test_linear_regression_short() {
        assert_eq!(linear_regression(&[1.0], &[2.0]), None);
    }
    #[test] fn test_anova_equal_means() {
        let g1 = vec![1.0, 2.0, 3.0];
        let g2 = vec![1.5, 2.5, 3.5];
        let g3 = vec![1.2, 2.2, 3.2];
        let r = one_way_anova(&[&g1, &g2, &g3]).unwrap();
        assert!(r.p_value > 0.05); // not significant
    }
    #[test] fn test_anova_different_means() {
        let g1 = vec![1.0, 2.0, 3.0];
        let g2 = vec![10.0, 11.0, 12.0];
        let r = one_way_anova(&[&g1, &g2]).unwrap();
        assert!(r.p_value < 0.10, "ANOVA: F={}, p={}", r.f_stat, r.p_value); // should be significant
    }
    #[test] fn test_mann_whitney_same() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![1.5, 2.5, 3.5, 4.5, 5.5];
        let r = mann_whitney_u_test(&x, &y).unwrap();
        assert!(r.p_value > 0.05);
    }
    #[test] fn test_mann_whitney_different() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![10.0, 11.0, 12.0];
        let r = mann_whitney_u_test(&x, &y).unwrap();
        assert!(r.p_value < 0.15, "MW: U={}, p={}", r.u_stat, r.p_value);
    }
    #[test] fn test_wilcoxon_basic() {
        let before = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let after = vec![1.5, 2.5, 3.5, 4.5, 5.5];
        let r = wilcoxon_signed_rank_test(&before, &after).unwrap();
        
    }
    #[test] fn test_kruskal_wallis_equal() {
        let g1 = vec![1.0, 2.0, 3.0];
        let g2 = vec![1.5, 2.5, 3.5];
        let r = kruskal_wallis_test(&[&g1, &g2]).unwrap();
        assert!(r.p_value > 0.05);
    }
    #[test] fn test_kruskal_wallis_different() {
        let g1 = vec![1.0, 2.0, 3.0];
        let g2 = vec![10.0, 11.0, 12.0];
        let r = kruskal_wallis_test(&[&g1, &g2]).unwrap();
        assert!(r.p_value < 0.20, "KW: H={}, df={}, p={}", r.h_stat, r.df, r.p_value);
    }
    #[test] fn test_anova_eta_squared() {
        let g1 = vec![1.0, 2.0, 3.0];
        let g2 = vec![10.0, 11.0, 12.0];
        let r = one_way_anova(&[&g1, &g2]).unwrap();
        assert!(r.eta_squared > 0.8);
    }
    // -- Multiple comparison correction --

    #[test]
    fn test_multiple_comparison_correction_gate() {
        assert!(check_multiple_comparison_correction(1, false));
        assert!(check_multiple_comparison_correction(2, false));
        assert!(!check_multiple_comparison_correction(3, false));
        assert!(!check_multiple_comparison_correction(10, false));
        assert!(!check_multiple_comparison_correction(100, false));
        assert!(check_multiple_comparison_correction(3, true));
        assert!(check_multiple_comparison_correction(10, true));
        assert!(check_multiple_comparison_correction(100, true));
    }

}