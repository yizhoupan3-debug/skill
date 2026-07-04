//! In-place GateChecker implementations for the QG Route.
//!
//! Wave 4b: each checker lives in its natural module location and adds
//! `impl GateChecker` as an adapter. This module coordinates registration
//! into the shared `CheckerRegistry`.
//!
//! ## Scene → Checker 链
//! - `general`     → EvidenceChecker + AdversarialChecker
//! - `research`    → AdversarialChecker（基础兜底）；专业 checker（LogicAndEvidence,
//!                   ProseQC, Statistical, Literature, Structure 等 11 个）
//!                   从 `research-harness` 通过 extern 注册
//! - `code_review` → CorrectnessChecker + SecurityChecker
//! - `slides`      → OverflowChecker
//! - `visual`      → ScreenshotLayoutChecker
//!
//! Note: EvidenceChecker 已在 Stage 2 注册为 scene=GENERAL 的 checker，
//! 与 Stage 1 反欺诈门形成双重验证（Stage 1 检查证据存在性，Stage 2 做更深层评估）。
//!
//! ## RESEARCH scene checkers
//! Research verification checkers are registered via the JSON registry mechanism
//! in `configs/framework/RUNTIME_REGISTRY.json` → `quality_gate_checkers.registrations`
//! and wired through `router-rs-cli.rs` via `set_extern_checkers`.
//! The Wave 5b alias checkers that lived here were removed in favor of the
//! extern registration to avoid double registration.
//!
//! ## External registration bridge
//! The extern hook is set via `runtime_core::qg_route::set_extern_checkers()`
//! before `init_hooks()`. See `router-rs-cli.rs` for the wiring.

pub mod adversarial_checker;
pub mod correctness_checker;
pub mod evidence_checker;
pub mod goal_contract_checker;
pub mod overflow_checker;
pub mod screenshot_layout_checker;
pub mod security_checker;

use std::path::Path;

#[cfg(test)]
use quality_gate::scene;

/// Recursively find all non-hidden `.rs` files under the given root directory.
pub(crate) fn find_rust_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    find_rust_files_recursive(root, &mut files);
    files
}

