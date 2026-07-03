//! Asymptotic / Order-of-Magnitude analysis — pure Rust implementation.
//!
//! Pure business logic: asymptotic relation types, chain verification,
//! magnitude estimation, growth comparison. No Python/SymPy subprocess.
//!
//! # Layer boundary
//!
//! FEATURE layer only. Tool dispatch (JSON → feature → JSON) is in
//! `mcp_tools.rs`.

use crate::types::{VerificationResult, VerificationStatus};
use serde::{Deserialize, Serialize};

// ===========================================================================
// Core types
// ===========================================================================

/// Asymptotic relation between two quantities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderRelation {
    /// ≲ — less-similar: |f| ≤ C|g| for some constant C
    LessSim,
    /// ≪ — much-less-than: |f| ≤ ε|g| for all ε > 0
    MuchLess,
    /// ≍ — asymptotically equivalent: f ≲ g ∧ g ≲ f
    Asymp,
}

impl OrderRelation {
    pub fn symbol(&self) -> &'static str {
        match self {
            OrderRelation::LessSim => "≲",
            OrderRelation::MuchLess => "≪",
            OrderRelation::Asymp => "≍",
        }
    }

    /// Returns true if composing across this relation preserves the same relation.
    pub fn is_transitive(&self) -> bool {
        true // All three are transitive
    }
}

/// A single step in an asymptotic chain: premise `relation` conclusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymptoticStep {
    pub premise: String,
    pub relation: OrderRelation,
    pub conclusion: String,
    pub justification: String,
}

/// A chain of asymptotic steps (f ≲ g ≪ h ≍ k, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymptoticChain {
    pub steps: Vec<AsymptoticStep>,
}

impl AsymptoticChain {
    pub fn new(steps: Vec<AsymptoticStep>) -> Self {
        Self { steps }
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// If all steps use the same relation, the chain is "pure".
    pub fn is_pure(&self) -> bool {
        if self.steps.len() < 2 {
            return true;
        }
        let first = &self.steps[0].relation;
        self.steps.iter().all(|s| s.relation == *first)
    }

    /// Collect unique relations in this chain.
    pub fn unique_relations(&self) -> Vec<OrderRelation> {
        let mut seen = Vec::new();
        for step in &self.steps {
            if !seen.contains(&step.relation) {
                seen.push(step.relation.clone());
            }
        }
        seen
    }
}

/// Report from composing an asymptotic chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainComposition {
    pub steps: Vec<AsymptoticStep>,
    pub is_pure: bool,
    pub unique_relations: Vec<String>,
    pub mixed_chain_warning: Option<String>,
}

/// Result of a magnitude estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagnitudeEstimate {
    pub expression: String,
    pub regime: String,
    pub leading_term: String,
    pub order: String,
}

// ===========================================================================
// Chain composition (pure Rust, no subprocess)
// ===========================================================================

/// Analyze an asymptotic chain without calling SymPy.
/// Returns composition report with mixed-chain detection.
pub fn compose_asymptotic_chain(chain: &AsymptoticChain) -> ChainComposition {
    let is_pure = chain.is_pure();
    let unique_rel: Vec<String> = chain
        .unique_relations()
        .iter()
        .map(|r| r.symbol().to_string())
        .collect();

    let warning = if !is_pure {
        Some(format!(
            "mixed chain ({}) — human review required",
            unique_rel.join(", ")
        ))
    } else {
        None
    };

    // Transitivity is structurally valid (all three relation types are
    // individually transitive; mixed chains get the WARN above instead).
    ChainComposition {
        steps: chain.steps.clone(),
        is_pure,
        unique_relations: unique_rel,
        mixed_chain_warning: warning,
    }
}

// ===========================================================================
// Magnitude estimation (pure Rust via symbolic module)
// ===========================================================================

/// Estimate the leading-order term of an expression in a given regime (e.g. n→∞, n→0).
pub fn magnitude_estimate(expr: &str, var: &str, regime: &str) -> VerificationResult {
    magnitude_estimate_with_name(expr, var, regime, "math_asymptotic_estimate")
}

