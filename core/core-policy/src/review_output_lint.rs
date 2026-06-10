//! Post-hoc validation of code-review-deep compact envelope output format.
//!
//! Checks that review output follows the **Compact envelope** rules from
//! `skills/code-review-deep/SKILL.md`:
//!
//! - First non-blank line is `[P0]`, `[P1]`, `[P2]`, `Caveat:`, `Scope:`, or `Out of scope:`
//! - No Markdown tables before the first `[P*]` or `Caveat:` line
//! - Verdict (P0–P2 + "blocked|revise|ship" line) appears after findings, not before
//!
//! All checks are **advisory** (non-blocking). Violations produce a `LintFinding`.

use regex::Regex;
use std::sync::LazyLock;

/// Severity of a lint finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Warning,
    Info,
}

/// A single lint finding about review output formatting.
#[derive(Debug, Clone)]
pub struct LintFinding {
    pub severity: LintSeverity,
    pub message: String,
}

static TABLE_ROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\|.*\|.*\|").expect("table row regex"));

static SEVERITY_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[P[012]\]").expect("severity prefix regex"));

static CAVEAT_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*Caveat:").expect("caveat prefix regex"));

static SCOPE_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(Scope:|Out of scope:)").expect("scope prefix regex"));

const SUBSTANTIVE_FINDING_BODY_MIN_LEN: usize = 12;

fn remainder_after_compact_finding_prefix(line: &str) -> &str {
    let t = line.trim();
    if let Some(m) = SEVERITY_PREFIX.find(t) {
        return t[m.end()..].trim_start();
    }
    if let Some(m) = CAVEAT_PREFIX.find(t) {
        return t[m.end()..].trim_start();
    }
    ""
}

fn line_is_substantive_compact_finding(line: &str) -> bool {
    let t = line.trim();
    if !(SEVERITY_PREFIX.is_match(t) || CAVEAT_PREFIX.is_match(t)) {
        return false;
    }
    let rest = remainder_after_compact_finding_prefix(t);
    rest.contains(':') && rest.len() >= SUBSTANTIVE_FINDING_BODY_MIN_LEN
}

/// Gate main-thread REVIEW_GATE clear: avoid `[P2] 见上文`-style prefix-only lines.
pub fn assistant_has_substantive_compact_review_finding_line(text: &str) -> bool {
    let finding_lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|t| SEVERITY_PREFIX.is_match(t) || CAVEAT_PREFIX.is_match(t))
        .collect();
    if finding_lines.is_empty() {
        return false;
    }
    finding_lines
        .iter()
        .all(|line| line_is_substantive_compact_finding(line))
}

