//! # routing-core
//!
//! Shared routing primitives for the Routing Layer.
//!
//! Provides:
//! - `fuzzy` — trigram-based fuzzy matching (extracted from routing-engine and mcp-tool-registry)
//! - `types` — `RoutableRecord` trait for unified scoring pipeline
//!
//! Both skill routing (`routing-engine`) and tool routing (`mcp-tool-registry`)
//! depend on this crate for shared utilities.

pub mod fuzzy;
pub mod types;

pub use fuzzy::{best_fuzzy_score, extract_trigrams, jaccard_similarity};
pub use types::RoutableRecord;