/// Like `magnitude_estimate` with an explicit check name.
pub fn magnitude_estimate_with_name(
    expr: &str,
    var: &str,
    regime: &str,
    check_name: &str,
) -> VerificationResult {
    // Transform expression for non-infinity regimes
    let transformed_expr = transform_regime(expr, var, regime);
    let regime_detail = regime_description(regime);

    match crate::verification::symbolic::leading_term(&transformed_expr, var) {
        Ok((leading, order)) => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Pass,
            details: format!(
                "{expr} ~ {leading} (order: {order}) as {var}{regime_detail}"
            ),
            evidence_path: None,
        },
        Err(e) => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Warn,
            details: format!("estimate error: {e}"),
            evidence_path: None,
        },
    }
}

/// Transform expression for a given regime.
///
/// - `"oo"` or `"inf"`: no transformation (x → ∞)
/// - `"0"` or `"zero"`: substitute x → 1/x (then analyze x → ∞)
/// - Numeric constant `"c"`: substitute x → x + c (then analyze x → ∞).
///   This shifts the point of analysis to the finite non-zero location.
/// - Other strings (variable-like): treated as unrecognized (no transformation),
///   matching the existing fallback behavior.
fn transform_regime(expr: &str, var: &str, regime: &str) -> String {
    // Finite non-zero numeric constant: substitute var → var + c, then analyze n→0.
    if let Ok(c) = regime.parse::<f64>() {
        // "inf" parses as f64::INFINITY — skip to preserve existing "oo"|"inf" behavior.
        if c.is_finite() && c != 0.0 {
            // Substitute var → var + c
            let mut result = String::with_capacity(expr.len() + 8);
            let mut i = 0;
            let chars: Vec<char> = expr.chars().collect();
            while i < chars.len() {
                if chars[i].is_alphabetic() || chars[i] == '_' {
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let name: String = chars[start..i].iter().collect();
                    if name == var {
                        result.push_str(&format!("({}+{})", var, c));
                    } else {
                        result.push_str(&name);
                    }
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            return result;
        }
        // c == 0.0 falls through to the "0" / "zero" handler below
    }

    match regime {
        "0" | "zero" | "Zero" => {
            // Substitute var → 1/var
            let mut result = String::with_capacity(expr.len() + 8);
            let mut i = 0;
            let chars: Vec<char> = expr.chars().collect();
            while i < chars.len() {
                // Check if this position starts the variable name
                if chars[i].is_alphabetic() || chars[i] == '_' {
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let name: String = chars[start..i].iter().collect();
                    if name == var {
                        result.push_str(&format!("(1/{})", var));
                    } else {
                        result.push_str(&name);
                    }
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            result
        }
        _ => expr.to_string(), // For "oo", "inf", or unrecognized, no transformation
    }
}

/// Get human-readable regime description.
fn regime_description(regime: &str) -> String {
    match regime {
        "oo" | "inf" | "Inf" | "" => "→∞".to_string(),
        "0" | "zero" | "Zero" => "→0".to_string(),
        other => format!("→{other}"),
    }
}

/// Check tilde equivalence: f ~ g as var → regime (i.e. f/g → 1).
///
/// Uses the symbolic engine to verify that f and g have the same growth
/// class AND share the same leading term, which implies f/g → 1.
pub fn check_tilde_equivalence(
    f: &str,
    g: &str,
    var: &str,
    regime: &str,
) -> VerificationResult {
    check_tilde_equivalence_with_name(f, g, var, regime, "math_asymptotic_claim")
}

/// Like `check_tilde_equivalence` with an explicit check name.
pub fn check_tilde_equivalence_with_name(
    f: &str,
    g: &str,
    var: &str,
    regime: &str,
    check_name: &str,
) -> VerificationResult {
    let tf = transform_regime(f, var, regime);
    let tg = transform_regime(g, var, regime);
    let regime_desc = regime_description(regime);

    // Parse both expressions
    let f_expr = match crate::verification::symbolic::parse(&tf) {
        Ok(e) => e,
        Err(e) => {
            return VerificationResult {
                check_name: check_name.to_string(),
                status: VerificationStatus::Fail,
                details: format!("parse f failed: {e}"),
                evidence_path: None,
            };
        }
    };
    let g_expr = match crate::verification::symbolic::parse(&tg) {
        Ok(e) => e,
        Err(e) => {
            return VerificationResult {
                check_name: check_name.to_string(),
                status: VerificationStatus::Fail,
                details: format!("parse g failed: {e}"),
                evidence_path: None,
            };
        }
    };

    let gf = crate::verification::symbolic::classify_growth(&f_expr, var);
    let gg = crate::verification::symbolic::classify_growth(&g_expr, var);

    // Step 1: Same growth class is necessary (f ≍ g).
    // Step 2: Same leading term ensures f/g → 1 (not some other constant).
    let holds = if gf == gg {
        match (
            crate::verification::symbolic::leading_term(&tf, var),
            crate::verification::symbolic::leading_term(&tg, var),
        ) {
            (Ok((f_lead, _)), Ok((g_lead, _))) => f_lead == g_lead,
            _ => false,
        }
    } else {
        false
    };

    if holds {
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Pass,
            details: format!(
                "{f} ~ {g} holds as {var}{regime_desc} (same growth class, same leading term)"
            ),
            evidence_path: None,
        }
    } else {
        let reason = if gf != gg {
            format!(
                "different growth classes: {:?} vs {:?}",
                gf, gg
            )
        } else {
            match (
                crate::verification::symbolic::leading_term(&tf, var),
                crate::verification::symbolic::leading_term(&tg, var),
            ) {
                (Ok((f_lead, _)), Ok((g_lead, _))) => {
                    format!("leading terms differ: {f_lead} vs {g_lead}")
                }
                _ => "cannot determine leading terms".to_string(),
            }
        };
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Fail,
            details: format!(
                "{f} ~ {g} does NOT hold as {var}{regime_desc}: {reason}"
            ),
            evidence_path: None,
        }
    }
}


/// Verify an asymptotic claim: f relation g in a given regime.
pub fn check_asymptotic_claim(
    f: &str,
    g: &str,
    relation: &OrderRelation,
    var: &str,
    regime: &str,
) -> VerificationResult {
    check_asymptotic_claim_with_name(f, g, relation, var, regime, "math_asymptotic_claim")
}

/// Like `check_asymptotic_claim` with an explicit check name.
pub fn check_asymptotic_claim_with_name(
    f: &str,
    g: &str,
    relation: &OrderRelation,
    var: &str,
    regime: &str,
    check_name: &str,
) -> VerificationResult {
    let tf = transform_regime(f, var, regime);
    let tg = transform_regime(g, var, regime);
    let regime_desc = regime_description(regime);
    let f_expr = match crate::verification::symbolic::parse(&tf) {
        Ok(e) => e,
        Err(e) => {
            return VerificationResult {
                check_name: check_name.to_string(),
                status: VerificationStatus::Fail,
                details: format!("parse f failed: {e}"),
                evidence_path: None,
            };
        }
    };
    let g_expr = match crate::verification::symbolic::parse(&tg) {
        Ok(e) => e,
        Err(e) => {
            return VerificationResult {
                check_name: check_name.to_string(),
                status: VerificationStatus::Fail,
                details: format!("parse g failed: {e}"),
                evidence_path: None,
            };
        }
    };

    let gf = crate::verification::symbolic::classify_growth(&f_expr, var);
    let gg = crate::verification::symbolic::classify_growth(&g_expr, var);

    let cmp = crate::verification::symbolic::compare_growth_classes(&gf, &gg);
    let symbol = relation.symbol();

    let holds = match relation {
        OrderRelation::MuchLess => {
            // f ≪ g: f grows strictly slower than g
            cmp == std::cmp::Ordering::Less || (cmp == std::cmp::Ordering::Equal && gf != gg)
        }
        OrderRelation::LessSim => {
            // f ≲ g: f grows no faster than g
            cmp != std::cmp::Ordering::Greater
        }
        OrderRelation::Asymp => {
            // f ≍ g: same growth class and same parameters
            gf == gg
        }
    };

    if holds {
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Pass,
            details: format!("{f} {symbol} {g} holds as {var}{regime_desc}"),
            evidence_path: None,
        }
    } else {
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Fail,
            details: format!("{f} {symbol} {g} does NOT hold as {var}{regime_desc}"),
            evidence_path: None,
        }
    }
}

