//! MCP tool dispatch for research tools.
//!
//! Delegated from host-projection's tool dispatcher (Phase 4 T1).
//!
//! # Dispatch architecture
//!
//! ## External entry point
//! [`handle_research_tool`] is the sole public entry point. It is wired into
//! host-projection's tool dispatcher via the global `research_tool_dispatch`
//! hook (set at runtime-core startup in `hooks.rs`). The host-projection
//! `dispatch_tool` function falls through to this hook for any tool name not
//! handled by its built-in `CompositeRegistry` (task CRUD, loop control, etc.).
//!
//! Tools reachable through [`handle_research_tool`] are registered in
//! **`MCP_TOOL_REGISTRY.json`** with `mcp_server: "research-harness"`.
//! The routing chain is:
//!
//! ```text
//! MCP_TOOL_REGISTRY.json  ──→  host-projection dispatch_tool()
//!     └─ falls through to →  research_tool_dispatch hook
//!         └─ calls →  handle_research_tool()
//! ```
//!
//! ## Sub-dispatchers (internal-only, NOT in MCP_TOOL_REGISTRY directly)
//!
//! These functions route to specific tool families and are never
//! called directly by the MCP layer — only via [`handle_research_tool`]:
//!
//! - [`math_tool_dispatch`] — routes all `math_*` tools
//! - [`verification_tool_dispatch`] — routes all `research_verification_*` tools
//!
//! ## Directly dispatched tools (in `handle_research_tool`)
//!
//! The following tools are dispatched directly (no sub-dispatcher):
//! `research_aigc_check`, `research_aigc_humanize`, `research_review_dimensions`,
//! `research_claim_drift`,
//! `research_review_loop`, `research_smoke`, `research_literature_search`.
//!
//! ## Individual tool functions
//!
//! Every `tool_*` function is implemented in its own module within this crate
//! (e.g. `aigc::tool_research_aigc_check`, `claim::tool_research_claim_drift`,
//! `proof_dag::tool_math_proof_dag_init`, etc.) and imported here for dispatch.
//! All are externally reachable — none is internal-only by design.
//!
//! # Input limits
//!
//! Array-type parameters (steps, witnesses, constraints, claims, references,
//! findings, children) are capped at MAX_ARRAY_ELEMENTS to prevent memory
//! exhaustion from oversized payloads received through the MCP tool interface.

use core_errors::FrameworkError;
use serde_json::Value;
use std::path::PathBuf;

/// Maximum number of elements in any array-type parameter to a research tool.
/// Prevents single-call memory exhaustion via malicious oversized arrays.
pub(super) const MAX_ARRAY_ELEMENTS: usize = 10_000;

/// Maximum number of key-value arguments in a tool call.
/// Prevents single-call exhaustion via oversized argument maps.
pub(super) const MAX_ARGS_ELEMENTS: usize = 100;

/// Resolve the framework project root by walking up from CWD.
/// Shared utility — avoids duplicating this function across 3 tool modules.
pub(super) fn resolve_repo_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            if d.join("templates").exists() || d.join(".git").exists() {
                return d.to_path_buf();
            }
            dir = d.parent();
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

// ── Module declarations ──

mod aigc;
mod ablation;
mod asymptotic;
mod auto_prover;
mod claim;
mod evaluation;
mod formal;
mod literature;
mod proof_dag;
mod prose;
mod reproducibility;
mod review;
mod search;
mod smoke;
mod statistical;
mod structure;
mod sympy;
mod z3;

// ── Function imports ──

use self::aigc::{tool_research_aigc_check, tool_research_aigc_humanize};
use self::ablation::tool_research_ablation;
use self::asymptotic::{tool_math_asymptotic_chain, tool_math_asymptotic_estimate};
use self::auto_prover::{
    tool_math_auto_prove, tool_math_check_homomorphism, tool_math_identity_chain,
    tool_math_perturbation_expand, tool_math_proof_trace_record,
    tool_math_prove_inequality, tool_math_tighten_bounds, tool_math_witness_consistency,
};
use self::claim::tool_research_claim_drift;
use self::evaluation::tool_research_evaluate;
use self::formal::tool_verification_formal;
use self::literature::tool_verification_literature;
use self::proof_dag::{
    tool_math_proof_dag_decompose, tool_math_proof_dag_init, tool_math_proof_dag_status,
    tool_math_proof_dag_verify,
};
use self::prose::tool_verification_prose;
use self::reproducibility::tool_verification_reproducibility;
use self::review::{tool_research_review_dimensions, tool_research_review_loop};
use self::search::tool_literature_search;
use self::smoke::tool_research_smoke;
use self::statistical::tool_verification_statistical;
use self::structure::tool_verification_structure;
use self::sympy::{
    tool_math_sympy_differentiate, tool_math_sympy_dimension_propagate,
    tool_math_sympy_expand, tool_math_sympy_factor, tool_math_sympy_integrate,
    tool_math_sympy_lambdify, tool_math_sympy_limit, tool_math_sympy_series,
    tool_math_sympy_simplify, tool_math_sympy_solve, tool_math_sympy_subs,
    tool_math_sympy_trig_simplify, tool_math_sympy_verify,
};
use self::z3::{
    tool_math_backend_available, tool_math_lean_verify, tool_math_z3_check_system,
    tool_math_z3_optimize, tool_math_z3_prove, tool_math_z3_solver_add,
    tool_math_z3_solver_batch, tool_math_z3_solver_check, tool_math_z3_solver_pop,
    tool_math_z3_solver_push, tool_math_z3_solver_reset,
};

// ── Public dispatch ──

