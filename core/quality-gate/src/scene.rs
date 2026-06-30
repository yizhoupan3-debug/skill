//! Scene constants for QG Route dispatch.
//!
//! These are the only valid scene values. Every SKILL.md must declare one;
use tracing;
// scene extraction defaults to GENERAL when absent or invalid.

/// General-purpose / default scene (fallback).
pub const GENERAL: &str = "general";
/// Research verification (papers, claims, evidence).
pub const RESEARCH: &str = "research";
/// Code review (correctness, security, ABI, deps, observability, first-principles).
pub const CODE_REVIEW: &str = "code_review";
/// Slide deck review (overflow, font, QA, visual layout).
pub const SLIDES: &str = "slides";
/// Visual output review (screenshot layout, accessibility, chart readability).
pub const VISUAL: &str = "visual";

/// All valid scene values, for validation.
pub(crate) const ALL: &[&str] = &[GENERAL, RESEARCH, CODE_REVIEW, SLIDES, VISUAL];

/// Return true if `s` is a valid scene constant.
pub(crate) fn is_valid(s: &str) -> bool {
    ALL.contains(&s)
}

/// Return `s` if valid, otherwise `GENERAL`. Never panics.
///
/// When an invalid scene is encountered, logs at `WARN` level (not `ERROR` — the
/// calling code may handle the fallback gracefully). The caller SHOULD check the
/// return value and surface a routing advisory if it differs from the input.
pub fn normalize(s: &str) -> &str {
    if is_valid(s) {
        s
    } else {
        tracing::warn!(
            invalid_scene = s,
            normalized_to = GENERAL,
            "QG scene: unknown scene '{s}' normalized to '{GENERAL}' — \
             this indicates a routing bug at the call site. \
             Check GOAL_STATE.scene or the checker registration target."
        );
        GENERAL
    }
}