/// Verify an entire asymptotic chain.
/// Pure chains → auto PASS; mixed chains → auto WARN + human review.
/// The `sympy_check` parameter is accepted for backward compatibility but
/// is ignored — all checking is now done in pure Rust.
pub fn verify_asymptotic_chain(
    steps: &[AsymptoticStep],
    var: &str,
    regime: &str,
    _sympy_check: bool,
) -> VerificationResult {
    verify_asymptotic_chain_with_name(steps, var, regime, _sympy_check, "math_asymptotic_chain")
}

/// Like `verify_asymptotic_chain` with an explicit check name.
pub fn verify_asymptotic_chain_with_name(
    steps: &[AsymptoticStep],
    var: &str,
    regime: &str,
    _sympy_check: bool,
    check_name: &str,
) -> VerificationResult {
    if steps.is_empty() {
        return VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Fail,
            details: "empty asymptotic chain".into(),
            evidence_path: None,
        };
    }

    let chain = AsymptoticChain::new(steps.to_vec());
    let composition = compose_asymptotic_chain(&chain);

    // Mixed chain → WARN regardless of individual step results
    if let Some(warning) = &composition.mixed_chain_warning {
        return VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Warn,
            details: format!("{warning} — human review required for mixed relation chain"),
            evidence_path: None,
        };
    }

    // Pure chain: verify each step's growth ordering using the symbolic engine
    let mut step_details = Vec::new();
    let mut all_pass = true;

    for (i, step) in steps.iter().enumerate() {
        let vr = check_asymptotic_claim_with_name(
            &step.premise,
            &step.conclusion,
            &step.relation,
            var,
            regime,
            check_name,
        );
        match vr.status {
            VerificationStatus::Pass => {
                step_details.push(format!("Step {}: PASS", i + 1));
            }
            _ => {
                all_pass = false;
                step_details.push(format!("Step {}: {:?} — {}", i + 1, vr.status, vr.details));
            }
        }
    }

    if all_pass {
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Pass,
            details: format!(
                "Chain verified ({} steps): {}",
                steps.len(),
                step_details.join("; ")
            ),
            evidence_path: None,
        }
    } else {
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Fail,
            details: format!(
                "Chain verification FAILED ({} steps): {}",
                steps.len(),
                step_details.join("; ")
            ),
            evidence_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_chain_detection() {
        let steps = vec![
            AsymptoticStep {
                premise: "n".into(),
                relation: OrderRelation::LessSim,
                conclusion: "n^2".into(),
                justification: "poly".into(),
            },
            AsymptoticStep {
                premise: "n^2".into(),
                relation: OrderRelation::LessSim,
                conclusion: "n^3".into(),
                justification: "poly".into(),
            },
        ];
        let chain = AsymptoticChain::new(steps);
        assert!(chain.is_pure());
    }

    #[test]
    fn test_mixed_chain_detection() {
        let steps = vec![
            AsymptoticStep {
                premise: "n".into(),
                relation: OrderRelation::LessSim,
                conclusion: "n^2".into(),
                justification: "".into(),
            },
            AsymptoticStep {
                premise: "n^2".into(),
                relation: OrderRelation::MuchLess,
                conclusion: "2^n".into(),
                justification: "".into(),
            },
        ];
        let chain = AsymptoticChain::new(steps);
        assert!(!chain.is_pure());
        let comp = compose_asymptotic_chain(&chain);
        assert!(comp.mixed_chain_warning.is_some());
    }

    #[test]
    fn test_empty_chain() {
        let chain = AsymptoticChain::new(vec![]);
        assert!(chain.is_empty());
        assert!(chain.is_pure());
    }

    #[test]
    fn test_relation_symbols() {
        assert_eq!(OrderRelation::LessSim.symbol(), "≲");
        assert_eq!(OrderRelation::MuchLess.symbol(), "≪");
        assert_eq!(OrderRelation::Asymp.symbol(), "≍");
    }

    #[test]
    fn test_unique_relations() {
        let steps = vec![
            AsymptoticStep {
                premise: "a".into(),
                relation: OrderRelation::LessSim,
                conclusion: "b".into(),
                justification: "".into(),
            },
            AsymptoticStep {
                premise: "b".into(),
                relation: OrderRelation::LessSim,
                conclusion: "c".into(),
                justification: "".into(),
            },
        ];
        let chain = AsymptoticChain::new(steps);
        let rels = chain.unique_relations();
        assert_eq!(rels.len(), 1);
    }

    #[test]
    fn test_magnitude_estimate_polynomial() {
        let vr = magnitude_estimate("n^2 + n", "n", "oo");
        assert_eq!(vr.status, VerificationStatus::Pass);
        assert!(vr.details.contains("n^2"));
    }

    #[test]
    fn test_check_asymptotic_claim_log_vs_linear() {
        let vr = check_asymptotic_claim("log(n)", "n", &OrderRelation::MuchLess, "n", "oo");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "log(n) ≪ n should hold, got: {}",
            vr.details
        );
    }

    #[test]
    fn test_check_asymptotic_claim_linear_vs_quadratic() {
        let vr = check_asymptotic_claim("n", "n^2", &OrderRelation::MuchLess, "n", "oo");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "n ≪ n^2 should hold, got: {}",
            vr.details
        );
    }

    #[test]
    fn test_check_asymptotic_chain_pure() {
        let steps = vec![
            AsymptoticStep {
                premise: "log(n)".into(),
                relation: OrderRelation::MuchLess,
                conclusion: "n".into(),
                justification: "".into(),
            },
            AsymptoticStep {
                premise: "n".into(),
                relation: OrderRelation::MuchLess,
                conclusion: "n^2".into(),
                justification: "".into(),
            },
        ];
        let vr = verify_asymptotic_chain(&steps, "n", "oo", true);
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "pure chain should pass, got: {:?} ({})",
            vr.status,
            vr.details
        );
    }

    #[test]
    fn test_verify_asymptotic_chain_empty() {
        let vr = verify_asymptotic_chain(&[], "n", "oo", true);
        assert_eq!(
            vr.status,
            VerificationStatus::Fail,
            "empty chain should fail, got: {:?} ({})",
            vr.status,
            vr.details
        );
        assert!(vr.details.contains("empty"), "details: {}", vr.details);
    }

    #[test]
    fn test_verify_asymptotic_chain_single_step() {
        let steps = vec![AsymptoticStep {
            premise: "log(n)".into(),
            relation: OrderRelation::MuchLess,
            conclusion: "n".into(),
            justification: "".into(),
        }];
        let vr = verify_asymptotic_chain(&steps, "n", "oo", true);
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "single-step chain should pass, got: {:?} ({})",
            vr.status,
            vr.details
        );
    }

    #[test]
    fn test_verify_asymptotic_chain_failing() {
        let steps = vec![AsymptoticStep {
            premise: "n^2".into(),
            relation: OrderRelation::MuchLess,
            conclusion: "n".into(),
            justification: "".into(),
        }];
        let vr = verify_asymptotic_chain(&steps, "n", "oo", true);
        assert_eq!(
            vr.status,
            VerificationStatus::Fail,
            "chain with invalid claim should fail, got: {:?} ({})",
            vr.status,
            vr.details
        );
    }

    // ── transform_regime with regime = 0 ──

    #[test]
    fn test_transform_regime_zero_simple_var() {
        // n → (1/n)
        let result = transform_regime("n", "n", "0");
        assert_eq!(result, "(1/n)");
    }

    #[test]
    fn test_transform_regime_zero_polynomial() {
        // n^2 + n → (1/n)^2 + (1/n)
        let result = transform_regime("n^2 + n", "n", "0");
        assert_eq!(result, "(1/n)^2 + (1/n)");
    }

    #[test]
    fn test_transform_regime_zero_aliases() {
        // "zero" and "Zero" should also trigger substitution
        assert_eq!(transform_regime("n", "n", "zero"), "(1/n)");
        assert_eq!(transform_regime("n", "n", "Zero"), "(1/n)");
    }

    #[test]
    fn test_transform_regime_zero_var_not_present() {
        // Variable "n" not in expression → unchanged
        let result = transform_regime("x", "n", "0");
        assert_eq!(result, "x");
    }

    #[test]
    fn test_transform_regime_unrecognized() {
        // Unrecognized regimes return expr unchanged
        let result = transform_regime("n^2 + n", "n", "invalid");
        assert_eq!(result, "n^2 + n");
    }

    #[test]
    fn test_transform_regime_inf() {
        // "oo" and "inf" also return expr unchanged
        assert_eq!(transform_regime("n^2 + n", "n", "oo"), "n^2 + n");
        assert_eq!(transform_regime("n^2 + n", "n", "inf"), "n^2 + n");
    }

    // ── regime_description for all variants ──

    #[test]
    fn test_regime_description_all() {
        assert_eq!(regime_description("oo"), "→∞");
        assert_eq!(regime_description("inf"), "→∞");
        assert_eq!(regime_description("Inf"), "→∞");
        assert_eq!(regime_description(""), "→∞");
        assert_eq!(regime_description("0"), "→0");
        assert_eq!(regime_description("zero"), "→0");
        assert_eq!(regime_description("Zero"), "→0");
        assert_eq!(regime_description("42"), "→42");
        assert_eq!(regime_description("a"), "→a");
        assert_eq!(regime_description("custom"), "→custom");
    }

    // ── magnitude_estimate with regime = 0 ──

    #[test]
    fn test_magnitude_estimate_regime_zero() {
        let vr = magnitude_estimate("n^2 + n", "n", "0");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "magnitude_estimate with regime=0 should pass, got: {:?} ({})",
            vr.status,
            vr.details
        );
        assert!(vr.details.contains("→0"), "details: {}", vr.details);
    }

    #[test]
    fn test_magnitude_estimate_regime_zero_constant_only() {
        // Expression with no variable matching n → no transformation
        let vr = magnitude_estimate("42", "n", "0");
        assert_eq!(vr.status, VerificationStatus::Pass);
        assert!(vr.details.contains("→0"), "details: {}", vr.details);
    }

    // ── check_asymptotic_claim with regime = 0 ──

    #[test]
    fn test_check_asymptotic_claim_regime_zero_less_sim() {
        // n^2 decays faster than n as n→0, so n^2 ≲ n holds
        let vr = check_asymptotic_claim("n^2", "n", &OrderRelation::LessSim, "n", "0");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "n^2 ≲ n as n→0 should pass, got: {:?} ({})",
            vr.status,
            vr.details
        );
        assert!(vr.details.contains("→0"), "details: {}", vr.details);
    }

    #[test]
    fn test_check_asymptotic_claim_regime_zero_asymp() {
        // Same expression → asymptotic holds trivially
        let vr = check_asymptotic_claim("n", "n", &OrderRelation::Asymp, "n", "0");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "n ≍ n as n→0 should pass, got: {:?} ({})",
            vr.status,
            vr.details
        );
        assert!(vr.details.contains("→0"), "details: {}", vr.details);
    }

    // ── transform_regime with finite non-zero constant ──

    #[test]
    fn test_transform_regime_finite_nonzero() {
        // n^2 at n=1 → (n+1)^2, then analyze as n→0
        let result = transform_regime("n^2", "n", "1");
        assert_eq!(result, "(n+1)^2");
    }

    #[test]
    fn test_transform_regime_finite_negative() {
        let result = transform_regime("n", "n", "-2");
        assert_eq!(result, "(n+-2)");
    }

    #[test]
    fn test_transform_regime_finite_nonzero_var_not_present() {
        let result = transform_regime("x", "n", "1");
        assert_eq!(result, "x");
    }

    #[test]
    fn test_transform_regime_finite_nonzero_mixed() {
        // 2*n + c at n=3 → 2*(n+3) + c
        let result = transform_regime("2*n + c", "n", "3");
        assert_eq!(result, "2*(n+3) + c");
    }

    // ── check_tilde_equivalence ──

    #[test]
    fn test_check_tilde_identical() {
        // f = g → f ~ g trivially holds
        let vr = check_tilde_equivalence("n", "n", "n", "oo");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "n ~ n should pass, got: {:?} ({})",
            vr.status,
            vr.details
        );
    }

    #[test]
    fn test_check_tilde_same_leading_coeff() {
        // f = 2*n^2, g = 2*n^2 → ratio = 1
        let vr = check_tilde_equivalence("2*n^2", "2*n^2", "n", "oo");
        assert_eq!(
            vr.status,
            VerificationStatus::Pass,
            "same leading term should hold: {:?} ({})",
            vr.status,
            vr.details
        );
    }

    #[test]
    fn test_check_tilde_different_growth_class() {
        // n ≁ n^2
        let vr = check_tilde_equivalence("n", "n^2", "n", "oo");
        assert_eq!(
            vr.status,
            VerificationStatus::Fail,
            "n ≁ n^2 should fail, got: {:?} ({})",
            vr.status,
            vr.details
        );
    }

    #[test]
    fn test_check_tilde_constant_diff() {
        // f = 2n, g = n → f/g → 2 ≠ 1 (same growth class, but ratio ≠ 1)
        let vr = check_tilde_equivalence("2*n", "n", "n", "oo");
        assert_eq!(
            vr.status,
            VerificationStatus::Fail,
            "2n ≁ n should fail (f/g → 2), got: {:?} ({})",
            vr.status,
            vr.details
        );
    }
}
