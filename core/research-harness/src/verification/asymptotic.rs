//! Asymptotic / Order-of-Magnitude analysis — ResearchHarness feature layer.
//!
//! Pure business logic: asymptotic relation types, chain verification,
//! magnitude estimation via SymPy subprocess. No MCP dispatch or JSON
//! argument extraction — those belong in `mcp_tools.rs`.
//!
//! # Layer boundary
//!
//! FEATURE layer only. Tool dispatch (JSON → feature → JSON) is in
//! `mcp_tools.rs`.

use crate::types::{VerificationResult, VerificationStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    pub fn is_empty(&self) -> bool { self.steps.is_empty() }

    /// If all steps use the same relation, the chain is "pure".
    pub fn is_pure(&self) -> bool {
        if self.steps.len() < 2 { return true; }
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
    let unique_rel: Vec<String> = chain.unique_relations().iter().map(|r| r.symbol().to_string()).collect();

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
// Subprocess bridge to SymPy asymptotic solver
// ===========================================================================

fn call_asymptotic_subprocess(input: &serde_json::Value) -> Result<serde_json::Value, String> {
    crate::subprocess::run_uv_module("asymptotic_solver", input)
}

// ===========================================================================
// Magnitude estimation
// ===========================================================================

/// Estimate the leading-order term of an expression in a given regime (e.g. n→∞).
pub fn magnitude_estimate(expr: &str, var: &str, regime: &str) -> VerificationResult {
    magnitude_estimate_with_name(expr, var, regime, "math_asymptotic_estimate")
}

/// Like `magnitude_estimate` with an explicit check name.
pub fn magnitude_estimate_with_name(expr: &str, var: &str, regime: &str, check_name: &str) -> VerificationResult {
    let input = serde_json::json!({
        "command": "estimate",
        "expr": expr,
        "var": var,
        "point": regime,
        "n_terms": 3,
    });

    match call_asymptotic_subprocess(&input) {
        Ok(resp) => {
            if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
                return VerificationResult {
                    check_name: check_name.to_string(),
                    status: VerificationStatus::Warn,
                    details: format!("estimate error: {err}"),
                    evidence_path: None,
                };
            }
            let leading = resp.get("leading_term").and_then(|v| v.as_str()).unwrap_or("?");
            let order = resp.get("order").and_then(|v| v.as_str()).unwrap_or("?");
            VerificationResult {
                check_name: check_name.to_string(),
                status: VerificationStatus::Pass,
                details: format!("{expr} ~ {leading} (order: {order}) as {var}→{regime}"),
                evidence_path: None,
            }
        }
        Err(e) => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Warn,
            details: format!("subprocess: {e}"),
            evidence_path: None,
        },
    }
}

// ===========================================================================
// Asymptotic claim verification
// ===========================================================================

/// Verify an asymptotic claim: f relation g in a given regime.
pub fn check_asymptotic_claim(f: &str, g: &str, relation: &OrderRelation, var: &str, regime: &str) -> VerificationResult {
    check_asymptotic_claim_with_name(f, g, relation, var, regime, "math_asymptotic_claim")
}

/// Like `check_asymptotic_claim` with an explicit check name.
pub fn check_asymptotic_claim_with_name(
    f: &str, g: &str, relation: &OrderRelation, var: &str, regime: &str, check_name: &str,
) -> VerificationResult {
    let relation_str = match relation {
        OrderRelation::LessSim => "LessSim",
        OrderRelation::MuchLess => "MuchLess",
        OrderRelation::Asymp => "Asymp",
    };

    let input = serde_json::json!({
        "command": "check_claim",
        "f": f,
        "g": g,
        "relation": relation_str,
        "var": var,
        "point": regime,
    });

    match call_asymptotic_subprocess(&input) {
        Ok(resp) => {
            if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
                return VerificationResult {
                    check_name: check_name.to_string(),
                    status: VerificationStatus::Warn,
                    details: format!("check error: {err}"),
                    evidence_path: None,
                };
            }
            let feasible = resp.get("feasible").and_then(|v| v.as_bool()).unwrap_or(false);
            let reason = resp.get("reason").and_then(|v| v.as_str()).unwrap_or("");
            let symbol = relation.symbol();

            if feasible {
                VerificationResult {
                    check_name: check_name.to_string(),
                    status: VerificationStatus::Pass,
                    details: format!("{f} {symbol} {g} holds as {var}→{regime}: {reason}"),
                    evidence_path: None,
                }
            } else {
                VerificationResult {
                    check_name: check_name.to_string(),
                    status: VerificationStatus::Fail,
                    details: format!("{f} {symbol} {g} does NOT hold as {var}→{regime}: {reason}"),
                    evidence_path: None,
                }
            }
        }
        Err(e) => VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Warn,
            details: format!("subprocess: {e}"),
            evidence_path: None,
        },
    }
}

