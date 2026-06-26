//! QG Route `GateChecker` adapter for the `structure` module.
//!
//! In-place adapter (Wave 5b): wraps the structure module's LaTeX
//! validation functions into a `GateChecker` for the RESEARCH scene.
//!
//! Registered by `research_harness::register_qg_checkers()`.

use quality_gate::checker::GateChecker;
use quality_gate::types::{CheckContext, CheckResult, Finding, Severity};

use super::structure;

/// QG Route checker that wraps `structure.rs` functions.
///
/// Checks:
/// - LaTeX compilability (brace balance, environment pairing)
/// - Figure/table cross-reference consistency (\ref vs \label)
pub struct Structure;

impl GateChecker for Structure {
    fn id(&self) -> &'static str {
        "structure"
    }

    fn scenes(&self) -> Vec<&'static str> {
        vec![quality_gate::scene::RESEARCH]
    }

    fn sub_scene_affinity(&self) -> Option<&'static str> {
        Some("structure")
    }

    fn description(&self) -> &'static str {
        "paper structure checks: LaTeX compilability, cross-reference consistency"
    }

    fn check(&self, ctx: &CheckContext) -> CheckResult {
        let mut findings = Vec::new();

        // Search for .tex files in the repo root to check structure.
        let repo_root = std::path::Path::new(&ctx.repo_root);

        // Check LaTeX compilability for any .tex files found
        if let Ok(entries) = std::fs::read_dir(repo_root) {
            let tex_files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "tex")
                        .unwrap_or(false)
                })
                .collect();

            for entry in &tex_files {
                let tex_path = entry.path();

                // Check compilability
                match structure::check_latex_compilable(&tex_path) {
                    Ok(true) => {}
                    Ok(false) => {
                        findings.push(Finding {
                            id: "structure_latex_compilable".to_string(),
                            severity: Severity::P0,
                            description: format!(
                                "LaTeX syntax check failed for {}",
                                tex_path.display()
                            ),
                            location: Some(tex_path.display().to_string()),
                            suggestion: Some("fix LaTeX syntax errors before submission".to_string()),
                        });
                    }
                    Err(e) => {
                        findings.push(Finding {
                            id: "structure_latex_check_error".to_string(),
                            severity: Severity::B,
                            description: format!(
                                "LaTeX check error for {}: {e}",
                                tex_path.display()
                            ),
                            location: Some(tex_path.display().to_string()),
                            suggestion: None,
                        });
                    }
                }

                // Check cross-references
                match structure::check_figure_references(&tex_path) {
                    Ok(missing) if missing.is_empty() => {}
                    Ok(missing) => {
                        for label in &missing {
                            findings.push(Finding {
                                id: "structure_missing_label".to_string(),
                                severity: Severity::B,
                                description: format!(
                                    "\\ref '{{{label}}}' in {} has no matching \\label definition",
                                    tex_path.display()
                                ),
                                location: Some(tex_path.display().to_string()),
                                suggestion: Some(format!("add \\label{{{label}}} to the referenced element")),
                            });
                        }
                    }
                    Err(e) => {
                        findings.push(Finding {
                            id: "structure_ref_check_error".to_string(),
                            severity: Severity::C,
                            description: format!(
                                "Cross-reference check error for {}: {e}",
                                tex_path.display()
                            ),
                            location: Some(tex_path.display().to_string()),
                            suggestion: None,
                        });
                    }
                }
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
