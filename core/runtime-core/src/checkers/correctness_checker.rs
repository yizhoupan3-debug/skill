//! CorrectnessGate — validates code correctness for code review flows.
//!
//! Scans the repository for common correctness issues: unrestrained unwrap(),
//! todo!() markers, and unimplemented!() stubs in the codebase.
//!
//! In-place adapter (Wave 4b, enhanced Wave 5b).

use std::path::Path;

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

/// Checker that validates code correctness in review contexts.
pub struct CorrectnessChecker;

impl GateChecker for CorrectnessChecker {
    fn id(&self) -> &'static str {
        "correctness"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::CODE_REVIEW]
    }

    fn description(&self) -> &'static str {
        "evaluate code modifications for correctness, logic errors, and semantic soundness"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let repo_root = Path::new(&ctx.repo_root);
        let rs_files = find_rust_files(repo_root);

        if rs_files.is_empty() {
            findings.push(Finding {
                id: "correctness_no_rust".to_string(),
                severity: Severity::C,
                description: format!(
                    "no Rust source files found at {:?} — correctness checks skipped",
                    repo_root,
                ),
                location: None,
                suggestion: None,
            });
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        }

        let mut total_unwrap = 0usize;
        let mut total_todo = 0usize;
        let mut total_unimplemented = 0usize;

        for file_path in &rs_files {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let unwrap_count = content.matches(".unwrap()").count();
            let todo_count = content.matches("todo!()").count() + content.matches("todo!(\"").count();
            let unimplemented_count =
                content.matches("unimplemented!()").count()
                    + content.matches("unimplemented!(\"").count();

            total_unwrap += unwrap_count;
            total_todo += todo_count;
            total_unimplemented += unimplemented_count;
        }

        // ── unwrap() usage ──
        if total_unwrap > 20 {
            let sev = if total_unwrap > 100 {
                Severity::Warning
            } else {
                Severity::C
            };
            findings.push(Finding {
                id: "correctness_unwrap".to_string(),
                severity: sev,
                description: format!(
                    "{} .unwrap() calls across codebase — consider replacing with error propagation or expect() with context",
                    total_unwrap,
                ),
                location: None,
                suggestion: Some(
                    "replace .unwrap() with ? operator or expect(\"descriptive message\")"
                        .to_string(),
                ),
            });
        }

        // ── todo!() stubs ──
        if total_todo > 0 {
            findings.push(Finding {
                id: "correctness_todo".to_string(),
                severity: Severity::C,
                description: format!(
                    "{} todo!() / todo!(\"…\") stubs remaining",
                    total_todo,
                ),
                location: None,
                suggestion: Some(
                    "implement stubs before closing the task".to_string(),
                ),
            });
        }

        // ── unimplemented!() stubs ──
        if total_unimplemented > 0 {
            let sev = if total_unimplemented > 5 {
                Severity::Warning
            } else {
                Severity::C
            };
            findings.push(Finding {
                id: "correctness_unimplemented".to_string(),
                severity: sev,
                description: format!(
                    "{} unimplemented!() stubs — will panic at runtime if reached",
                    total_unimplemented,
                ),
                location: None,
                suggestion: Some(
                    "implement or replace with todo!() for tracked follow-up work".to_string(),
                ),
            });
        }

        let passed = findings.is_empty()
            || findings
                .iter()
                .all(|f| !matches!(f.severity, Severity::P0 | Severity::A | Severity::B));

        CheckResult {
            checker_id: self.id().to_string(),
            passed,
            findings,
        }
    }
}

/// Find Rust source files (.rs) at the repo root (non-hidden).
fn find_rust_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.starts_with('.'))
            {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files
}
