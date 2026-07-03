//! DAG Task Chain Engine — conditional branching, fan-out/fan-in, retry, timeout groups.
//!
//! This crate provides types and logic for executing task chains with DAG
//! dependency graphs. It extends the original linear TASK_CHAIN.json format
//! with parallel groups, condition gates, retry policies, and timeout groups.
//!
//! The `compat` module has been inlined (2026-07-02): `load_chain_from_path`
//! auto-detects old linear vs new DAG format.

pub mod scheduler;
pub mod tracker;
pub mod types;

use core_errors::FrameworkError;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Schema version constant for CHAIN_OUTPUT.json (re-exported from core-state).
pub use core_state::chain_output::CHAIN_OUTPUT_SCHEMA_VERSION;

use crate::types::{
    ChainDagRoot, ChainMode, DagTaskEntry, TaskStatus, CHAIN_DAG_SCHEMA_VERSION,
};

/// Path to TASK_CHAIN.json in the artifacts/current directory.
pub fn chain_file_path(repo_root: &Path) -> PathBuf {
    repo_root.join("artifacts/current/TASK_CHAIN.json")
}

/// Load the current chain file using repo_root path resolution.
/// Auto-detects old linear vs new DAG format.
pub fn load_chain(repo_root: &Path) -> Result<types::ChainDagRoot, FrameworkError> {
    let path = chain_file_path(repo_root);
    if !path.is_file() {
        return Err(FrameworkError::not_found(format!(
            "TASK_CHAIN.json not found at {}",
            path.display()
        )));
    }
    load_chain_from_path(&path)
}

/// Load a TASK_CHAIN.json file from the given path, auto-detecting old linear
/// vs new DAG format and converting if necessary.
pub fn load_chain_from_path(path: &Path) -> Result<ChainDagRoot, FrameworkError> {
    let raw = std::fs::read_to_string(path)?;
    let val: Value = serde_json::from_str(&raw)?;

    if is_old_linear_format(&val) {
        convert_old_linear(val)
    } else {
        serde_json::from_value::<ChainDagRoot>(val).map_err(FrameworkError::Json)
    }
}

/// Try to detect the schema format of a TASK_CHAIN.json payload.
/// Returns true if the value looks like the old linear format.
fn is_old_linear_format(val: &Value) -> bool {
    val.get("current_index").and_then(Value::as_u64).is_some()
        || val.get("schema_version").and_then(Value::as_str) != Some(CHAIN_DAG_SCHEMA_VERSION)
}

/// Convert the old linear format to ChainDagRoot.
fn convert_old_linear(old: Value) -> Result<ChainDagRoot, FrameworkError> {
    let chain_id = old
        .get("chain_id")
        .and_then(Value::as_str)
        .unwrap_or("linear-chain")
        .to_string();

    let tasks = old
        .get("tasks")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|t| {
                    let task_id = t
                        .get("task_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let title = t.get("title").and_then(Value::as_str).map(String::from);
                    let status_str = t
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("pending");
                    let status = match status_str {
                        "running" => TaskStatus::Running,
                        "completed" => TaskStatus::Completed,
                        "failed" => TaskStatus::Failed,
                        _ => TaskStatus::Pending,
                    };
                    DagTaskEntry {
                        task_id,
                        title,
                        status,
                        ..DagTaskEntry::default()
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(ChainDagRoot {
        schema_version: CHAIN_DAG_SCHEMA_VERSION.to_string(),
        chain_id,
        mode: ChainMode::Linear,
        tasks,
        global_config: types::GlobalDagConfig::default(),
        paused: false,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn detect_old_linear_format() {
        let old = json!({
            "tasks": [{"task_id": "a"}, {"task_id": "b"}],
            "current_index": 0
        });
        assert!(is_old_linear_format(&old));
    }

    #[test]
    fn detect_dag_format() {
        let dag = json!({
            "schema_version": "chain-dag-v1",
            "chain_id": "test",
            "mode": "dag",
            "tasks": [{"task_id": "a"}]
        });
        assert!(!is_old_linear_format(&dag));
    }

    #[test]
    fn convert_old_linear_roundtrip() {
        let path = std::env::temp_dir().join("test_old_chain.json");
        let old = json!({
            "chain_id": "my-chain",
            "tasks": [
                {"task_id": "scan", "title": "Scan", "status": "running"},
                {"task_id": "fix", "title": "Fix", "status": "pending"}
            ],
            "current_index": 0
        });
        fs::write(&path, serde_json::to_string_pretty(&old).unwrap()).unwrap();

        let dag = load_chain_from_path(&path).expect("load");
        assert_eq!(dag.chain_id, "my-chain");
        assert_eq!(dag.mode, ChainMode::Linear);
        assert_eq!(dag.tasks.len(), 2);
        assert_eq!(dag.tasks[0].task_id, "scan");
        assert_eq!(dag.tasks[0].status, TaskStatus::Running);
        assert_eq!(dag.tasks[1].status, TaskStatus::Pending);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_dag_format_directly() {
        let path = std::env::temp_dir().join("test_dag_chain.json");
        let dag = ChainDagRoot::new("dag-chain", vec![
            DagTaskEntry::with_title("a", "Task A"),
            DagTaskEntry::with_title("b", "Task B"),
        ]);
        fs::write(&path, serde_json::to_string_pretty(&dag).unwrap()).unwrap();

        let loaded = load_chain_from_path(&path).expect("load");
        assert_eq!(loaded.chain_id, "dag-chain");
        assert_eq!(loaded.mode, ChainMode::Dag);
        assert_eq!(loaded.tasks.len(), 2);

        let _ = fs::remove_file(&path);
    }
}
