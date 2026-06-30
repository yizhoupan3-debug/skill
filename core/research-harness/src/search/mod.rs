// Migrated from tools/autoresearch-rs/src/search.rs + research.rs

//! Literature search: Semantic Scholar, arXiv, paperplain MCP bridge,
//! orchestration, strategy, claim-driven research, and two-layer
//! fuzzy+authoritative search.
//!
//! ## Architecture
//!
//! ```text
//! research_literature_search (MCP tool)
//!   ├─ options::SearchOptions  — 11-parameter config (query, limit, source,
//!   │                             year_from/to, sort_by, categories,
//!   │                             advanced_query, fuzzy_query, prefer_authoritative)
//!   ├─ orchestration::search_raw()  — Layer 1: broadcast to sources
//!   │   ├─ semantic_scholar::search()    — S2 Graph API (year filter)
//!   │   └─ arxiv::search()              — arXiv Atom (quick-xml, date sort, cat filter)
//!   └─ orchestration::search_raw()  — Layer 2 (when prefer_authoritative=true)
//!       └─ score_paper()           — composite: DOI + IF + conf tier + cites + recency
//! ```
//!
//! ## Search options
//!
//! | Parameter | Type | Default | Description |
//! |-----------|------|---------|-------------|
//! | `query` | string | required | plain text search |
//! | `limit` | integer | 20 | max results per source (1..100) |
//! | `source` | string | "all" | "all" / "semantic-scholar" / "arxiv" |
//! | `year_from` | integer | — | min publication year |
//! | `year_to` | integer | — | max publication year |
//! | `sort_by` | string | "relevance" | "relevance" / "date" (arXiv) |
//! | `categories` | string | — | arXiv category filter, comma-sep |
//! | `advanced_query` | string | — | arXiv native syntax override |
//! | `fuzzy_query` | bool | false | arXiv word-level OR fuzzy matching |
//! | `prefer_authoritative` | bool | false | 2-pass score+rank (3× fetch) |

pub mod arxiv;
mod helpers;
pub mod options;
pub mod orchestration;
pub mod paperplain_bridge;
pub mod research;
pub mod semantic_scholar;
pub mod strategy;

// Re-export shared types.
pub use helpers::ExternalSourceArg;
pub use options::{SearchOptions, SortBy};