/// Handle a research MCP tool call.
/// Delegates to the appropriate research-harness module.
pub fn handle_research_tool(name: &str, arguments: &Value) -> Result<String, FrameworkError> {
    match name {
        "research_aigc_check" => Ok(tool_research_aigc_check(arguments)?),
        "research_aigc_humanize" => Ok(tool_research_aigc_humanize(arguments)?),
        "research_review_dimensions" => Ok(tool_research_review_dimensions(arguments)?),
        "research_claim_drift" => Ok(tool_research_claim_drift(arguments)?),
        "research_review_loop" => Ok(tool_research_review_loop(arguments)?),
        "research_smoke" => Ok(tool_research_smoke(arguments)?),
        "research_ablation" => Ok(tool_research_ablation(arguments)?),
        "research_evaluate" => Ok(tool_research_evaluate(arguments)?),
        "research_literature_search" => Ok(tool_literature_search(arguments)?),
        _ if name.starts_with("math_") => Ok(math_tool_dispatch(name, arguments)?),
        _ if name.starts_with("research_verification_") => {
            verification_tool_dispatch(name, arguments)
        }
        _ => Err(FrameworkError::validation(format!(
            "unknown research tool: {name}"
        ))),
    }
}

// ── Math tool sub-dispatch ──

pub(super) fn math_tool_dispatch(
    name: &str,
    arguments: &Value,
) -> Result<String, FrameworkError> {
    match name {
        "math_asymptotic_estimate" => tool_math_asymptotic_estimate(arguments),
        "math_asymptotic_chain" => tool_math_asymptotic_chain(arguments),
        // ── Proof DAG tools ──
        "math_proof_dag_init" => tool_math_proof_dag_init(arguments),
        "math_proof_dag_decompose" => tool_math_proof_dag_decompose(arguments),
        "math_proof_dag_verify" => tool_math_proof_dag_verify(arguments),
        "math_proof_dag_status" => tool_math_proof_dag_status(arguments),
        // ── SymPy bridge tools ──
        "math_sympy_verify" => tool_math_sympy_verify(arguments),
        "math_sympy_simplify" => tool_math_sympy_simplify(arguments),
        "math_sympy_trig_simplify" => tool_math_sympy_trig_simplify(arguments),
        "math_sympy_subs" => tool_math_sympy_subs(arguments),
        "math_sympy_limit" => tool_math_sympy_limit(arguments),
        "math_sympy_lambdify" => tool_math_sympy_lambdify(arguments),
        "math_sympy_expand" => tool_math_sympy_expand(arguments),
        "math_sympy_factor" => tool_math_sympy_factor(arguments),
        "math_sympy_series" => tool_math_sympy_series(arguments),
        "math_sympy_differentiate" => tool_math_sympy_differentiate(arguments),
        "math_sympy_integrate" => tool_math_sympy_integrate(arguments),
        "math_sympy_solve" => tool_math_sympy_solve(arguments),
        "math_sympy_dimension_propagate" => tool_math_sympy_dimension_propagate(arguments),
        // ── Z3 solver tools ──
        "math_z3_prove" => tool_math_z3_prove(arguments),
        "math_z3_solver_push" => tool_math_z3_solver_push(arguments),
        "math_z3_solver_pop" => tool_math_z3_solver_pop(arguments),
        "math_z3_solver_add" => tool_math_z3_solver_add(arguments),
        "math_z3_solver_check" => tool_math_z3_solver_check(arguments),
        "math_z3_solver_reset" => tool_math_z3_solver_reset(arguments),
        "math_z3_solver_batch" => tool_math_z3_solver_batch(arguments),
        "math_z3_optimize" => tool_math_z3_optimize(arguments),
        "math_z3_check_system" => tool_math_z3_check_system(arguments),
        "math_backend_available" => tool_math_backend_available(arguments),
        "math_lean_verify" => tool_math_lean_verify(arguments),
        // ── Inequality tool ──
        "math_prove_inequality" => tool_math_prove_inequality(arguments),
        // ── Auto theorem proving tools ──
        "math_auto_prove" => tool_math_auto_prove(arguments),
        "math_identity_chain" => tool_math_identity_chain(arguments),
        "math_tighten_bounds" => tool_math_tighten_bounds(arguments),
        "math_witness_consistency" => tool_math_witness_consistency(arguments),
        "math_check_homomorphism" => tool_math_check_homomorphism(arguments),
        "math_proof_trace_record" => tool_math_proof_trace_record(arguments),
        "math_perturbation_expand" => tool_math_perturbation_expand(arguments),
        _ => Err(FrameworkError::validation(format!(
            "unknown math tool: {name}"
        ))),
    }
}

// ── Verification tool sub-dispatch ──

pub(super) fn verification_tool_dispatch(
    name: &str,
    arguments: &Value,
) -> Result<String, FrameworkError> {
    match name {
        "research_verification_prose" => tool_verification_prose(arguments),
        "research_verification_statistical" => tool_verification_statistical(arguments),
        "research_verification_literature" => tool_verification_literature(arguments),
        "research_verification_structure" => tool_verification_structure(arguments),
        "research_verification_reproducibility" => tool_verification_reproducibility(arguments),
        "research_verification_formal" => tool_verification_formal(arguments),
        _ => Err(FrameworkError::validation(format!(
            "unknown verification tool: {name}"
        ))),
    }
}

// ── Dispatch-only tests ──

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use serde_json::json;

    /// Unknown tool name at the outer dispatch level.
    #[test]
    fn handle_research_tool_unknown() {
        let result = handle_research_tool("nonexistent_tool", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown research tool")
        );
    }

    /// Unknown math_* tool routes to the math sub-dispatcher correctly.
    #[test]
    fn test_math_unknown_tool() {
        let result = handle_research_tool("math_nonexistent", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown math tool")
        );
    }
}
