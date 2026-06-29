//! Compile-time embedded `ROUTER_RS_HOOK_OBSERVATION_RULES.json` from the repo
//! `configs/framework/` tree (`include_str!` path is relative to `core/router-rs`).

const EXPECTED_SCHEMA: &str = "router-rs-hook-observation-rules-v1";

#[derive(Debug, Clone)]
pub struct GateClassified {
    pub code: String,
    pub human_prefix: String,
}

/// Returns the schema version constant embedded at compile time for
/// `ROUTER_RS_HOOK_OBSERVATION_RULES.json`. Used by integration tests to verify
/// the embedded JSON matches the disk file.
pub fn embedded_schema_version() -> &'static str {
    EXPECTED_SCHEMA
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::OnceLock;

    const RULES_EMBED: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json"
    ));

    #[allow(dead_code)]
    struct ParsedRules {
        tokens: HashMap<String, String>,
        unknown_code: String,
        followup_rules: Vec<Value>,
        additional_rules: Vec<Value>,
    }

    fn parsed_rules() -> &'static ParsedRules {
        static CELL: OnceLock<ParsedRules> = OnceLock::new();
        CELL.get_or_init(|| {
            let root: Value = serde_json::from_str(RULES_EMBED).unwrap_or_else(|e| {
                panic!(
                    "ROUTER_RS_HOOK_OBSERVATION_RULES.json: failed to parse embedded JSON: {}",
                    e
                );
            });
            let sv = root
                .get("schema_version")
                .and_then(Value::as_str)
                .unwrap_or("");
            if sv != EXPECTED_SCHEMA {
                panic!(
                    "ROUTER_RS_HOOK_OBSERVATION_RULES: schema_version mismatch: expected='{}', actual='{}'",
                    EXPECTED_SCHEMA, sv
                );
            }
            let mut tokens = HashMap::new();
            if let Some(obj) = root.get("router_rs_tokens").and_then(Value::as_object) {
                for (k, v) in obj {
                    if let Some(s) = v.as_str() {
                        tokens.insert(k.clone(), s.to_string());
                    }
                }
            }
            let unknown_code = root
                .get("unknown_router_rs_token_code")
                .and_then(Value::as_str)
                .unwrap_or("unknown_router_rs")
                .to_string();
            let followup_rules = root
                .get("followup_first_line_rules")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let additional_rules = root
                .get("additional_context_line_rules")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            ParsedRules {
                tokens,
                unknown_code,
                followup_rules,
                additional_rules,
            }
        })
    }

    #[test]
    fn embedded_rules_parse_and_schema() {
        let _ = parsed_rules();
    }
}
