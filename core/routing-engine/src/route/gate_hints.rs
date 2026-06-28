use std::collections::HashMap;
use std::sync::OnceLock;

const PHRASES_EMBED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/gate_hint_phrases.json"
));

const EXPECTED_SCHEMA: &str = "gate-hint-phrases-v1";

fn parsed_phrases() -> &'static HashMap<String, Vec<String>> {
    static CELL: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    CELL.get_or_init(|| {
        let root: serde_json::Value = match serde_json::from_str(PHRASES_EMBED) {
            Ok(v) => v,
            Err(e) => panic!("gate_hint_phrases.json parse: {e}"),
        };
        let sv = root
            .get("schema_version")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        assert_eq!(
            sv, EXPECTED_SCHEMA,
            "gate_hint_phrases.json schema_version mismatch"
        );
        let phrases_val = root
            .get("phrases")
            .unwrap_or_else(|| panic!("missing phrases"));
        match serde_json::from_value(phrases_val.clone()) {
            Ok(v) => v,
            Err(e) => panic!("phrases deserialization: {e}"),
        }
    })
}

pub fn gate_hint_phrases(gate: &str) -> Vec<String> {
    parsed_phrases().get(gate).cloned().unwrap_or_default()
}

pub fn embedded_schema_version() -> &'static str {
    EXPECTED_SCHEMA
}
