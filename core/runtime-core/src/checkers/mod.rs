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
//! Note: EvidenceChecker 不在 Stage 2 注册——Stage 1 反欺诈门已做证据完整性验证。
//!
//! ## RESEARCH scene checkers
//! Research verification checkers are registered via the extern mechanism
//! in `router-rs-cli.rs` through `research_harness::register_qg_checkers`.
//! The Wave 5b alias checkers that lived here were removed in favor of the
//! extern registration to avoid double registration.
//!
//! ## External registration bridge
//! The extern hook is set via `runtime_core::qg_route::set_extern_checkers()`
//! before `init_hooks()`. See `router-rs-cli.rs` for the wiring.

pub mod adversarial_checker;
pub mod correctness_checker;
pub mod evidence_checker;
pub mod overflow_checker;
pub mod screenshot_layout_checker;
pub mod security_checker;

use std::path::Path;

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
            find_rust_files_recursive(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

/// Register all in-place checkers into the registry.
/// Called once at startup from `runtime_core::init_quality_gate()`.
pub(crate) fn register_checkers(registry: &mut quality_gate::CheckerRegistry) {
    registry.register(scene::GENERAL, Box::new(evidence_checker::EvidenceChecker));
    registry.register(
        scene::GENERAL,
        Box::new(adversarial_checker::AdversarialChecker),
    );
    registry.register(
        scene::RESEARCH,
        Box::new(adversarial_checker::AdversarialChecker),
    );
    registry.register(
        scene::CODE_REVIEW,
        Box::new(correctness_checker::CorrectnessChecker),
    );
    registry.register(
        scene::CODE_REVIEW,
        Box::new(security_checker::SecurityChecker),
    );
    registry.register(
        scene::VISUAL,
        Box::new(screenshot_layout_checker::ScreenshotLayoutChecker),
    );
    registry.register(scene::SLIDES, Box::new(overflow_checker::OverflowChecker));
}