/// Verify an entire asymptotic chain.
/// Pure chains → auto PASS; mixed chains → auto WARN + human review.
pub fn verify_asymptotic_chain(steps: &[AsymptoticStep], var: &str, regime: &str, sympy_check: bool) -> VerificationResult {
    verify_asymptotic_chain_with_name(steps, var, regime, sympy_check, "math_asymptotic_chain")
}

/// Like `verify_asymptotic_chain` with an explicit check name.
pub fn verify_asymptotic_chain_with_name(steps: &[AsymptoticStep], var: &str, regime: &str, sympy_check: bool, check_name: &str) -> VerificationResult {
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

    // If SymPy check is requested, verify each step
    if sympy_check && chain.steps.len() <= 10 {
        let mut step_details = Vec::new();
        let mut all_pass = true;

        for (i, step) in steps.iter().enumerate() {
            let vr = check_asymptotic_claim_with_name(
                &step.premise, &step.conclusion,
                &step.relation, var, regime, check_name,
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
                details: format!("Chain verified ({} steps): {}", steps.len(), step_details.join("; ")),
                evidence_path: None,
            }
        } else {
            VerificationResult {
                check_name: check_name.to_string(),
                status: VerificationStatus::Fail,
                details: format!("Chain verification FAILED ({} steps): {}", steps.len(), step_details.join("; ")),
                evidence_path: None,
            }
        }
    } else if chain.steps.len() > 10 {
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Warn,
            details: format!("chain too long ({} steps) for SymPy verification, manual review", chain.steps.len()),
            evidence_path: None,
        }
    } else {
        // Pure chain, no SymPy check — structural PASS
        VerificationResult {
            check_name: check_name.to_string(),
            status: VerificationStatus::Pass,
            details: format!("Pure chain ({} steps): all {} relations are consistent",
                steps.len(), chain.unique_relations.join(", ")),
            evidence_path: None,
        }
    }
}

// ===========================================================================
// Backend availability
// ===========================================================================

pub fn sympy_available() -> bool {
    crate::verification::sympy_available()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_chain_detection() {
        let steps = vec![
            AsymptoticStep {
                premise: "n".into(), relation: OrderRelation::LessSim,
                conclusion: "n^2".into(), justification: "poly".into(),
            },
            AsymptoticStep {
                premise: "n^2".into(), relation: OrderRelation::LessSim,
                conclusion: "n^3".into(), justification: "poly".into(),
            },
        ];
        let chain = AsymptoticChain::new(steps);
        assert!(chain.is_pure());
    }

    #[test]
    fn test_mixed_chain_detection() {
        let steps = vec![
            AsymptoticStep {
                premise: "n".into(), relation: OrderRelation::LessSim,
                conclusion: "n^2".into(), justification: "".into(),
            },
            AsymptoticStep {
                premise: "n^2".into(), relation: OrderRelation::MuchLess,
                conclusion: "2^n".into(), justification: "".into(),
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
                premise: "a".into(), relation: OrderRelation::LessSim,
                conclusion: "b".into(), justification: "".into(),
            },
            AsymptoticStep {
                premise: "b".into(), relation: OrderRelation::LessSim,
                conclusion: "c".into(), justification: "".into(),
            },
        ];
        let chain = AsymptoticChain::new(steps);
        let rels = chain.unique_relations();
        assert_eq!(rels.len(), 1);
    }

    #[test]
    fn test_sympy_backend_probe() {
        let _ = sympy_available();
    }
}
