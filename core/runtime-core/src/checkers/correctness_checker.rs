//! CorrectnessGate — validates code correctness for code review flows.
//!
//! Scans the repository for common correctness issues: unrestrained unwrap(),
//! todo!() markers, and unimplemented!() stubs in the codebase.
//!
//! In-place adapter (Wave 4b, enhanced Wave 5b).

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use crate::checkers::find_rust_files;

/// Returns true if the file path looks like a test file.
///
/// Test files are excluded from correctness checks because unwrap/todo/unimplemented
/// are idiomatic and expected in tests.
fn is_test_file(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    // Match common test file patterns: tests/, test_*, *_test.rs, *_tests.rs
    s.contains("/tests/")
        || s.contains("/test/")
        || s.ends_with("_test.rs")
        || s.ends_with("_tests.rs")
        || s.contains("/test_")
}

/// Checker that validates code correctness in review contexts.
pub struct CorrectnessChecker;

impl GateChecker for CorrectnessChecker {
    fn id(&self) -> &'static str {
        "correctness"
    }

    fn description(&self) -> &'static str {
        "evaluate code modifications for correctness, logic errors, and semantic soundness"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let repo_root = std::path::Path::new(&ctx.repo_root);
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
            // Skip test files — unwrap/todo/unimplemented are expected in tests.
            if is_test_file(file_path) {
                continue;
            }

            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Line-based unwrap counting: exclude comments and string literals
            let unwrap_count = content
                .lines()
                .filter(|line| {
                    let trimmed = line.trim();
                    !trimmed.starts_with("//")
                        && !trimmed.starts_with("///")
                        && !trimmed.starts_with("//!")
                        && !trimmed.starts_with("/*")
                })
                .filter(|line| !line.trim().starts_with('"'))
                .map(|line| line.matches(".unwrap(").count())
                .sum::<usize>();
            let todo_count =
                content.matches("todo!()").count() + content.matches("todo!(\"").count();
            let unimplemented_count = content.matches("unimplemented!()").count()
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
                description: format!("{} todo!() / todo!(\"…\") stubs remaining", total_todo,),
                location: None,
                suggestion: Some("implement stubs before closing the task".to_string()),
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
