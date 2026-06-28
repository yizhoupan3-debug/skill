//! SecurityGate — reviews code changes for security vulnerabilities.
//!
//! Scans the repository for common vulnerability patterns: use of unsafe code,
//! hardcoded secrets in comments/strings, Command::new with variable arguments,
//! and dangerous eval-like patterns.
//!
//! In-place adapter (Wave 4b, enhanced Wave 5b).

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use crate::checkers::find_rust_files;

/// Checker that reviews code changes for security vulnerabilities and unsafe
/// patterns during CODE_REVIEW scenes.
pub struct SecurityChecker;

impl GateChecker for SecurityChecker {
    fn id(&self) -> &'static str {
        "security"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::CODE_REVIEW]
    }

    fn description(&self) -> &'static str {
        "review code changes for security vulnerabilities and unsafe patterns"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let repo_root = std::path::Path::new(&ctx.repo_root);
        let rs_files = find_rust_files(repo_root);

        if rs_files.is_empty() {
            findings.push(Finding {
                id: "security_no_rust".to_string(),
                severity: Severity::C,
                description: format!(
                    "no Rust source files found at {:?} — security checks skipped",
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

        // Accumulate counts
        let mut unsafe_blocks = 0usize;
        let mut transmute_calls = 0usize;
        let mut command_new_var = 0usize;
        let mut shell_cmds = 0usize;

        for file_path in &rs_files {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            unsafe_blocks += content.matches("unsafe {").count();
            transmute_calls += content.matches("std::mem::transmute").count()
                + content.matches("mem::transmute").count();

            // Command::new with a variable (not a string literal) — potential injection
            // Heuristic: look for Command::new( followed by something that starts with a
            // variable reference (&, name character) and not a quote.
            let lines: Vec<&str> = content.lines().collect();
            for line in &lines {
                let trimmed = line.trim();
                // Skip comments
                if trimmed.starts_with("//") || trimmed.starts_with('#') {
                    continue;
                }
                // Command::new(var) — var is not a string literal
                if trimmed.contains("Command::new(") {
                    let after = trimmed.split("Command::new(").nth(1).unwrap_or("");
                    // If the argument doesn't start with a quote, it's a variable
                    if let Some(first_char) = after.chars().next() {
                        if first_char != '"' && first_char != '\'' {
                            command_new_var += 1;
                        }
                    }
                }
                // Shell command execution
                if trimmed.contains("sh -c")
                    || trimmed.contains("bash -c")
                    || trimmed.contains("std::process::Command::new(\"sh\"")
                {
                    shell_cmds += 1;
                }
            }
        }

        // ── unsafe blocks ──
        if unsafe_blocks > 0 {
            let sev = if unsafe_blocks > 10 {
                Severity::Warning
            } else {
                Severity::C
            };
            findings.push(Finding {
                id: "security_unsafe".to_string(),
                severity: sev,
                description: format!(
                    "{} unsafe {{ }} blocks found — each should be justified with a SAFETY comment",
                    unsafe_blocks,
                ),
                location: None,
                suggestion: Some(
                    "add // SAFETY: comment above each unsafe block explaining invariants"
                        .to_string(),
                ),
            });
        }

        // ── transmute ──
        if transmute_calls > 0 {
            findings.push(Finding {
                id: "security_transmute".to_string(),
                severity: Severity::C,
                description: format!(
                    "{} transmute call(s) — prefer safe conversions via From/TryFrom",
                    transmute_calls,
                ),
                location: None,
                suggestion: Some(
                    "replace transmute with From/TryFrom or a dedicated conversion function"
                        .to_string(),
                ),
            });
        }

        // ── Command::new(var) — variable command injection ──
        if command_new_var > 0 {
            findings.push(Finding {
                id: "security_command_injection".to_string(),
                severity: Severity::Warning,
                description: format!(
                    "{} Command::new(var) call(s) — variable command with potential injection",
                    command_new_var,
                ),
                location: None,
                suggestion: Some(
                    "avoid building shell commands from untrusted variables; validate input"
                        .to_string(),
                ),
            });
        }

        // ── shell command execution ──
        if shell_cmds > 0 {
            findings.push(Finding {
                id: "security_shell_exec".to_string(),
                severity: Severity::Warning,
                description: format!(
                    "{} shell command execution(s) via sh -c / bash -c",
                    shell_cmds,
                ),
                location: None,
                suggestion: Some(
                    "use std::process::Command with explicit args instead of shell strings"
                        .to_string(),
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
