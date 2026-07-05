#![deny(clippy::unwrap_used, clippy::expect_used)]
//! # routing-core
//!
//! Shared routing primitives for the Routing Layer.
//!
//! Provides trigram-based fuzzy matching and shared token-scoring pipeline
//! used by both skill routing (`routing-engine`) and tool routing
//! (`tool-routing-engine`).
//!
//! ## Fuzzy matching
//!
//! - `fuzzy::extract_trigrams`, `fuzzy::jaccard_similarity`,
//!   `fuzzy::trigram_similarity`, `fuzzy::best_fuzzy_jaccard`
//!
//! ## Token scoring
//!
//! - `scoring::score_shared_token_matches()` — 5-step dedup token scoring
//! - `scoring::best_fuzzy_score()` — shared fuzzy rescue
//! - `scoring::TokenScoreWeights`, `scoring::TokenScoreResult`
//!
//! ## Audit logging
//!
//! - `audit_log::AuditLog` — lazily-initialized JSON-lines log writer
//! - `audit_log::days_to_date()`, `audit_log::iso_timestamp_now()` — date utilities

pub mod audit_log;
pub mod fuzzy;
pub mod scoring;

pub use fuzzy::{
    best_fuzzy_jaccard, character_ngrams, cosine_similarity, extract_trigrams, jaccard_similarity,
    weighted_ngram_similarity,
};
pub use scoring::{best_fuzzy_score, score_shared_token_matches, TokenScoreResult, TokenScoreWeights};
