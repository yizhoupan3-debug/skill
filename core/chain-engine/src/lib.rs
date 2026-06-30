//! DAG Task Chain Engine — conditional branching, fan-out/fan-in, retry, timeout groups.
//!
//! This crate provides types and logic for executing task chains with DAG
//! dependency graphs. It extends the original linear TASK_CHAIN.json format
//! with parallel groups, condition gates, retry policies, and timeout groups.

pub mod compat;
pub mod engine;
pub mod scheduler;
pub mod tracker;
pub mod types;

use core_errors::FrameworkError;
use std::path::{Path, PathBuf};

/// Schema version constant for CHAIN_OUTPUT.json (re-exported from core-state).
pub use core_state::chain_output::CHAIN_OUTPUT_SCHEMA_VERSION;

/// Path to TASK_CHAIN.json in the artifacts/current directory.
pub fn chain_file_path(repo_root: &Path) -> PathBuf {
    repo_root.join("artifacts/current/TASK_CHAIN.json")
}

/// Load the current chain file, auto-detecting old linear vs new DAG format.
pub fn load_chain(repo_root: &Path) -> Result<types::ChainDagRoot, FrameworkError> {
    let path = chain_file_path(repo_root);
    if !path.is_file() {
        return Err(FrameworkError::not_found(format!(
            "TASK_CHAIN.json not found at {}",
            path.display()
        )));
    }
    compat::load_chain_file(&path)
}
