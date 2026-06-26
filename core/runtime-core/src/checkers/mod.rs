//! In-place GateChecker implementations for the QG Route.
//!
//! Wave 4b: each checker lives in its natural module location and adds
//! `impl GateChecker` as an adapter. This module coordinates registration
//! into the shared `CheckerRegistry`.
//!
//! ## Available checkers
//! - `evidence_checker`: validates that task evidence exists
//! - (Wave 4b+) ProseQC, LogicAndEvidence, LiteracyChecker, etc.
//! - (Wave 5b) 6 verification skill → QG Checker aliases for RESEARCH scene

pub mod adversarial_checker;
pub mod correctness_checker;
pub mod evidence_checker;
pub mod formal_gate;
pub mod literature_gate;
pub mod overflow_checker;
pub mod prose_qc;
pub mod reproducibility;
pub mod screenshot_layout_checker;
pub mod security_checker;
pub mod statistical_gate;
pub mod structure_gate;

use quality_gate::scene;

/// Register all in-place checkers into the registry.
/// Called once at startup from `runtime_core::init_quality_gate()`.
pub fn register_checkers(registry: &mut quality_gate::CheckerRegistry) {
    registry.register(
        scene::GENERAL,
        Box::new(evidence_checker::EvidenceChecker),
    );
    registry.register(
        scene::GENERAL,
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
    registry.register(
        scene::SLIDES,
        Box::new(overflow_checker::OverflowChecker),
    );
    // Wave 5b: 6 verification skill → QG Checker aliases for RESEARCH scene
    registry.register(
        scene::RESEARCH,
        Box::new(prose_qc::ProseQcChecker::new()),
    );
    registry.register(
        scene::RESEARCH,
        Box::new(literature_gate::LiteratureGateChecker::new()),
    );
    registry.register(
        scene::RESEARCH,
        Box::new(statistical_gate::StatisticalGateChecker::new()),
    );
    registry.register(
        scene::RESEARCH,
        Box::new(reproducibility::ReproducibilityChecker::new()),
    );
    registry.register(
        scene::RESEARCH,
        Box::new(structure_gate::StructureGateChecker::new()),
    );
    registry.register(
        scene::RESEARCH,
        Box::new(formal_gate::FormalGateChecker::new()),
    );
}
