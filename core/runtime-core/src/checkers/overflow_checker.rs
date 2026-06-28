//! OverflowChecker — detects overflow conditions in slide generation tasks.
//!
//! Checks for token limits, context window overflow, and output length violations
//! during slide/presentation generation.
//!
//! **Status: C-level stub** — the `check()` method always returns `passed: true`
//! with empty findings. Requires integration with a token counter (e.g.,
//! `tiktoken-rs` or similar) and slide parser to extract content metrics.
//!
//! **Implementation path:**
//! - Parse slide output (Markdown/LaTeX) to extract text blocks
//! - Count tokens using tiktoken or equivalent tokenizer
//! - Compare against configurable thresholds (context window, output length)
//! - Emit Warning/B-level findings when approaching or exceeding limits

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult};

pub struct OverflowChecker;

impl GateChecker for OverflowChecker {
    fn id(&self) -> &'static str {
        "overflow"
    }
    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::SLIDES]
    }
    fn description(&self) -> &'static str {
        "detect overflow conditions (token limits, context window, output length) in slide generation tasks"
    }
    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let _ = ctx; // unused while checker is a stub
        CheckResult {
            checker_id: "overflow".to_string(),
            passed: true,
            findings: Vec::new(),
        }
    }
}
