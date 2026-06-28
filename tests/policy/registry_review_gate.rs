use crate::common;
use crate::common::{project_root, read_json};
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};

#[test]
fn runtime_registry_review_gate_spawn_first_fields() {
    let root = project_root();
    let registry = read_json(&root.join("configs/framework/RUNTIME_REGISTRY.json"));
    let rg = &registry["review_gate"];
    assert_eq!(rg["spawn_first_enabled"], true);
    let nudge = rg["spawn_first_nudge"]
        .as_str()
        .expect("spawn_first_nudge str");
    assert!(nudge.contains("fork_context"));
    assert!(nudge.contains("配对审稿") || nudge.contains("spawn"));
    assert!(
        !nudge.contains("claude_reviewer_lanes"),
        "global spawn_first_nudge must not mention Claude-only lanes"
    );
    let template = rg["spawn_first_nudge_template"]
        .as_str()
        .expect("spawn_first_nudge_template str");
    assert!(template.contains("{host_label}"));
    let labels = rg["spawn_first_nudge_host_labels"]
        .as_object()
        .expect("spawn_first_nudge_host_labels object");
    assert!(labels.contains_key("claude"));
    let by_host = rg["spawn_first_nudge_by_host"]
        .as_object()
        .expect("spawn_first_nudge_by_host object");
    assert!(by_host.contains_key("cursor"));
    assert!(by_host.contains_key("opencode"));
    assert!(
        !by_host.contains_key("codex"),
        "codex uses template+label, not per-host override"
    );
    let cursor_line = by_host["cursor"].as_str().expect("cursor nudge str");
    assert!(
        !cursor_line.contains("claude_reviewer_lanes"),
        "cursor spawn nudge must not mention Claude-only lanes"
    );
    let claude_label = labels["claude"].as_str().expect("claude label");
    let claude_line = template.replace("{host_label}", claude_label);
    assert!(
        claude_line.contains("review") || claude_line.contains("Claude"),
        "claude template nudge should mention Claude"
    );
}

#[test]
fn runtime_registry_host_projections_split_harness_capabilities() {
    let schema = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY_SCHEMA.json"));
    let policy = schema
        .get("harness_capability_policy")
        .and_then(Value::as_object)
        .expect("RUNTIME_REGISTRY_SCHEMA must define harness_capability_policy");
    let core_always: Vec<&str> = policy["core_always"]
        .as_array()
        .expect("harness_capability_policy.core_always")
        .iter()
        .map(|v| v.as_str().expect("core_always token"))
        .collect();
    let hook_baseline: Vec<&str> = policy["cli_agent_hook_baseline"]
        .as_array()
        .expect("harness_capability_policy.cli_agent_hook_baseline")
        .iter()
        .map(|v| v.as_str().expect("cli_agent_hook_baseline token"))
        .collect();
    assert!(
        !core_always.is_empty(),
        "harness_capability_policy.core_always must be non-empty"
    );
    assert!(
        !hook_baseline.is_empty(),
        "harness_capability_policy.cli_agent_hook_baseline must be non-empty"
    );
    let allowed_exception_status: HashSet<&str> =
        schema["harness_capability_exception_status_values"]
            .as_array()
            .expect("schema harness_capability_exception_status_values")
            .iter()
            .map(|v| v.as_str().expect("exception status token"))
            .collect();

    let runtime = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let projections = runtime["host_projections"]
        .as_object()
        .expect("host_projections");

    for (host_id, proj) in projections {
        let harness = proj
            .get("harness_capabilities")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{host_id}: missing harness_capabilities"));
        assert!(
            !harness.is_empty(),
            "{host_id}: harness_capabilities must be non-empty"
        );
        for cap in harness {
            let s = cap.as_str().expect("harness capability must be string");
            assert!(!s.is_empty(), "{host_id}: empty harness token");
            assert_ne!(
                s, "mcp_servers",
                "{host_id}: mcp_servers belongs in product capabilities, not harness_capabilities"
            );
        }
        let harness_set: BTreeSet<_> = harness.iter().filter_map(|v| v.as_str()).collect();

        for token in &core_always {
            assert!(
                harness_set.contains(token),
                "{host_id}: harness_capabilities must include `{token}` (core baseline)"
            );
        }

        let exceptions = proj
            .get("harness_capability_exceptions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut exempt_hook: BTreeSet<String> = BTreeSet::new();
        for ex in &exceptions {
            let obj = ex.as_object().unwrap_or_else(|| {
                panic!("{host_id}: harness_capability_exceptions entries must be objects")
            });
            let cap = obj
                .get("cap")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{host_id}: harness_capability_exceptions.cap required"));
            let status = obj
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    panic!("{host_id}: harness_capability_exceptions.status required for cap={cap}")
                });
            let rationale = obj.get("rationale").and_then(Value::as_str).unwrap_or("");
            assert!(
                !rationale.trim().is_empty(),
                "{host_id}: harness_capability_exceptions[{cap}] must include non-empty rationale"
            );
            assert!(
                allowed_exception_status.contains(status),
                "{host_id}: unknown harness_capability_exceptions.status `{status}` for cap={cap}"
            );
            assert!(
                hook_baseline.contains(&cap),
                "{host_id}: may only declare exceptions for cli_agent_hook_baseline tokens, not `{cap}`"
            );
            assert!(
                !core_always.contains(&cap),
                "{host_id}: core harness tokens cannot be listed in harness_capability_exceptions"
            );
            assert!(
                exempt_hook.insert(cap.to_string()),
                "{host_id}: duplicate harness_capability_exceptions.cap `{cap}`"
            );
        }

        for cap in &hook_baseline {
            if exempt_hook.contains(*cap) {
                assert!(
                    !harness_set.contains(cap),
                    "{host_id}: `{cap}` is listed as unsupported in harness_capability_exceptions and must not appear in harness_capabilities"
                );
            } else {
                assert!(
                    harness_set.contains(cap),
                    "{host_id}: harness_capabilities must include `{cap}` or declare it under harness_capability_exceptions"
                );
            }
        }
    }
}

