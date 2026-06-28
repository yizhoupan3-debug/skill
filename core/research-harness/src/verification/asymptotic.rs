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

/// Estimate the leading-order term of an expression in a given regime (e.g. n→∞).
pub fn magnitude_estimate(expr: &str, var: &str, regime: &str) -> VerificationResult {
    magnitude_estimate_with_name(expr, var, regime, "math_asymptotic_estimate")
}

/// Like `magnitude_estimate` with an explicit check name.
pub fn magnitude_estimate_with_name(
    expr: &str,
    var: &str,
    _regime: &str,
    check_name: &str,
) -> VerificationResult {
    match crate::verification::symbolic::leading_term(expr, var) {
        Ok((leading, order)) => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Pass,
            details: format!("{expr} ~ {leading} (order: {order}) as {var}→∞"),
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

// ===========================================================================
// Asymptotic claim verification (pure Rust)
// ===========================================================================

/// Verify an asymptotic claim: f relation g in a given regime.
pub fn check_asymptotic_claim(
    f: &str,
    g: &str,
    relation: &OrderRelation,
    var: &str,
    _regime: &str,
) -> VerificationResult {
    check_asymptotic_claim_with_name(f, g, relation, var, _regime, "math_asymptotic_claim")
}

/// Like `check_asymptotic_claim` with an explicit check name.
pub fn check_asymptotic_claim_with_name(
    f: &str,
    g: &str,
    relation: &OrderRelation,
    var: &str,
    _regime: &str,
    check_name: &str,
) -> VerificationResult {
    let f_expr = match crate::verification::symbolic::parse(f) {
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
    let g_expr = match crate::verification::symbolic::parse(g) {
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
            details: format!("{f} {symbol} {g} holds as {var}→∞"),
            evidence_path: None,
        }
    } else {
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Fail,
            details: format!("{f} {symbol} {g} does NOT hold as {var}→∞"),
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
    _regime: &str,
    _sympy_check: bool,
) -> VerificationResult {
    verify_asymptotic_chain_with_name(steps, var, _regime, _sympy_check, "math_asymptotic_chain")
}

/// Like `verify_asymptotic_chain` with an explicit check name.
pub fn verify_asymptotic_chain_with_name(
    steps: &[AsymptoticStep],
    var: &str,
    _regime: &str,
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
            _regime,
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
}
