//! Lightweight proof trace recording for formal verification.
//!
//! Provides a `ProofTrace` structure that records:
//! - Proof steps (rule_applied, premise, conclusion)
//! - The backend used (SymPy, Z3, minilp, Lean, pure Rust)
//! - Verification time
//! - Assumptions used
//!
//! This module collects proof metadata that callers (including the auto_prover)
//! populate. It does not perform verification itself.

use serde::{Deserialize, Serialize};
use std::time::Instant;

// ===========================================================================
// Backend indicator
// ===========================================================================

/// Which backend was used for verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UsedBackend {
    /// Pure Rust symbolic engine (formerly SymPy)
    SymPy,
    /// Z3 SMT solver (pure Rust z3 crate)
    Z3,
    /// Pure Rust minilp solver
    Minilp,
    /// Lean theorem prover
    Lean,
    /// Pure Rust symbolic engine
    PureRust,
    /// No backend (fallback / error)
    None,
}

impl std::fmt::Display for UsedBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsedBackend::SymPy => write!(f, "SymPy"),
            UsedBackend::Z3 => write!(f, "Z3"),
            UsedBackend::Minilp => write!(f, "minilp"),
            UsedBackend::Lean => write!(f, "Lean"),
            UsedBackend::PureRust => write!(f, "pure_rust"),
            UsedBackend::None => write!(f, "none"),
        }
    }
}

// ===========================================================================
// ProofStep
// ===========================================================================

/// A single step in a proof trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofStep {
    /// The rule/transformation applied (e.g. "sympy_verify", "z3_prove", "expand", "simplify")
    pub rule_applied: String,
    /// The premise expression or state before the step
    pub premise: String,
    /// The conclusion expression or state after the step
    pub conclusion: String,
}

impl ProofStep {
    pub fn new(rule_applied: impl Into<String>, premise: impl Into<String>, conclusion: impl Into<String>) -> Self {
        Self {
            rule_applied: rule_applied.into(),
            premise: premise.into(),
            conclusion: conclusion.into(),
        }
    }
}

// ===========================================================================
// ProofTrace
// ===========================================================================

/// A complete proof trace recording verification metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofTrace {
    /// Ordered list of proof steps
    pub steps: Vec<ProofStep>,
    /// Backend that succeeded (the final verifier)
    pub backend: UsedBackend,
    /// Verification time in milliseconds
    pub verification_time_ms: u64,
    /// Assumptions used during verification
    pub assumptions: Vec<String>,
}

impl ProofTrace {
    /// Create a new empty proof trace with timing.
    pub fn new(backend: UsedBackend) -> Self {
        Self {
            steps: Vec::new(),
            backend,
            verification_time_ms: 0,
            assumptions: Vec::new(),
        }
    }

    /// Add a proof step to the trace.
    pub fn add_step(&mut self, step: ProofStep) {
        self.steps.push(step);
    }

    /// Add a step via its components.
    pub fn record_step(&mut self, rule_applied: impl Into<String>, premise: impl Into<String>, conclusion: impl Into<String>) {
        self.steps.push(ProofStep::new(rule_applied, premise, conclusion));
    }

    /// Add an assumption.
    pub fn add_assumption(&mut self, assumption: impl Into<String>) {
        self.assumptions.push(assumption.into());
    }

    /// Set verification time.
    pub fn set_time_ms(&mut self, ms: u64) {
        self.verification_time_ms = ms;
    }

    /// Return a human-readable summary of the proof trace.
    pub fn summary(&self) -> String {
        format!(
            "backend={}, {} steps, {}ms, {} assumption(s)",
            self.backend,
            self.steps.len(),
            self.verification_time_ms,
            self.assumptions.len(),
        )
    }

    /// Return a verbose multi-line description of the proof trace.
    pub fn describe(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Backend: {}", self.backend));
        lines.push(format!("Verification time: {}ms", self.verification_time_ms));
        if !self.assumptions.is_empty() {
            lines.push(format!("Assumptions: {}", self.assumptions.join(", ")));
        }
        lines.push(format!("Steps ({}):", self.steps.len()));
        for (i, step) in self.steps.iter().enumerate() {
            lines.push(format!(
                "  {}. [{}] {} → {}",
                i + 1,
                step.rule_applied,
                step.premise,
                step.conclusion
            ));
        }
        lines.join("\n")
    }
}

// ===========================================================================
// Utility: time a closure and return its result + trace
// ===========================================================================

