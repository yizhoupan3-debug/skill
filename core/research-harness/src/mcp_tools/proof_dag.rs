//! Proof DAG MCP tools.
//!
//! DAGs are stored in a name-keyed HashMap for basic session isolation.
//! Each tool accepts an optional `name` argument (defaults to "default").
//! Callers that may run concurrent proof sessions MUST pass distinct names.
//!
//! Each function in this module is `pub(super)` — visible only to the parent
//! `mcp_tools` module (i.e. `mod.rs`), which routes calls via
//! [`math_tool_dispatch`](super::math_tool_dispatch).

use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Maximum number of elements in any array-type parameter to a research tool.
const MAX_ARRAY_ELEMENTS: usize = 10_000;

fn get_or_create_dag_store()
-> &'static std::sync::Mutex<HashMap<String, crate::proof_dag::Blueprint>> {
    static STORE: OnceLock<std::sync::Mutex<HashMap<String, crate::proof_dag::Blueprint>>> =
        OnceLock::new();
    STORE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Extract the DAG name from tool arguments (defaults to "default").
fn dag_name(arguments: &Value) -> String {
    arguments
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("default")
        .to_string()
}

pub(super) fn tool_math_proof_dag_init(arguments: &Value) -> Result<String, FrameworkError> {
    let goal = arguments
        .get("goal")
        .and_then(Value::as_str)
        .ok_or(FrameworkError::validation(
            "math_proof_dag_init requires 'goal' (string)",
        ))?;
    let name = arguments
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("proof");
    let dag_id = dag_name(arguments);
    let bp = crate::proof_dag::Blueprint::new(goal, name);
    let serialized = crate::proof_dag_serialize::serialize_blueprint(&bp)?;
    if let Ok(mut guard) = get_or_create_dag_store().lock() {
        const MAX_DAGS: usize = 64;
        if guard.len() >= MAX_DAGS && !guard.contains_key(&dag_id) {
            return Err(FrameworkError::validation(format!(
                "DAG store limit reached ({MAX_DAGS}). Delete unused DAGs first."
            )));
        }
        guard.insert(dag_id, bp);
    }
    Ok(serialized)
}

pub(super) fn tool_math_proof_dag_decompose(arguments: &Value) -> Result<String, FrameworkError> {
    let parent_id =
        arguments
            .get("parent_id")
            .and_then(Value::as_str)
            .ok_or(FrameworkError::validation(
                "math_proof_dag_decompose requires 'parent_id'",
            ))?;
    let and = arguments
        .get("and")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let children_val =
        arguments
            .get("children")
            .and_then(Value::as_array)
            .ok_or(FrameworkError::validation(
                "math_proof_dag_decompose requires 'children' array",
            ))?;

    if children_val.len() > MAX_ARRAY_ELEMENTS {
        return Err(FrameworkError::validation(format!(
            "children array too large: {} elements (max {MAX_ARRAY_ELEMENTS})",
            children_val.len()
        )));
    }
    let children: Vec<crate::proof_dag::DagNode> =
        serde_json::from_value(Value::Array(children_val.clone()))
            .map_err(|e| FrameworkError::Json(e))?;
    let dag_id = dag_name(arguments);
    let mut guard = get_or_create_dag_store()
        .lock()
        .map_err(|e| FrameworkError::session(format!("lock: {e}")))?;
    let bp = guard.get_mut(&dag_id).ok_or(FrameworkError::validation(
        "no proof DAG for this name — call math_proof_dag_init first",
    ))?;
    bp.decompose(parent_id, children, and)?;
    Ok(crate::proof_dag_serialize::serialize_blueprint(bp)?)
}

pub(super) fn tool_math_proof_dag_verify(arguments: &Value) -> Result<String, FrameworkError> {
    let dag_id = dag_name(arguments);
    let mut guard = get_or_create_dag_store()
        .lock()
        .map_err(|e| FrameworkError::session(format!("lock: {e}")))?;
    let bp = guard.get_mut(&dag_id).ok_or(FrameworkError::validation(
        "no proof DAG for this name — call math_proof_dag_init first",
    ))?;
    bp.verify()?;
    if let Err(warning) = bp.validate_manual_prose_ratio(0.30) {
        let summary = bp.status_summary();
        return serde_json::to_string_pretty(&json!({
            "result": summary,
            "manual_prose_warning": warning.to_string(),
        }))
        .map_err(FrameworkError::Json);
    }
    Ok(crate::proof_dag_serialize::serialize_blueprint(bp)?)
}

