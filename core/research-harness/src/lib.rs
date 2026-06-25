#![deny(clippy::unwrap_used, clippy::expect_used)]
//! # Research Harness
//!
//! Unified research harness crate for the skill framework.
//! Integrates paper revision loop, literature search, claims management,
//! AIGC detection/reduction, verification pipelines, and research logging.
//!
//! ## Module Map
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | `search` | Literature search (Semantic Scholar, arXiv, paperplain MCP bridge), claim-driven research orchestration |
//! | `claims` | Claim ledger management, drift detection, ceiling computation, hypothesis/run/reflect lifecycle |
//! | `log` | Research activity logging (SQLite FTS5), knowledge graph, entity extraction |
//! | `citation` | Citation audit, BibTeX rendering, DOI validation |
//! | `review` | Multi-round adversarial review orchestration, dimensions, convergence |
//! | `hooks` | Paper prose/ adversarial hooks, research activity log hooks |
//! | `aigc` | AIGC detection (n-gram, burstiness, syntactic patterns), humanization |
//! | `verification` | Literature, statistical, prose QC, structure, formal verification |
//! | `latex` | LaTeX math formula parser and SVG renderer (based on RaTeX) |
//! | `render` | Markdown rendering pipeline (findings, novelty gate, search plan, hypothesis cards, run records) |
//! | `state` | Research state persistence: load/save/migrate/hydrate from YAML/JSON |
//! | `workspace` | Workspace initialization, file sync, ledger events |
//! | `text` | Text processing: slugification, XML parsing, content word extraction |
//! | `provenance` | Git provenance and environment fingerprint capture |
//! | `smoke` | Smoke tests for academic source freshness |

pub mod aigc;
pub mod citation;
pub mod claims;
pub mod hooks;
pub mod latex;
pub mod log;
pub mod mcp_tools;
pub mod proof_dag;
pub mod proof_dag_serialize;
pub mod provenance;
pub mod render;
pub mod subprocess;
pub mod research_mode;
pub mod review;
pub mod search;
#[cfg(feature = "smoke")]
pub mod smoke;
pub mod state;
pub mod text;
pub mod types;
pub mod util;
pub mod verification;
pub mod workspace;

/// Register all research hooks into the L0 function-pointer registry.
///
/// Must be called once during application bootstrap, after `runtime_core::init_hooks()`.
/// Safe to call multiple times — internal `OnceLock` guards make repeated
/// registration calls no-ops.
pub use hooks::init::init_hooks;