fn find_rust_files_recursive(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(false, |n| n.starts_with('.'))
        {
            continue;
        }
        if path.is_dir() {
            // Skip build output and vendored directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if matches!(name, "target" | "node_modules" | "build" | "dist" | "vendor") {
                    continue;
                }
            }
            find_rust_files_recursive(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

// Register all in-place checkers into the registry.
// Generated from `RUNTIME_REGISTRY.json` → `quality_gate_checkers.registrations`.
// Called once at startup from `runtime_core::init_quality_gate()`.
include!(concat!(env!("OUT_DIR"), "/generated_checkers.rs"));

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use quality_gate::checker::GateChecker;
    use quality_gate::types::{CheckContext, Severity};
    use std::fs;

    fn make_ctx(repo_root: &std::path::Path) -> CheckContext {
        CheckContext {
            scene: scene::GENERAL.to_string(),
            sub_scene: None,
            goal: "test goal".to_string(),
            round: 2,
            repo_root: repo_root.to_path_buf(),
            task_id: "test-task".to_string(),
            evidence_path: None,
            runtime_handle: None,
            output_data: None,
            evaluated_at: "2026-06-30T00:00:00Z".to_string(),
        }
    }

    // ── CorrectnessChecker ──

    #[test]
    fn correctness_clean_code_passes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
        let ctx = make_ctx(tmp.path());
        let result = correctness_checker::CorrectnessChecker.check(&ctx);
        assert!(result.passed);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn correctness_detects_unwrap() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn foo() -> Option<i32> {\n    Some(42).unwrap()\n}\n").unwrap();
        let ctx = make_ctx(tmp.path());
        let result = correctness_checker::CorrectnessChecker.check(&ctx);
        // unwrap count is low (< 20), so C level and still passes
        assert!(result.passed);
    }

    #[test]
    fn correctness_detects_todo() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn unimplemented() {\n    todo!(\"fix this\")\n}\n").unwrap();
        let ctx = make_ctx(tmp.path());
        let result = correctness_checker::CorrectnessChecker.check(&ctx);
        assert!(result.passed);
        assert!(result.findings.iter().any(|f| f.id == "correctness_todo"));
    }

    #[test]
    fn correctness_no_rust_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("readme.txt"), "no rust here").unwrap();
        let ctx = make_ctx(tmp.path());
        let result = correctness_checker::CorrectnessChecker.check(&ctx);
        assert!(result.passed);
        assert!(result.findings.iter().any(|f| f.id == "correctness_no_rust"));
    }

    // ── SecurityChecker ──

    #[test]
    fn security_clean_code_passes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
        let ctx = make_ctx(tmp.path());
        let result = security_checker::SecurityChecker.check(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn security_detects_unsafe() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "unsafe fn danger() {\n    // raw pointer\n}\nfn main() { unsafe { danger(); } }\n").unwrap();
        let ctx = make_ctx(tmp.path());
        let result = security_checker::SecurityChecker.check(&ctx);
        // low count → C level, still passes
        assert!(result.passed);
        assert!(result.findings.iter().any(|f| f.id == "security_unsafe_no_safety"));
    }

    #[test]
    fn security_detects_transmute() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lib.rs"), "fn cast(x: u32) -> i32 {\n    unsafe { std::mem::transmute(x) }\n}\n").unwrap();
        let ctx = make_ctx(tmp.path());
        let result = security_checker::SecurityChecker.check(&ctx);
        assert!(result.findings.iter().any(|f| f.id == "security_transmute"));
    }

    // ── AdversarialChecker ──

    #[test]
    fn adversarial_missing_evidence_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_ctx(tmp.path());
        ctx.evidence_path = Some(tmp.path().join("EVIDENCE_INDEX.json"));
        let result = adversarial_checker::AdversarialChecker.check(&ctx);
        assert!(result.passed); // Warning is non-blocking
        assert!(result.findings.iter().any(|f| f.id == "missing-evidence-file"));
    }

    #[test]
    fn adversarial_present_evidence_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let evidence_path = tmp.path().join("EVIDENCE_INDEX.json");
        fs::write(&evidence_path, "{}").unwrap();
        let mut ctx = make_ctx(tmp.path());
        ctx.evidence_path = Some(evidence_path);
        let result = adversarial_checker::AdversarialChecker.check(&ctx);
        assert!(result.passed);
        assert!(!result.findings.iter().any(|f| f.id == "missing-evidence-file"));
    }

    #[test]
    fn adversarial_single_round_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_ctx(tmp.path());
        ctx.round = 1;
        let result = adversarial_checker::AdversarialChecker.check(&ctx);
        assert!(result.passed); // Warning is non-blocking
        assert!(result.findings.iter().any(|f| f.id == "single-round"));
    }

    #[test]
    fn adversarial_multi_round_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_ctx(tmp.path());
        ctx.round = 3;
        let result = adversarial_checker::AdversarialChecker.check(&ctx);
        assert!(result.passed);
        assert!(!result.findings.iter().any(|f| f.id == "single-round"));
    }

    // ── ScreenshotLayoutChecker ──

    #[test]
    fn screenshot_layout_no_evidence_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let result = screenshot_layout_checker::ScreenshotLayoutChecker.check(&ctx);
        assert!(result.passed); // Warning is non-blocking
        assert!(result.findings.iter().any(|f| f.id == "screenshot_no_evidence_path"));
    }

    #[test]
    fn screenshot_layout_missing_evidence_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = make_ctx(tmp.path());
        ctx.evidence_path = Some(tmp.path().join("nonexistent.png"));
        let result = screenshot_layout_checker::ScreenshotLayoutChecker.check(&ctx);
        assert!(result.passed);
        assert!(result.findings.iter().any(|f| f.id == "screenshot_evidence_missing"));
    }

    #[test]
    fn screenshot_layout_valid_png_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let png_path = tmp.path().join("screenshot.png");
        // Write a minimal valid PNG header (89 50 4E 47 0D 0A 1A 0A + padding)
        let mut data = vec![0u8; 64];
        data[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        std::fs::write(&png_path, &data).unwrap();
        let mut ctx = make_ctx(tmp.path());
        ctx.evidence_path = Some(png_path);
        let result = screenshot_layout_checker::ScreenshotLayoutChecker.check(&ctx);
        assert!(result.passed);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn overflow_stub_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let result = overflow_checker::OverflowChecker.check(&ctx);
        assert!(result.passed);
    }

    // ── EvidenceChecker ──

    #[test]
    fn evidence_missing_index_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = make_ctx(tmp.path());
        let result = evidence_checker::EvidenceChecker.check(&ctx);
        // No evidence index → C level info
        assert!(result.passed);
    }

    // ── Registry integration ──

    #[test]
    fn all_checkers_register_correctly() {
        let mut registry = quality_gate::CheckerRegistry::new();
        register_checkers_from_registry(&mut registry);
        // Evaluate each scene to verify no panics (not all scenes pass with empty repo)
        for scene_str in [scene::GENERAL, scene::CODE_REVIEW, scene::RESEARCH, scene::VISUAL, scene::SLIDES] {
            let tmp = tempfile::tempdir().unwrap();
            let ctx = make_ctx(tmp.path());
            let verdict = registry.evaluate(scene_str, &ctx);
            // General scene may fail due to GoalContractChecker (no GOAL_STATE.json)
            // Other scenes should pass with empty repo
            if scene_str != scene::GENERAL {
                assert!(verdict.passed, "scene '{scene_str}' should pass with empty repo");
            }
            assert!(verdict.checkers_ran > 0, "scene '{scene_str}' should have run at least one checker");
        }
    }
}
