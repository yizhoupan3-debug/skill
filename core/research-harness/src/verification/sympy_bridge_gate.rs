//! QG Route `GateChecker` adapter for `sympy_bridge`.
//!
//! Extracts expression data from `CheckContext::output_data` and calls the
//! SymPy bridge for symbolic verification.
//!
//! Expected `output_data` JSON (all fields optional):
//! ```json
//! {
//!   "identity": { "lhs": "sin(x)^2 + cos(x)^2", "rhs": "1" },
//!   "simplify": "x^2 + 2*x + 1"
//! }
//! ```

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use crate::verification::sympy_bridge;
use crate::types::VerificationStatus;

pub struct SympyBridge;

impl GateChecker for SympyBridge {
    fn id(&self) -> &'static str {
        "sympy_bridge"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "SymPy symbolic verification: identity check, expression simplification"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        if !sympy_bridge::sympy_available() {
            findings.push(Finding {
                id: "sympy_unavailable".to_string(),
                severity: Severity::C,
                description: "SymPy is not available — bridge checks degraded".to_string(),
                location: None,
                suggestion: Some("install sympy and a Python environment to enable full checks".to_string()),
            });
            return CheckResult { checker_id: self.id().to_string(), passed: true, findings };
        }

        let Some(data) = ctx.output_data.as_ref() else {
            findings.push(Finding {
                id: "sympy_no_data".to_string(),
                severity: Severity::C,
                description: "No output_data provided — SymPy checks skipped".to_string(),
                location: None,
                suggestion: Some("pass output_data with sympy keys to enable checks".to_string()),
            });
            return CheckResult { checker_id: self.id().to_string(), passed: true, findings };
        };

        // Identity verification via SymPy
        if let Some(id) = data.get("identity") {
            let lhs = id.get("lhs").and_then(|v| v.as_str()).unwrap_or("");
            let rhs = id.get("rhs").and_then(|v| v.as_str()).unwrap_or("");
            let vr = sympy_bridge::verify_identity(lhs, rhs);
            let severity = match vr.status {
                VerificationStatus::Pass => Severity::C,
                VerificationStatus::Fail => Severity::B,
                VerificationStatus::Warn => Severity::Warning,
                VerificationStatus::Skip => Severity::C,
            };
            findings.push(Finding {
                id: "sympy_identity".to_string(),
                severity,
                description: format!("SymPy identity '{lhs}' = '{rhs}': {}", vr.details),
                location: None,
                suggestion: if matches!(vr.status, VerificationStatus::Fail) {
                    Some("identity not verified by SymPy — check expressions".to_string())
                } else { None },
            });
        }

        // Simplification check via SymPy
        if let Some(expr_val) = data.get("simplify").and_then(|v| v.as_str()) {
            let vr = sympy_bridge::simplify_expression(expr_val);
            let severity = match vr.status {
                VerificationStatus::Pass => Severity::C,
                VerificationStatus::Fail => Severity::Warning,
                VerificationStatus::Warn => Severity::Warning,
                VerificationStatus::Skip => Severity::C,
            };
            findings.push(Finding {
                id: "sympy_simplify".to_string(),
                severity,
                description: format!("SymPy simplify '{expr_val}': {}", vr.details),
                location: None,
                suggestion: None,
            });
        }

        let passed = findings.iter().all(|f| matches!(f.severity, Severity::C | Severity::Warning));
        CheckResult { checker_id: self.id().to_string(), passed, findings }
    }
}
