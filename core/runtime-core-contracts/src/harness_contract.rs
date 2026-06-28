use serde_json::{Value, json};

pub(crate) const HARNESS_CONTRACT_SCHEMA_VERSION: &str = "router-rs-harness-contract-v1";

pub use framework_kernel::skill_lint::{
    FAILURE_TAXONOMY, HARNESS_CONTRACT_AUTHORITY, HARNESS_SKILL_LINT_SCHEMA_VERSION,
    failure_taxonomy_values, lint_skill_contracts,
};

pub fn harness_contract() -> Value {
    json!({
        "schema_version": HARNESS_CONTRACT_SCHEMA_VERSION,
        "authority": HARNESS_CONTRACT_AUTHORITY,
        "failure_taxonomy": failure_taxonomy_values(),
        "trajectory_event_convention": {
            "sink": "TRACE_EVENTS.jsonl via trace_runtime record-event",
            "required_payload_fields": [
                "task_id",
                "owner",
                "gate",
                "overlay",
                "horizon",
                "phase",
                "tool_or_lane",
                "status",
                "failure_class",
                "evidence_ref",
                "context_bytes"
            ],
            "model_context_policy": "Persist full trajectory events; inject only summaries, cursors, or evidence refs."
        },
        "behavioral_eval_tracks": [
            "routing_accuracy",
            "token_efficiency",
            "long_task_continuity",
            "trajectory_health",
            "closeout_integrity",
            "skill_contract_quality",
            "subagent_lane_integrity",
            "review_gate_integrity",
            "contract_integrity"
        ],
        "step_recovery": {
            "ledger": "STEP_LEDGER.jsonl",
            "summary_projection": "TASK_STATE.json.step_ledger",
            "canonical_writer": "router-rs framework step-ledger"
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn harness_contract_lists_failure_taxonomy() {
        let contract = harness_contract();
        let ids = contract["failure_taxonomy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"verification_missing"));
        assert!(ids.contains(&"step_recovery_gap"));
    }

    #[test]
    fn skill_lint_reports_existing_high_impact_shape() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills");
        let report = lint_skill_contracts(json!({
            "skills_root": root.display().to_string(),
            "slugs": ["skill-framework-developer"]
        }))
        .expect("lint");
        assert_eq!(report["schema_version"], HARNESS_SKILL_LINT_SCHEMA_VERSION);
        assert_eq!(report["skills_scanned"][0], "skill-framework-developer");
        assert!(report["findings"].as_array().is_some());
    }

    #[test]
    fn debug_failure_taxonomy_constants() {
        insta::assert_debug_snapshot!(FAILURE_TAXONOMY);
    }

    #[test]
    fn debug_failure_taxonomy_values() {
        insta::assert_debug_snapshot!(failure_taxonomy_values());
    }

    #[test]
    fn debug_harness_contract_output() {
        insta::assert_debug_snapshot!(harness_contract());
    }
}
