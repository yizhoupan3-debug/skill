//! Verification pipelines: literature, statistical, prose QC, structure, formal.

pub mod asymptotic;
pub mod asymptotic_gate;
pub mod auto_prover;
pub mod formal;
pub mod formal_gate;
pub mod inequality;
pub mod inequality_gate;
pub mod lean_bridge;
pub mod literature;
pub mod literature_gate;
pub mod proof_trace;
pub mod prose_qc;
pub mod prose_qc_gate;
pub mod python_bridge;
pub mod reproducibility;
pub mod reproducibility_gate;
pub mod statistical;
pub mod statistical_gate;
pub mod structure;
pub mod structure_gate;
pub mod symbolic;
pub mod symbolic_gate;
pub mod sympy_bridge;
pub mod sympy_bridge_gate;
pub mod z3_bridge;

/// Shared test utilities for verification gate tests.
#[cfg(test)]
pub(crate) mod test_util {
    use quality_gate::types::CheckContext;

    /// Build a minimal `CheckContext` with the given `output_data`.
    ///
    /// All gate tests need this identical boilerplate. Using this shared
    /// function eliminates the `fn ctx()` helper that was copy-pasted
    /// across 5+ gate test files.
    pub fn test_ctx(output_data: Option<serde_json::Value>) -> CheckContext {
        CheckContext {
            scene: "test".into(),
            sub_scene: None,
            goal: "test".into(),
            round: 1,
            repo_root: std::path::PathBuf::from("."),
            task_id: "t1".into(),
            evidence_path: None,
            runtime_handle: None,
            output_data,
            evaluated_at: "2026-01-01T00:00:00Z".into(),
        }
    }
}
