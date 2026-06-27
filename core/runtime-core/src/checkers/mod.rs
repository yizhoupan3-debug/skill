//! In-place GateChecker implementations for the QG Route.
//!
//! Wave 4b: each checker lives in its natural module location and adds
//! `impl GateChecker` as an adapter. This module coordinates registration
//! into the shared `CheckerRegistry`.
//!
//! ## Available checkers
//! - `evidence_checker`: validates that task evidence exists
//! - (Wave 4b+) ProseQC, LogicAndEvidence, LiteracyChecker, etc.
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
}
