//! Scene constants for QG Route dispatch.
//!
//! These are the only valid scene values. Every SKILL.md must declare one;
//! scene extraction defaults to GENERAL when absent or invalid.

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
pub const ALL: &[&str] = &[GENERAL, RESEARCH, CODE_REVIEW, SLIDES, VISUAL];

/// Return true if `s` is a valid scene constant.
pub fn is_valid(s: &str) -> bool {
    ALL.contains(&s)
}

/// Return `s` if valid, otherwise `GENERAL`. Never panics.
pub fn normalize(s: &str) -> &str {
    if is_valid(s) { s } else { GENERAL }
}
