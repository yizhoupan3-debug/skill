//! OverflowChecker — detects overflow conditions in slide generation tasks.
//!
//! Checks for token limits, context window overflow, and output length violations
//! during slide/presentation generation.
//!
//! **Design:** lightweight checks only — no external tokenizer dependency.
//! Checks that output content exists, isn't suspiciously empty, and isn't
//! obviously oversized (would overflow an LLM context window).

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use std::fs;

pub struct OverflowChecker;

impl GateChecker for OverflowChecker {
    fn id(&self) -> &'static str {
        "overflow"
    }
    fn description(&self) -> &'static str {
        "detect overflow conditions (empty output, excessive size) in slide generation tasks"
    }
    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        // 1. Check output_data size if present
        if let Some(ref data) = ctx.output_data {
            let raw = serde_json::to_string(data).unwrap_or_default();
            let char_count = raw.chars().count();

            // Empty output check
            if char_count < 50 {
                findings.push(Finding {
                    id: "output-too-small".to_string(),
                    severity: Severity::Warning,
                    description: format!(
                        "slide output has only ~{char_count} characters — likely incomplete"
                    ),
                    location: None,
                    suggestion: Some("verify the slide content was fully generated".to_string()),
                });
            }

            // Oversized output check (~100k chars ≈ 25k CJK tokens ≈ context window)
            if char_count > 100_000 {
                findings.push(Finding {
                    id: "output-too-large".to_string(),
                    severity: Severity::B,
                    description: format!(
                        "slide output is ~{char_count} characters — may overflow LLM context window"
                    ),
                    location: None,
                    suggestion: Some(
                        "split into smaller sections or reduce content per slide".to_string(),
                    ),
                });
            }
        }

        // 2. Check evidence artifacts for missing output files
        if let Some(ref ev_path) = ctx.evidence_path {
            if ev_path.is_file() {
                if let Ok(content) = fs::read_to_string(ev_path) {
                    if content.trim().is_empty() {
                        findings.push(Finding {
                            id: "empty-evidence".to_string(),
                            severity: Severity::Warning,
                            description: "evidence index exists but is empty".to_string(),
                            location: Some(ev_path.to_string_lossy().to_string()),
                            suggestion: None,
                        });
                    }
                }
            } else {
                findings.push(Finding {
                    id: "missing-evidence".to_string(),
                    severity: Severity::C,
                    description: "no evidence index found — cannot verify output completeness"
                        .to_string(),
                    location: None,
                    suggestion: Some("complete the task and record evidence first".to_string()),
                });
            }
        }

        let passed = findings.is_empty()
            || findings
                .iter()
                .all(|f| !matches!(f.severity, Severity::P0 | Severity::A | Severity::B));

        CheckResult {
            checker_id: "overflow".to_string(),
            passed,
            findings,
        }
    }
}
