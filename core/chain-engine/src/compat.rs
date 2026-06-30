//! Backward compatibility layer for loading the old linear TASK_CHAIN.json
//! format and converting it to ChainDagRoot.
//!
//! The old format:
//! ```json
//! { "tasks": [{ "task_id": "...", "title": "...", "status": "..." }],
//!   "current_index": 0 }
//! ```

use core_errors::FrameworkError;
use serde_json::Value;

use crate::types::{
    ChainDagRoot, ChainMode, DagTaskEntry, TaskStatus, CHAIN_DAG_SCHEMA_VERSION,
};

/// Try to detect the schema format of a TASK_CHAIN.json payload.
/// Returns true if the value looks like the old linear format.
pub fn is_old_linear_format(val: &Value) -> bool {
    // Old format has `current_index` and a `tasks` array with objects that
    // don't have DAG-specific fields.
    val.get("current_index").and_then(Value::as_u64).is_some()
        || val.get("schema_version").and_then(Value::as_str) != Some(CHAIN_DAG_SCHEMA_VERSION)
}

/// Load a TASK_CHAIN.json file, automatically detecting and converting
/// the old linear format to ChainDagRoot.
pub fn load_chain_file(path: &std::path::Path) -> Result<ChainDagRoot, FrameworkError> {
    let raw = std::fs::read_to_string(path)?;
    let val: Value = serde_json::from_str(&raw)?;

    if is_old_linear_format(&val) {
        convert_old_linear(val)
    } else {
        serde_json::from_value::<ChainDagRoot>(val)
            .map_err(|e| FrameworkError::Json(e))
    }
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
        global_config: crate::types::GlobalDagConfig::default(),
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

        let dag = load_chain_file(&path).expect("load");
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

        let loaded = load_chain_file(&path).expect("load");
        assert_eq!(loaded.chain_id, "dag-chain");
        assert_eq!(loaded.mode, ChainMode::Dag);
        assert_eq!(loaded.tasks.len(), 2);

        let _ = fs::remove_file(&path);
    }
}