#[test]
fn runtime_registry_review_gate_reviewer_lanes_non_empty() {
    let v = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let lanes = v["review_gate"]["reviewer_lanes"]
        .as_array()
        .expect("review_gate.reviewer_lanes must be array");
    assert!(
        !lanes.is_empty(),
        "review_gate.reviewer_lanes must list cross-host countable review lanes"
    );
}

#[test]
fn runtime_registry_review_gate_lane_sets_closed() {
    let v = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    let lanes = common::reviewer_lanes_from_registry(&v);
    common::assert_reviewer_lanes_closed(&lanes);
}

#[test]
fn runtime_registry_schema_covers_execution_critical_fields_only() {
    let schema = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY_SCHEMA.json"));
    let registry = read_json(&project_root().join("configs/framework/RUNTIME_REGISTRY.json"));
    assert_eq!(
        schema["schema_version"],
        "framework-runtime-registry-schema-v1"
    );
    assert!(
        schema.get("harness_capability_policy").is_some(),
        "schema must define harness_capability_policy for host harness baseline tests"
    );
    assert!(
        schema
            .get("harness_capability_exception_status_values")
            .is_some(),
        "schema must define harness_capability_exception_status_values"
    );
    let allowed_hosts = schema["host_ids"]
        .as_array()
        .expect("schema host_ids")
        .iter()
        .filter_map(Value::as_str)
        .collect::<HashSet<_>>();
    let allowed_status = schema["projection_status_values"]
        .as_array()
        .expect("schema projection_status_values")
        .iter()
        .filter_map(Value::as_str)
        .collect::<HashSet<_>>();
    let required = schema["required_host_metadata_fields"]
        .as_array()
        .expect("schema required fields")
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let hosts = registry["host_targets"]["supported"]
        .as_array()
        .expect("runtime supported hosts");
    for host in hosts {
        let host = host.as_str().expect("host id");
        assert!(
            allowed_hosts.contains(host),
            "host id outside schema: {host}"
        );
        let metadata = &registry["host_targets"]["metadata"][host];
        for field in &required {
            assert!(
                !metadata[*field].is_null(),
                "host metadata missing execution field {host}.{field}"
            );
        }
        let status = metadata["projection_status"]
            .as_str()
            .expect("projection_status");
        assert!(
            allowed_status.contains(status),
            "invalid projection_status {status} for {host}"
        );
    }

    let review_gate = registry
        .get("review_gate")
        .and_then(Value::as_object)
        .expect("RUNTIME_REGISTRY must include review_gate object");
    let required_review_gate = schema["required_review_gate_fields"]
        .as_array()
        .expect("schema required_review_gate_fields");
    for field in required_review_gate {
        let field = field.as_str().expect("required_review_gate_fields entry");
        let lanes = review_gate
            .get(field)
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("review_gate.{field} must be array"));
        assert!(!lanes.is_empty(), "review_gate.{field} must be non-empty");
        for item in lanes {
            assert!(
                item.as_str().is_some(),
                "review_gate.{field} entries must be strings"
            );
        }
    }
}