/// Time a verification closure and record timing in the trace.
pub fn timed_verify<F>(mut trace: ProofTrace, f: F) -> (ProofTrace, crate::types::VerificationResult)
where
    F: FnOnce() -> crate::types::VerificationResult,
{
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed().as_millis() as u64;
    trace.set_time_ms(elapsed);
    (trace, result)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_proof_trace_new() {
        let trace = ProofTrace::new(UsedBackend::SymPy);
        assert_eq!(trace.backend, UsedBackend::SymPy);
        assert!(trace.steps.is_empty());
        assert_eq!(trace.verification_time_ms, 0);
    }

    #[test]
    fn test_proof_trace_add_step() {
        let mut trace = ProofTrace::new(UsedBackend::PureRust);
        trace.add_step(ProofStep::new("expand", "(x+1)^2", "x^2 + 2*x + 1"));
        trace.record_step("simplify", "x^2 + 2*x + 1", "x^2 + 2*x + 1");
        assert_eq!(trace.steps.len(), 2);
        assert_eq!(trace.steps[0].rule_applied, "expand");
        assert_eq!(trace.steps[1].rule_applied, "simplify");
    }

    #[test]
    fn test_proof_trace_summary() {
        let trace = ProofTrace::new(UsedBackend::Z3);
        let summary = trace.summary();
        assert!(summary.contains("Z3"));
        assert!(summary.contains("0 steps"));
    }

    #[test]
    fn test_proof_trace_describe() {
        let mut trace = ProofTrace::new(UsedBackend::SymPy);
        trace.set_time_ms(42);
        trace.add_assumption("x > 0");
        trace.record_step("verify", "x", "x");
        let desc = trace.describe();
        assert!(desc.contains("SymPy"));
        assert!(desc.contains("42ms"));
        assert!(desc.contains("x > 0"));
        assert!(desc.contains("verify"));
    }

    #[test]
    fn test_backend_display() {
        assert_eq!(UsedBackend::SymPy.to_string(), "SymPy");
        assert_eq!(UsedBackend::Z3.to_string(), "Z3");
        assert_eq!(UsedBackend::Minilp.to_string(), "minilp");
        assert_eq!(UsedBackend::Lean.to_string(), "Lean");
        assert_eq!(UsedBackend::PureRust.to_string(), "pure_rust");
        assert_eq!(UsedBackend::None.to_string(), "none");
    }

    #[test]
    fn test_timed_verify_records_time() {
        let trace = ProofTrace::new(UsedBackend::PureRust);
        let (new_trace, result) = timed_verify(trace, || {
            crate::types::VerificationResult {
                check_name: "test".into(),
                status: crate::types::VerificationStatus::Pass,
                details: "ok".into(),
                evidence_path: None,
            }
        });
        assert!(new_trace.verification_time_ms > 0);
        assert_eq!(result.status, crate::types::VerificationStatus::Pass);
    }

    #[test]
    fn test_timed_verify_with_fail_result() {
        let trace = ProofTrace::new(UsedBackend::Z3);
        let (new_trace, result) = timed_verify(trace, || {
            crate::types::VerificationResult {
                check_name: "fail_test".into(),
                status: crate::types::VerificationStatus::Fail,
                details: "expected failure".into(),
                evidence_path: None,
            }
        });
        assert_eq!(result.status, crate::types::VerificationStatus::Fail);
        assert!(new_trace.verification_time_ms > 0);
    }

    #[test]
    fn test_add_assumptions_multiple() {
        let mut trace = ProofTrace::new(UsedBackend::SymPy);
        trace.add_assumption("x > 0");
        trace.add_assumption("y > 0");
        trace.add_assumption("z = x + y");
        assert_eq!(trace.assumptions.len(), 3);
        let summary = trace.summary();
        assert!(summary.contains("3 assumption"));
    }

    #[test]
    fn test_describe_empty_trace() {
        let trace = ProofTrace::new(UsedBackend::None);
        let desc = trace.describe();
        assert!(desc.contains("Backend: none"));
        assert!(desc.contains("Steps (0)"));
    }

    #[test]
    fn test_proof_step_new_into_string() {
        let step = ProofStep::new("rule", "premise", "conclusion");
        assert_eq!(step.rule_applied, "rule");
        assert_eq!(step.premise, "premise");
        assert_eq!(step.conclusion, "conclusion");
    }

    #[test]
    fn test_set_time_ms_overwrites() {
        let mut trace = ProofTrace::new(UsedBackend::Minilp);
        trace.set_time_ms(100);
        assert_eq!(trace.verification_time_ms, 100);
        trace.set_time_ms(200);
        assert_eq!(trace.verification_time_ms, 200);
    }

    #[test]
    fn test_clone_and_serde_roundtrip() {
        let mut trace = ProofTrace::new(UsedBackend::Lean);
        trace.record_step("prove", "a = b", "a = b");
        trace.add_assumption("a = b");
        trace.set_time_ms(50);

        let cloned = trace.clone();
        assert_eq!(cloned.steps.len(), 1);
        assert_eq!(cloned.backend, UsedBackend::Lean);
        assert_eq!(cloned.verification_time_ms, 50);

        let json = serde_json::to_string(&trace).unwrap();
        let deserialized: ProofTrace = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.backend, UsedBackend::Lean);
        assert_eq!(deserialized.steps.len(), 1);
        assert_eq!(deserialized.assumptions.len(), 1);
        assert_eq!(deserialized.verification_time_ms, 50);
    }
}
