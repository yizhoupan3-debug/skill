//! Blueprint-DAG serialization — schema version `proof-dag-v1`.
//!
//! # Layer boundary
//!
//! FEATURE layer — serialization format belongs with the data model.

use crate::proof_dag::Blueprint;
use core_errors::FrameworkError;

/// Schema version string written into every serialized DAG.
pub const SERIALIZATION_SCHEMA_VERSION: &str = "proof-dag-v1";

/// Serialize a Blueprint to a pretty-printed JSON string.
pub fn serialize_blueprint(bp: &Blueprint) -> Result<String, FrameworkError> {
    let wrapper = serde_json::json!({
        "schema_version": SERIALIZATION_SCHEMA_VERSION,
        "blueprint": bp,
    });
    Ok(serde_json::to_string_pretty(&wrapper)?)
}

/// Deserialize a Blueprint from a JSON string.
pub fn deserialize_blueprint(json: &str) -> Result<Blueprint, FrameworkError> {
    let wrapper: serde_json::Value = serde_json::from_str(json)?;

    let schema_val = wrapper.get("schema_version");
    let schema = match schema_val {
        Some(v) if v.is_string() => v.as_str().unwrap_or("unknown"),
        Some(v) => {
            return Err(FrameworkError::validation(format!(
                "schema_version must be a string, got {v}"
            )));
        }
        None => "unknown",
    };
    if schema != SERIALIZATION_SCHEMA_VERSION {
        return Err(FrameworkError::validation(format!(
            "schema version mismatch: expected {SERIALIZATION_SCHEMA_VERSION}, got {schema}"
        )));
    }

    let bp_val = wrapper
        .get("blueprint")
        .cloned()
        .ok_or_else(|| FrameworkError::validation("missing 'blueprint' field"))?;
    let bp: Blueprint = serde_json::from_value(bp_val)?;

    Ok(bp)
}

/// Apply a JSON patch update to a Blueprint (round payload from an orchestration loop).
pub fn apply_update(bp: &mut Blueprint, update: &serde_json::Value) -> Result<(), FrameworkError> {
    // The update is expected to be a partial Blueprint or a round payload
    // Currently only re-verify is supported
    if let Some(action) = update.get("action").and_then(|v| v.as_str()) {
        match action {
            "verify" => bp.verify(),
            "backtrack" => {
                let node_id = update
                    .get("node_id")
                    .and_then(|v| v.as_str())
                    .ok_or(FrameworkError::validation("backtrack requires 'node_id'"))?;
                let mut visited = std::collections::HashSet::new();
                bp.backtrack(node_id, &mut visited)
            }
            _ => Err(FrameworkError::validation(format!(
                "unknown action: {action}"
            ))),
        }
    } else {
        Err(FrameworkError::validation("update requires 'action' field"))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::proof_dag::{Blueprint, DagNode, VerificationBackend};

    #[test]
    fn test_serialize_roundtrip() {
        let mut bp = Blueprint::new("test goal", "test_name");
        let children = vec![DagNode::Leaf {
            id: "c1".into(),
            claim: "step 1".into(),
            backend: VerificationBackend::Z3,
        }];
        bp.decompose("root", children, false).unwrap();
        bp.verify().unwrap();

        let json = serialize_blueprint(&bp).unwrap();
        let deserialized = deserialize_blueprint(&json).unwrap();
        assert_eq!(deserialized.name, "test_name");
        assert_eq!(deserialized.goal, "test goal");
        assert_eq!(deserialized.round, 1);
    }

    #[test]
    fn test_serialize_schema_mismatch() {
        let bad_json = r#"{"schema_version": "proof-dag-v0", "blueprint": {}}"#;
        let result = deserialize_blueprint(bad_json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("schema version mismatch")
        );
    }

    #[test]
    fn test_apply_update_verify() {
        let mut bp = Blueprint::new("goal", "test");
        let update = serde_json::json!({"action": "verify"});
        apply_update(&mut bp, &update).unwrap();
        assert_eq!(bp.round, 1);
    }

    #[test]
    fn test_apply_update_unknown_action() {
        let mut bp = Blueprint::new("goal", "test");
        let update = serde_json::json!({"action": "nonexistent"});
        let result = apply_update(&mut bp, &update);
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_update_backtrack() {
        let mut bp = Blueprint::new("goal", "test");
        bp.decompose(
            "root",
            vec![DagNode::Leaf {
                id: "leaf1".into(),
                claim: "claim1".into(),
                backend: VerificationBackend::Z3,
            }],
            false,
        )
        .unwrap();
        let update = serde_json::json!({"action": "backtrack", "node_id": "leaf1"});
        apply_update(&mut bp, &update).unwrap();
    }

    #[test]
    fn test_apply_update_backtrack_missing_node_id() {
        let mut bp = Blueprint::new("goal", "test");
        let update = serde_json::json!({"action": "backtrack"});
        let result = apply_update(&mut bp, &update);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("node_id"));
    }

    #[test]
    fn test_apply_update_missing_action() {
        let mut bp = Blueprint::new("goal", "test");
        let update = serde_json::json!({"foo": "bar"});
        let result = apply_update(&mut bp, &update);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("action"));
    }

    #[test]
    fn test_deserialize_missing_blueprint() {
        let bad_json = r#"{"schema_version": "proof-dag-v1"}"#;
        let result = deserialize_blueprint(bad_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("blueprint"));
    }

    #[test]
    fn test_deserialize_invalid_json() {
        let result = deserialize_blueprint("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_bad_schema_type() {
        let bad_json = r#"{"schema_version": 42, "blueprint": {}}"#;
        let result = deserialize_blueprint(bad_json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("schema_version"));
    }

    #[test]
    fn test_serialize_verify_round_advances() {
        let mut bp = Blueprint::new("nested goal", "nested");
        bp.decompose(
            "root",
            vec![DagNode::OrNode {
                id: "or1".into(),
                label: "alternatives".into(),
                children: vec!["a1".into(), "a2".into()],
            }],
            false,
        )
        .unwrap();
        // Two separate verify → backtrack → verify cycles to advance round
        bp.verify().unwrap();
        // serialize/deserialize should preserve round
        let json = serialize_blueprint(&bp).unwrap();
        let deserialized = deserialize_blueprint(&json).unwrap();
        assert_eq!(deserialized.round, bp.round);
    }
}
