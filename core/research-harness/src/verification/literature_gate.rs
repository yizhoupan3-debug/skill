//! QG Route `GateChecker` adapter for `literature` module.
//!
//! In-place adapter (Wave 5b): wraps the `verification::literature` module's
//! pure functions into a `GateChecker` for the RESEARCH scene.
//!
//! Searches paper files for DOI references and verifies reachability
//! when an async runtime handle is available.
//!
//! Registered via `RUNTIME_REGISTRY.json` → `quality_gate_checkers.registrations`.

use std::path::Path;

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};
use regex::Regex;

use super::literature;

/// File extensions to search for DOI references.
const PAPER_EXTENSIONS: &[&str] = &["tex", "md", "txt", "bib", "bbl"];

/// QG Route checker that wraps `literature` module functions.
///
/// Checks:
/// - DOI presence in paper files
/// - DOI reachability (via HEAD request to doi.org)
/// - Claim coverage (future: needs structured claims/references)
pub struct Literature;

impl GateChecker for Literature {
    fn id(&self) -> &'static str {
        "literature"
    }

    fn description(&self) -> &'static str {
        "literature verification checks: DOI extraction and reachability, claim coverage analysis"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let repo_root = Path::new(&ctx.repo_root);
        #[allow(clippy::unwrap_used, clippy::expect_used)]
        let doi_re = Regex::new(r"10\.\d{4,}/[\w\.\-/:]+").unwrap();
        let paper_files = find_literature_files(repo_root);

        if paper_files.is_empty() {
            findings.push(Finding {
                id: "literature_no_paper".to_string(),
                severity: Severity::C,
                description: format!(
                    "no paper or bibliography files found at {:?} — literature checks skipped",
                    repo_root,
                ),
                location: None,
                suggestion: Some(
                    "ensure paper source files (.tex, .md, .bib) are at the repository root"
                        .to_string(),
                ),
            });

            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        }

        // Collect all DOIs from paper files.
        let mut all_dois: Vec<String> = Vec::new();
        for file_path in &paper_files {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    findings.push(Finding {
                        id: "literature_read_error".to_string(),
                        severity: Severity::C,
                        description: format!(
                            "cannot read literature file {}: {e}",
                            file_path.display()
                        ),
                        location: Some(file_path.display().to_string()),
                        suggestion: None,
                    });
                    continue;
                }
            };
            for cap in doi_re.captures_iter(&content) {
                all_dois.push(cap[0].to_string());
            }
        }

        if all_dois.is_empty() {
            findings.push(Finding {
                id: "literature_no_doi".to_string(),
                severity: Severity::C,
                description: "no DOI references found in paper files".to_string(),
                location: None,
                suggestion: Some("add DOI references to paper or bibliography files".to_string()),
            });
        } else {
            // Deduplicate.
            all_dois.sort();
            all_dois.dedup();

            findings.push(Finding {
                id: "literature_doi_count".to_string(),
                severity: Severity::C,
                description: format!(
                    "{} unique DOI references found across {} files",
                    all_dois.len(),
                    paper_files.len(),
                ),
                location: None,
                suggestion: None,
            });

            // Verify reachability when a tokio runtime handle is available.
            // 使用 buffer_unordered 并发验证所有 DOI，限制最大并发为 5 防止限流。
            const MAX_DOI_CONCURRENCY: usize = 5;
            if let Some(ref handle) = ctx.runtime_handle {
                let results = handle.block_on(async {
                    use futures::stream::{self, StreamExt};
                    let futures = all_dois.iter().map(|doi| literature::verify_doi_reachable(doi));
                    stream::iter(futures)
                        .buffer_unordered(MAX_DOI_CONCURRENCY)
                        .collect::<Vec<_>>()
                        .await
                });
                let mut unreachable = Vec::new();
                for (doi, result) in all_dois.iter().zip(results) {
                    match result {
                        Ok(true) => {}
                        Ok(false) => {
                            unreachable.push(doi.clone());
                        }
                        Err(e) => {
                            findings.push(Finding {
                                id: "literature_doi_check_error".to_string(),
                                severity: Severity::C,
                                description: format!(
                                    "DOI reachability check failed for '{doi}': {e}"
                                ),
                                location: None,
                                suggestion: None,
                            });
                        }
                    }
                }
                if !unreachable.is_empty() {
                    let severity = if unreachable.len() > 3 {
                        Severity::Warning
                    } else {
                        Severity::C
                    };
                    findings.push(Finding {
                        id: "literature_doi_unreachable".to_string(),
                        severity,
                        description: format!(
                            "{} DOI(s) unreachable: {}",
                            unreachable.len(),
                            unreachable.join(", "),
                        ),
                        location: None,
                        suggestion: Some(
                            "verify that all cited DOIs resolve at https://doi.org/".to_string(),
                        ),
                    });
                }
            } else {
                findings.push(Finding {
                    id: "literature_no_runtime".to_string(),
                    severity: Severity::C,
                    description: format!(
                        "{} DOI(s) found but async runtime not available — reachability check skipped",
                        all_dois.len(),
                    ),
                    location: None,
                    suggestion: Some(
                        "provide a tokio runtime handle in CheckContext for DOI verification"
                            .to_string(),
                    ),
                });
            }
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

/// Find literature files (tex/md/txt/bib/bbl) near the repo root.
fn find_literature_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();

    // Check well-known paper file names in the root directory.
    let well_known = [
        "paper.tex",
        "paper.md",
        "main.tex",
        "draft.tex",
        "draft.md",
        "references.bib",
        "refs.bib",
        "bibliography.bib",
    ];
    for name in &well_known {
        let path = root.join(name);
        if path.is_file() {
            files.push(path);
        }
    }

    if !files.is_empty() {
        return files;
    }

    // Broader scan: collect non-hidden files with recognized extensions.
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
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if PAPER_EXTENSIONS.contains(&ext) {
                    files.push(path);
                }
            }
        }
    }

    files
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use quality_gate::checker::GateChecker;
    use quality_gate::types::CheckContext;

    fn ctx_with_repo(repo_root: std::path::PathBuf) -> CheckContext {
        CheckContext {
            scene: "test".into(),
            sub_scene: None,
            goal: "test".into(),
            round: 1,
            repo_root,
            task_id: "t1".into(),
            evidence_path: None,
            runtime_handle: None,
            output_data: None,
            evaluated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn no_paper_files_returns_passed() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let gate = Literature;
        let result = gate.check(&ctx_with_repo(dir.path().to_path_buf()));
        assert!(result.passed);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].id, "literature_no_paper");
        assert!(matches!(result.findings[0].severity, Severity::C));
    }

    #[test]
    fn paper_with_doi_finds_references() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let paper_path = dir.path().join("paper.tex");
        std::fs::write(&paper_path, "See \\cite{10.1234/test.5678} for details.")
            .expect("write paper");
        let gate = Literature;
        let result = gate.check(&ctx_with_repo(dir.path().to_path_buf()));
        assert!(result.passed);
        // Should have doi_count finding and no_runtime finding
        let doi_finding = result.findings.iter().find(|f| f.id == "literature_doi_count");
        assert!(doi_finding.is_some());
    }

    #[test]
    fn paper_without_doi_reports_no_doi() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let paper_path = dir.path().join("paper.tex");
        std::fs::write(&paper_path, "This paper has no DOI references.")
            .expect("write paper");
        let gate = Literature;
        let result = gate.check(&ctx_with_repo(dir.path().to_path_buf()));
        assert!(result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.id == "literature_no_doi"));
    }

    #[test]
    fn no_runtime_skips_reachability() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let paper_path = dir.path().join("paper.tex");
        std::fs::write(&paper_path, "Reference: 10.1234/test.5678")
            .expect("write paper");
        let gate = Literature;
        let result = gate.check(&ctx_with_repo(dir.path().to_path_buf()));
        // No runtime_handle provided → no_runtime finding
        assert!(result
            .findings
            .iter()
            .any(|f| f.id == "literature_no_runtime"));
    }
}