/// Validate review output text against compact envelope rules.
///
/// Returns a list of findings (empty = clean). All findings are advisory.
pub fn lint_review_output(text: &str) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let non_blank_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, _)| i)
        .collect();

    if non_blank_indices.is_empty() {
        findings.push(LintFinding {
            severity: LintSeverity::Info,
            message: "Review output appears empty or all-blank.".to_string(),
        });
        return findings;
    }

    let first_idx = non_blank_indices[0];
    let first_line = lines[first_idx].trim();

    // Check 1: first non-blank line must start with [P0]/[P1]/[P2]/Caveat:/Scope:/Out of scope:
    let valid_start = SEVERITY_PREFIX.is_match(first_line)
        || CAVEAT_PREFIX.is_match(first_line)
        || SCOPE_PREFIX.is_match(first_line);
    if !valid_start {
        findings.push(LintFinding {
            severity: LintSeverity::Warning,
            message: format!(
                "First non-blank line must start with `[P0]`, `[P1]`, `[P2]`, `Caveat:`, `Scope:`, or `Out of scope:`; got: {:?}",
                if first_line.len() > 60 { &first_line[..60] } else { first_line }
            ),
        });
    }

    // Check 2: no Markdown tables before the first [P*] or Caveat: line
    // (Scope:/Out of scope: lines are allowed before findings)
    let first_finding_idx = non_blank_indices
        .iter()
        .find(|&&i| {
            let line = lines[i].trim();
            SEVERITY_PREFIX.is_match(line) || CAVEAT_PREFIX.is_match(line)
        })
        .copied();

    if let Some(limit) = first_finding_idx {
        for i in 0..limit {
            if TABLE_ROW.is_match(lines[i]) {
                findings.push(LintFinding {
                    severity: LintSeverity::Warning,
                    message: "Markdown table detected before first `[P*]`/`Caveat:` line. Tables before findings violate the compact envelope.".to_string(),
                });
                break; // one table warning is enough
            }
        }
    }

    // Check 3: verdict-like lines after a finding line → OK.
    // But if a verdict-like line (blocked|revise|ship) appears *before* any [P*]/Caveat: line → warn.
    let has_any_finding = non_blank_indices.iter().any(|&i| {
        let line = lines[i].trim();
        SEVERITY_PREFIX.is_match(line) || CAVEAT_PREFIX.is_match(line)
    });

    if !has_any_finding && !first_line.is_empty() {
        // No findings at all — may still be a valid brief Scope-only reply
        // Only flag if first line looks like a verdict
        let is_verdict = first_line.starts_with("blocked")
            || first_line.starts_with("revise")
            || first_line.starts_with("ship");
        if is_verdict {
            findings.push(LintFinding {
                severity: LintSeverity::Warning,
                message: "Verdict appears before any `[P*]`/`Caveat:` finding. Verdict must appear only after the findings list.".to_string(),
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_output_with_p0_first() {
        let text =
            "[P0] src/main.rs:42 — use-after-free in parse loop — segfault on malformed input
[P1] src/lib.rs:15 — unchecked unwrap — panic on empty slice
verdict: revise before merge";
        let f = lint_review_output(text);
        assert!(f.is_empty(), "expected clean, got: {f:?}");
    }

    #[test]
    fn clean_output_with_scope_first() {
        let text = "Scope: src/main.rs
[P0] src/main.rs:42 — use-after-free — crash path";
        let f = lint_review_output(text);
        assert!(f.is_empty(), "expected clean, got: {f:?}");
    }

    #[test]
    fn clean_output_with_caveat() {
        let text = "Caveat: tests not run — residual risk in edge-case behavior
[P0] src/core.rs:10 — null deref — repro with empty config";
        let f = lint_review_output(text);
        assert!(f.is_empty(), "expected clean, got: {f:?}");
    }

    #[test]
    fn warns_on_plain_text_first() {
        let text = "This review found several issues:
[P0] src/main.rs:42 — use-after-free";
        let f = lint_review_output(text);
        assert!(!f.is_empty(), "expected warnings");
        assert!(f
            .iter()
            .any(|fl| fl.message.contains("First non-blank line")));
    }

    #[test]
    fn warns_on_table_before_findings() {
        let text = "| File | Issue | Severity |
|------|-------|----------|
| src/main.rs:42 | use-after-free | P0 |
[P0] src/main.rs:42 — use-after-free";
        let f = lint_review_output(text);
        assert!(f.iter().any(|fl| fl.message.contains("Markdown table")));
    }

    #[test]
    fn warns_on_verdict_before_finding() {
        let text = "blocked: this should not ship
Also some prose here";
        let f = lint_review_output(text);
        assert!(f
            .iter()
            .any(|fl| fl.message.contains("Verdict appears before")));
    }

    #[test]
    fn scope_and_out_of_scope_allowed_before_findings() {
        let text = "Scope: src/
Out of scope: tests/
[P0] src/main.rs:42 — use-after-free";
        let f = lint_review_output(text);
        assert!(f.is_empty(), "expected clean, got: {f:?}");
    }

    #[test]
    fn empty_text_returns_info() {
        let f = lint_review_output("");
        assert!(!f.is_empty());
        assert_eq!(f[0].severity, LintSeverity::Info);
    }

    #[test]
    fn table_after_finding_is_fine() {
        let text = "[P0] src/main.rs:42 — use-after-free
[P1] src/lib.rs:15 — unchecked unwrap
| File | Verdict |
|------|---------|
| all | revise |";
        let f = lint_review_output(text);
        assert!(f.is_empty(), "expected clean, got: {f:?}");
    }

    #[test]
    fn substantive_requires_path_anchor_or_two_lines() {
        assert!(!assistant_has_substantive_compact_review_finding_line(
            "[P2] 见上文"
        ));
        assert!(assistant_has_substantive_compact_review_finding_line(
            "[P1] core/router-rs/src/cursor_hooks/handlers.rs:3000 — issue"
        ));
        assert!(!assistant_has_substantive_compact_review_finding_line(
            "[P0] a\n[P1] b"
        ));
    }
}
