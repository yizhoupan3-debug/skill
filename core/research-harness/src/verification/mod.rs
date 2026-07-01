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
