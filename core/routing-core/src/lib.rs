#![deny(clippy::unwrap_used, clippy::expect_used)]
//! # routing-core
//!
//! Shared routing primitives for the Routing Layer.
//!
//! Provides trigram-based fuzzy matching used by both skill routing
//! (`routing-engine`) and tool routing (`tool-routing-engine`).
//!
//! ## Re-exports
//!
//! - `fuzzy::extract_trigrams`, `fuzzy::jaccard_similarity`,
//!   `fuzzy::trigram_similarity`, `fuzzy::best_fuzzy_jaccard`

pub mod config_hooks;
pub mod fuzzy;

pub use fuzzy::{best_fuzzy_jaccard, extract_trigrams, jaccard_similarity};
