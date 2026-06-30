//! QG Route `GateChecker` adapter for the symbolic verification module.
//!
//! Extracts expression pairs from `CheckContext::output_data` and calls
//! the underlying pure functions in `verification::symbolic`.
//!
//! Expected `output_data` JSON (all fields optional):
//! ```json
//! {
//!   "identity": { "lhs": "sin(x)^2 + cos(x)^2", "rhs": "1" },
//!   "equivalent": { "lhs": "x^2 + 2*x + 1", "rhs": "(x+1)^2" },
//!   "growth": { "expr": "x^3 + 2*x", "var": "x" },
//!   "compare_growth": { "f": "x^2", "g": "x^3", "var": "x" }
//! }
//! ```

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use crate::verification::symbolic;

pub struct Symbolic;

impl GateChecker for Symbolic {
    fn id(&self) -> &'static str {
        "symbolic"
    }

    fn description(&self) -> &'static str {
        "symbolic verification: identity proving, equivalence, growth classification"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let Some(data) = ctx.output_data.as_ref() else {
            findings.push(Finding {
                id: "symbolic_no_data".to_string(),
                severity: Severity::C,
                description: "No output_data provided — symbolic checks skipped".to_string(),
                location: None,
                suggestion: Some(
                    "pass output_data with symbolic keys to enable checks".to_string(),
                ),
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        };

        // Identity verification (prove lhs == rhs via algebraic rewrite)
        if let Some(id) = data.get("identity") {
            let lhs = id.get("lhs").and_then(|v| v.as_str()).unwrap_or("");
            let rhs = id.get("rhs").and_then(|v| v.as_str()).unwrap_or("");
            let (proved, proof) = symbolic::verify_identity(lhs, rhs);
            findings.push(Finding {
                id: "symbolic_identity".to_string(),
                severity: if proved { Severity::C } else { Severity::B },
                description: format!("Identity '{lhs}' = '{rhs}': {proof}"),
                location: None,
                suggestion: if proved {
                    None
                } else {
                    Some("identity could not be proved — check expressions".to_string())
                },
            });
        }

        // Equivalence check
        if let Some(eq) = data.get("equivalent") {
            let lhs = eq.get("lhs").and_then(|v| v.as_str()).unwrap_or("");
            let rhs = eq.get("rhs").and_then(|v| v.as_str()).unwrap_or("");
            let equiv = symbolic::equivalent(lhs, rhs);
            findings.push(Finding {
                id: "symbolic_equivalent".to_string(),
                severity: if equiv { Severity::C } else { Severity::B },
                description: format!(
                    "Equivalence '{lhs}' ≡ '{rhs}': {}",
                    if equiv {
                        "proved equivalent"
                    } else {
                        "not equivalent"
                    }
                ),
                location: None,
                suggestion: if equiv {
                    None
                } else {
                    Some("expressions are not algebraically equivalent".to_string())
                },
            });
        }

        // Growth classification
        if let Some(gr) = data.get("growth") {
            let expr_str = gr.get("expr").and_then(|v| v.as_str()).unwrap_or("");
            let var = gr.get("var").and_then(|v| v.as_str()).unwrap_or("x");
            match symbolic::parse(expr_str) {
                Ok(expr) => {
                    let growth = symbolic::classify_growth(&expr, var);
                    findings.push(Finding {
                        id: "symbolic_growth".to_string(),
                        severity: Severity::C,
                        description: format!("Growth class of '{expr_str}' in {var}: {growth:?}"),
                        location: None,
                        suggestion: None,
                    });
                }
                Err(e) => {
                    findings.push(Finding {
                        id: "symbolic_growth_parse_error".to_string(),
                        severity: Severity::C,
                        description: format!("Could not parse '{expr_str}': {e}"),
                        location: None,
                        suggestion: None,
                    });
                }
            }
        }

        // Compare growth of two expressions
        if let Some(cg) = data.get("compare_growth") {
            let f = cg.get("f").and_then(|v| v.as_str()).unwrap_or("");
            let g = cg.get("g").and_then(|v| v.as_str()).unwrap_or("");
            let var = cg.get("var").and_then(|v| v.as_str()).unwrap_or("x");
            match symbolic::compare_growth(f, g, var) {
                Ok(rel) => {
                    findings.push(Finding {
                        id: "symbolic_compare_growth".to_string(),
                        severity: Severity::C,
                        description: format!("Growth comparison: {f} vs {g} in {var}: {rel:?}"),
                        location: None,
                        suggestion: None,
                    });
                }
                Err(e) => {
                    findings.push(Finding {
                        id: "symbolic_compare_growth_error".to_string(),
                        severity: Severity::C,
                        description: format!("Growth comparison error for {f} vs {g}: {e}"),
                        location: None,
                        suggestion: None,
                    });
                }
            }
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
