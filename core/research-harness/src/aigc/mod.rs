//! AIGC detection and humanization.
//!
//! Pure-local rule-based AIGC detection: n-gram anomaly, burstiness,
//! syntactic pattern analysis, and text humanization strategies.

pub mod detector;
pub mod humanizer;
pub mod scorer;

/// Target language for detection / humanization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Chinese,
}
