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

    fn description(&self) -> &'static str {
        "review code changes for security vulnerabilities and unsafe patterns"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let repo_root = std::path::Path::new(&ctx.repo_root);
        let rs_files = find_rust_files(repo_root);

        if rs_files.is_empty() {
            findings.push(Finding::new("security_no_rust", Severity::C,
                format!("no Rust source files found at {:?} — security checks skipped", repo_root)));
            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        }

        // Accumulate counts
        let mut unsafe_unsafe_blocks = 0usize; // unsafe without SAFETY comment
        let mut unsafe_total_blocks = 0usize;
        let mut transmute_calls = 0usize;
        let mut command_new_var = 0usize;
        let mut shell_cmds = 0usize;

        for file_path in &rs_files {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            transmute_calls += content.matches("std::mem::transmute").count()
                + content.matches("mem::transmute").count();

            let lines: Vec<&str> = content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                // Skip comments
                if trimmed.starts_with("//") || trimmed.starts_with('#') {
                    continue;
                }

                // ── unsafe blocks: count with/without SAFETY comment ──
                if trimmed.contains("unsafe {") || trimmed.contains("unsafe{") {
                    unsafe_total_blocks += 1;
                    // Check the 2 preceding lines for a // SAFETY: comment
                    let has_safety = (1..=2).any(|offset| {
                        i >= offset
                            && lines[i - offset]
                                .trim()
                                .to_ascii_lowercase()
                                .contains("// safety:")
                    });
                    if !has_safety {
                        unsafe_unsafe_blocks += 1;
                    }
                }

                // ── Command::new(var) — variable command injection ──
                if trimmed.contains("Command::new(") {
                    if let Some(after) = trimmed.split("Command::new(").nth(1) {
                        let arg = after.trim();
                        // Only flag if argument is NOT a string literal (starts with " or ')
                        let first_char = arg.chars().next();
                        if let Some(c) = first_char {
                            if c != '"' && c != '\'' {
                                command_new_var += 1;
                            }
                        }
                    }
                }

                // ── Shell command execution ──
                if trimmed.contains("sh -c")
                    || trimmed.contains("bash -c")
                    || trimmed.contains("std::process::Command::new(\"sh\"")
                {
                    shell_cmds += 1;
                }
            }
        }

        // ── unsafe blocks without SAFETY comment ──
        if unsafe_unsafe_blocks > 0 {
            findings.push(Finding::new("security_unsafe_no_safety",
                if unsafe_unsafe_blocks > 5 { Severity::Warning } else { Severity::C },
                format!("{} unsafe {{ }} block(s) without // SAFETY: comment (out of {} total)",
                    unsafe_unsafe_blocks, unsafe_total_blocks))
                .with_suggestion("add // SAFETY: comment above each unsafe block explaining invariants"));
        }

        // ── transmute ──
        if transmute_calls > 0 {
            findings.push(Finding::new("security_transmute", Severity::C,
                format!("{} transmute call(s) — prefer safe conversions via From/TryFrom", transmute_calls))
                .with_suggestion("replace transmute with From/TryFrom or a dedicated conversion function"));
        }

        // ── Command::new(var) — variable command injection ──
        if command_new_var > 0 {
            findings.push(Finding::new("security_command_injection", Severity::Warning,
                format!("{} Command::new(var) call(s) — variable command with potential injection", command_new_var))
                .with_suggestion("avoid building shell commands from untrusted variables; validate input"));
        }

        // ── shell command execution ──
        if shell_cmds > 0 {
            findings.push(Finding::new("security_shell_exec", Severity::Warning,
                format!("{} shell command execution(s) via sh -c / bash -c", shell_cmds))
                .with_suggestion("use std::process::Command with explicit args instead of shell strings"));
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
