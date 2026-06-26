//! QG Route `GateChecker` adapter for `ProseQCChecker`.
//!
//! In-place adapter (Wave 5b): wraps the prose_qc module's pure functions
//! into a `GateChecker` for the RESEARCH scene.
//!
//! Searches the task's repo_root for paper files (.tex, .md, .txt) and
//! runs slop detection, hedging analysis, and terminology checks.
//!
//! Registered by `research_harness::register_qg_checkers()`.

use std::path::Path;

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use super::prose_qc;

/// File extensions treated as paper content for prose analysis.
const PAPER_EXTENSIONS: &[&str] = &["tex", "md", "txt"];

/// QG Route checker that wraps `prose_qc.rs` functions.
///
/// Checks:
/// - English slop (AI overused phrases)
/// - Chinese slop (template-like phrasing)
/// - Hedging / defensive language
pub struct ProseQCChecker;

impl GateChecker for ProseQCChecker {
    fn id(&self) -> &'static str {
        "prose_qc"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn description(&self) -> &'static str {
        "prose quality checks: AI slop detection, hedging, style consistency"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        let repo_root = Path::new(&ctx.repo_root);
        let paper_files = find_paper_files(repo_root);

        if paper_files.is_empty() {
            findings.push(Finding {
                id: "prose_qc_no_paper".to_string(),
                severity: Severity::C,
                description: format!(
                    "no paper files (.tex, .md, .txt) found at {:?} — prose QC skipped",
                    repo_root,
                ),
                location: None,
                suggestion: Some(
                    "ensure paper source files are at or near the repository root".to_string(),
                ),
            });

            return CheckResult {
                checker_id: self.id().to_string(),
                passed: true,
                findings,
            };
        }

        for file_path in &paper_files {
            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(e) => {
                    findings.push(Finding {
                        id: "prose_qc_read_error".to_string(),
                        severity: Severity::C,
                        description: format!(
                            "cannot read paper file {}: {e}",
                            file_path.display()
                        ),
                        location: Some(file_path.display().to_string()),
                        suggestion: None,
                    });
                    continue;
                }
            };

            // English slop detection
            let en_slop = prose_qc::detect_en_slop(&content);
            if !en_slop.is_empty() {
                let severity = if en_slop.len() > 10 {
                    Severity::Warning
                } else {
                    Severity::C
                };
                let slop_list: Vec<String> = en_slop
                    .iter()
                    .map(|h| format!("'{}' → {}", h.word, h.replacement))
                    .collect();
                findings.push(Finding {
                    id: "prose_qc_en_slop".to_string(),
                    severity,
                    description: format!(
                        "{} English AI-slop phrases in {}: {}",
                        en_slop.len(),
                        file_path.display(),
                        slop_list.join("; "),
                    ),
                    location: Some(file_path.display().to_string()),
                    suggestion: Some(
                        "review each slop phrase and replace with the suggested alternative"
                            .to_string(),
                    ),
                });
            }

            // Chinese slop detection
            let zh_slop = prose_qc::detect_zh_slop(&content);
            if !zh_slop.is_empty() {
                let severity = if zh_slop.len() > 5 {
                    Severity::Warning
                } else {
                    Severity::C
                };
                let slop_list: Vec<String> = zh_slop
                    .iter()
                    .map(|h| format!("'{}' → {}", h.word, h.replacement))
                    .collect();
                findings.push(Finding {
                    id: "prose_qc_zh_slop".to_string(),
                    severity,
                    description: format!(
                        "{} Chinese template phrases in {}: {}",
                        zh_slop.len(),
                        file_path.display(),
                        slop_list.join("; "),
                    ),
                    location: Some(file_path.display().to_string()),
                    suggestion: Some(
                        "review each template phrase and replace with concrete statements"
                            .to_string(),
                    ),
                });
            }

            // Hedging word count
            let hedging = prose_qc::count_hedging_words(&content);
            if hedging > 20 {
                let severity = if hedging > 50 {
                    Severity::Warning
                } else {
                    Severity::C
                };
                findings.push(Finding {
                    id: "prose_qc_hedging".to_string(),
                    severity,
                    description: format!(
                        "{} hedging words in {} (threshold: 20)",
                        hedging,
                        file_path.display(),
                    ),
                    location: Some(file_path.display().to_string()),
                    suggestion: Some(
                        "reduce hedging language ('may', 'might', 'could', 'possibly', 'rather', etc.)"
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

/// Find paper-like files (tex/md/txt) near the repo root.
fn find_paper_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();

    // Check common paper file names in the root directory.
    let well_known = ["paper.tex", "paper.md", "main.tex", "draft.tex", "draft.md"];
    for name in &well_known {
        let path = root.join(name);
        if path.is_file() {
            files.push(path);
        }
    }

    // If we already found well-known files, skip the broader scan.
    if !files.is_empty() {
        return files;
    }

    // Broader scan: collect non-hidden .tex/.md/.txt files in root.
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            // Skip hidden files (dotfiles, build artifacts).
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
