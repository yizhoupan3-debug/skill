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
pub mod log;
pub mod mcp_tools;
pub mod proof_dag;
pub mod proof_dag_serialize;
pub mod provenance;
pub mod render;
pub mod review;
pub mod search;
#[cfg(feature = "smoke")]
pub mod smoke;
pub mod state;
pub mod subprocess;
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

/// Register all research verification gate checkers into the QG Route registry.
///
/// Call this during application bootstrap (after `init_qg_route()`) to register
/// RESEARCH-scene checkers from research-harness into the shared CheckerRegistry.
pub fn register_qg_checkers(registry: &mut quality_gate::CheckerRegistry) {
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::asymptotic_gate::Asymptotic),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::formal_gate::DimensionalConsistency),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::inequality_gate::Inequality),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::literature_gate::Literature),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::prose_qc_gate::ProseQCChecker),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::reproducibility_gate::Reproducibility),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::statistical_gate::StatisticalChecker),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::structure_gate::Structure),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::symbolic_gate::Symbolic),
    );
    registry.register(
        quality_gate::scene::RESEARCH,
        Box::new(verification::sympy_bridge_gate::SympyBridge),
    );
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn smoke() {
        assert!(true);
    }
}
