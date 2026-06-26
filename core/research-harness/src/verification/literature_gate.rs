//! QG Route `GateChecker` adapter for `literature` module.
//!
//! In-place adapter (Wave 5b): wraps the `verification::literature` module's
//! pure functions into a `GateChecker` for the RESEARCH scene.
//!
//! Searches paper files for DOI references and verifies reachability
//! when an async runtime handle is available.
//!
//! Registered by `research_harness::register_qg_checkers()`.

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

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "literature verification checks: DOI extraction and reachability, claim coverage analysis"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let repo_root = Path::new(&ctx.repo_root);
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
                suggestion: Some(
                    "add DOI references to paper or bibliography files".to_string(),
                ),
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
            if let Some(ref handle) = ctx.runtime_handle {
                let mut unreachable = Vec::new();
                for doi in &all_dois {
                    match handle.block_on(literature::verify_doi_reachable(doi)) {
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
        "paper.tex", "paper.md", "main.tex", "draft.tex", "draft.md",
        "references.bib", "refs.bib", "bibliography.bib",
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