pub(super) fn tool_math_proof_dag_status(arguments: &Value) -> Result<String, FrameworkError> {
    let dag_id = dag_name(arguments);
    let guard = get_or_create_dag_store()
        .lock()
        .map_err(|e| FrameworkError::session(format!("lock: {e}")))?;
    let bp = guard.get(&dag_id).ok_or(FrameworkError::validation(
        "no proof DAG for this name — call math_proof_dag_init first",
    ))?;
    let summary = bp.status_summary();
    serde_json::to_string_pretty(&summary).map_err(FrameworkError::Json)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::super::handle_research_tool;
    use serde_json::{Value, json};

    #[test]
    fn test_math_proof_dag_init_missing_goal() {
        let result = handle_research_tool("math_proof_dag_init", &json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires 'goal'"));
    }

    #[test]
    fn test_math_proof_dag_decompose_missing_parent_id() {
        let result = handle_research_tool("math_proof_dag_decompose", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires 'parent_id'")
        );
    }

    #[test]
    fn test_math_proof_dag_verify_without_init() {
        let result = handle_research_tool("math_proof_dag_verify", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no proof DAG for this name")
        );
    }

    #[test]
    fn test_math_proof_dag_status_without_init() {
        let result = handle_research_tool("math_proof_dag_status", &json!({}));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("no proof DAG for this name")
        );
    }

    #[test]
    fn test_math_proof_dag_init_decompose_verify_full_lifecycle() {
        // Step 1: Init — create a new proof DAG
        let result = handle_research_tool(
            "math_proof_dag_init",
            &json!({"goal": "Prove x > 0", "name": "happy-test"}),
        );
        assert!(result.is_ok(), "init failed: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("schema_version").and_then(Value::as_str),
            Some("proof-dag-v1")
        );
        assert_eq!(
            parsed.pointer("/blueprint/name").and_then(Value::as_str),
            Some("happy-test")
        );
        assert_eq!(
            parsed.pointer("/blueprint/goal").and_then(Value::as_str),
            Some("Prove x > 0")
        );

        // Step 2: Decompose — add an OrNode child to the root
        let result = handle_research_tool(
            "math_proof_dag_decompose",
            &json!({
                "parent_id": "root",
                "name": "happy-test",
                "and": false,
                "children": [
                    {"OrNode": {"id": "approach-a", "label": "Via inequality engine", "children": []}},
                ],
            }),
        );
        assert!(result.is_ok(), "decompose failed: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.pointer("/blueprint/name").and_then(Value::as_str),
            Some("happy-test")
        );
        assert_eq!(
            parsed.pointer("/blueprint/goal").and_then(Value::as_str),
            Some("Prove x > 0")
        );

        // Step 3: Decompose again — add a Leaf node under approach-a
        let result = handle_research_tool(
            "math_proof_dag_decompose",
            &json!({
                "parent_id": "approach-a",
                "name": "happy-test",
                "and": true,
                "children": [
                    {"Leaf": {"id": "leaf-x", "claim": "x > 0", "backend": "InequalityEngine"}},
                ],
            }),
        );
        assert!(result.is_ok(), "second decompose failed: {:?}", result.err());

        // Step 4: Verify — run verification traversal
        let result = handle_research_tool(
            "math_proof_dag_verify",
            &json!({"name": "happy-test"}),
        );
        assert!(result.is_ok(), "verify failed: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        if parsed.get("schema_version").is_some() {
            assert_eq!(
                parsed.pointer("/blueprint/name").and_then(Value::as_str),
                Some("happy-test")
            );
            assert!(parsed.pointer("/blueprint/round").and_then(Value::as_u64).unwrap_or(0) >= 1);
        } else if parsed.get("manual_prose_warning").is_some() {
            let result = parsed.get("result").unwrap();
            assert_eq!(
                result.get("name").and_then(Value::as_str),
                Some("happy-test")
            );
        } else {
            panic!("unexpected verify output format: {parsed}");
        }

        // Step 5: Status — get summary
        let result = handle_research_tool(
            "math_proof_dag_status",
            &json!({"name": "happy-test"}),
        );
        assert!(result.is_ok(), "status failed: {:?}", result.err());
        let parsed: Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(
            parsed.get("name").and_then(Value::as_str),
            Some("happy-test")
        );
        assert!(parsed.get("node_count").and_then(Value::as_u64).unwrap_or(0) >= 3);
    }
}
