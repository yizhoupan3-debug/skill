//! QG Route `GateChecker` adapter for the `asymptotic` module.
//!
//! Extracts asymptotic chain/claim/magnitude data from `CheckContext::output_data`
//! and calls the underlying pure functions in `verification::asymptotic`.
//!
//! Expected `output_data` JSON (all fields optional):
//! ```json
//! {
//!   "magnitude_estimate": { "expr": "n^2 + n", "var": "n", "regime": "oo" },
//!   "chain": {
//!     "steps": [{"premise":"n^2", "relation":"much_less", "conclusion":"n^3", "justification":"..."}],
//!     "var": "n", "regime": "oo", "sympy_check": false
//!   },
//!   "claim": { "f": "n^2", "g": "n^3", "relation": "much_less", "var": "n", "regime": "oo" }
//! }
//! ```

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use crate::types::{VerificationResult, VerificationStatus};
use crate::verification::asymptotic::{self, AsymptoticStep, OrderRelation};

fn vr_to_finding(vr: &VerificationResult, id_prefix: &str) -> Finding {
    let severity = match vr.status {
        VerificationStatus::Pass => Severity::C,
        VerificationStatus::Fail => Severity::B,
        VerificationStatus::Warn => Severity::Warning,
        VerificationStatus::Skip => Severity::C,
    };
    Finding {
        id: format!("{id_prefix}_{}", vr.check_name),
        severity,
        description: vr.details.clone(),
        location: None,
        suggestion: if matches!(vr.status, VerificationStatus::Fail) {
            Some(
                "asymptotic claim failed verification — review expressions and relation"
                    .to_string(),
            )
        } else {
            None
        },
    }
}

fn parse_order_relation(s: &str) -> OrderRelation {
    match s.to_lowercase().as_str() {
        "much_less" | "<<" => OrderRelation::MuchLess,
        "less_sim" | "lesssim" | "≲" => OrderRelation::LessSim,
        "asymp" | "≈" | "~" | "equivalent" => OrderRelation::Asymp,
        _ => OrderRelation::Asymp,
    }
}

pub struct Asymptotic;

impl GateChecker for Asymptotic {
    fn id(&self) -> &'static str {
        "asymptotic"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "asymptotic analysis: chain composition, magnitude estimation, claim verification"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let Some(data) = ctx.output_data.as_ref() else {
            findings.push(Finding {
                id: "asymptotic_no_data".to_string(),
                severity: Severity::C,
                description: "No output_data provided — asymptotic checks skipped".to_string(),
                location: None,
                suggestion: Some(
                    "pass output_data with asymptotic keys to enable checks".to_string(),
                ),
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        };

        // Magnitude estimate
        if let Some(mag) = data.get("magnitude_estimate") {
            let expr = mag.get("expr").and_then(|v| v.as_str()).unwrap_or("");
            let var = mag.get("var").and_then(|v| v.as_str()).unwrap_or("x");
            let regime = mag.get("regime").and_then(|v| v.as_str()).unwrap_or("oo");
            let vr = asymptotic::magnitude_estimate(expr, var, regime);
            findings.push(vr_to_finding(&vr, "asymptotic_mag"));
        }

        // Asymptotic chain verification
        if let Some(chain_data) = data.get("chain") {
            let var = chain_data
                .get("var")
                .and_then(|v| v.as_str())
                .unwrap_or("x");
            let regime = chain_data
                .get("regime")
                .and_then(|v| v.as_str())
                .unwrap_or("oo");
            let sympy_check = chain_data
                .get("sympy_check")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if let Some(steps_arr) = chain_data.get("steps").and_then(|v| v.as_array()) {
                let steps: Vec<AsymptoticStep> = steps_arr
                    .iter()
                    .filter_map(|s| {
                        let premise = s.get("premise")?.as_str()?;
                        let conclusion = s.get("conclusion")?.as_str()?;
                        let rel = s
                            .get("relation")
                            .map(|r| r.as_str().unwrap_or("asymp"))
                            .unwrap_or("asymp");
                        let justification = s
                            .get("justification")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        Some(AsymptoticStep {
                            premise: premise.to_string(),
                            relation: parse_order_relation(rel),
                            conclusion: conclusion.to_string(),
                            justification: justification.to_string(),
                        })
                    })
                    .collect();
                if !steps.is_empty() {
                    let vr = asymptotic::verify_asymptotic_chain(&steps, var, regime, sympy_check);
                    findings.push(vr_to_finding(&vr, "asymptotic_chain"));
                }
            }
        }

        // Single asymptotic claim
        if let Some(claim) = data.get("claim") {
            let f = claim.get("f").and_then(|v| v.as_str()).unwrap_or("");
            let g = claim.get("g").and_then(|v| v.as_str()).unwrap_or("");
            let relation = claim
                .get("relation")
                .and_then(|v| v.as_str())
                .unwrap_or("asymp");
            let var = claim.get("var").and_then(|v| v.as_str()).unwrap_or("x");
            let regime = claim.get("regime").and_then(|v| v.as_str()).unwrap_or("oo");
            let vr = asymptotic::check_asymptotic_claim(
                f,
                g,
                &parse_order_relation(relation),
                var,
                regime,
            );
            findings.push(vr_to_finding(&vr, "asymptotic_claim"));
        }

        let passed = findings
            .iter()
            .all(|f| matches!(f.severity, Severity::C | Severity::Warning));
        CheckResult {
            checker_id: self.id().to_string(),
            passed,
            findings,
        }
    }
}
