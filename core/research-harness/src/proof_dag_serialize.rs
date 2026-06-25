//! Blueprint-DAG serialization — schema version `proof-dag-v1`.
//!
//! # Layer boundary
//!
//! FEATURE layer — serialization format belongs with the data model.

use crate::proof_dag::Blueprint;

/// Schema version string written into every serialized DAG.
pub const SERIALIZATION_SCHEMA_VERSION: &str = "proof-dag-v1";

/// Serialize a Blueprint to a pretty-printed JSON string.
pub fn serialize_blueprint(bp: &Blueprint) -> Result<String, String> {
    let wrapper = serde_json::json!({
        "schema_version": SERIALIZATION_SCHEMA_VERSION,
        "blueprint": bp,
    });
    serde_json::to_string_pretty(&wrapper).map_err(|e| format!("serialize: {e}"))
}

/// Deserialize a Blueprint from a JSON string.
pub fn deserialize_blueprint(json: &str) -> Result<Blueprint, String> {
    let wrapper: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("parse: {e}"))?;

    let schema = wrapper.get("schema_version").and_then(|v| v.as_str()).unwrap_or("unknown");
    if schema != SERIALIZATION_SCHEMA_VERSION {
        return Err(format!(
            "schema version mismatch: expected {SERIALIZATION_SCHEMA_VERSION}, got {schema}"
        ));
    }

    let bp: Blueprint = serde_json::from_value(
        wrapper.get("blueprint").ok_or("missing 'blueprint' field")?.clone()
    ).map_err(|e| format!("deserialize blueprint: {e}"))?;

    Ok(bp)
}

/// Apply a JSON patch update to a Blueprint (round payload from an orchestration loop).
pub fn apply_update(bp: &mut Blueprint, update: &serde_json::Value) -> Result<(), String> {
    // The update is expected to be a partial Blueprint or a round payload
    // Currently only re-verify is supported
    if let Some(action) = update.get("action").and_then(|v| v.as_str()) {
        match action {
            "verify" => bp.verify().map_err(|e| format!("verify: {e}")),
            "backtrack" => {
                let node_id = update.get("node_id").and_then(|v| v.as_str())
                    .ok_or("backtrack requires 'node_id'")?;
                bp.backtrack(node_id).map_err(|e| format!("backtrack: {e}"))
            }
            _ => Err(format!("unknown action: {action}")),
        }
    } else {
        Err("update requires 'action' field".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof_dag::{Blueprint, DagNode, VerificationBackend};

    #[test]
    fn test_serialize_roundtrip() {
        let mut bp = Blueprint::new("test goal", "test_name");
        let children = vec![
            DagNode::Leaf { id: "c1".into(), claim: "step 1".into(), backend: VerificationBackend::Z3 },
        ];
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
        assert!(result.unwrap_err().contains("schema version mismatch"));
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
}
