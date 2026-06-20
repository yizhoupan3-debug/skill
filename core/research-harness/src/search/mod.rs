// Migrated from tools/autoresearch-rs/src/search.rs + research.rs

//! Literature search: Semantic Scholar, arXiv, paperplain MCP bridge,
//! orchestration, strategy, and claim-driven research.

pub mod arxiv;
mod helpers;
pub mod orchestration;
pub mod paperplain_bridge;
pub mod research;
pub mod semantic_scholar;
pub mod strategy;

// Re-export the shared ExternalSourceArg from helpers.
pub use helpers::ExternalSourceArg;
