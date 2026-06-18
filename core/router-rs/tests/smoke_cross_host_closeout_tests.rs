//! cross-host closeout + evidence write consistency (registry-driven).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::closeout_enforcement::{
    CLOSEOUT_ENFORCEMENT_AUTHORITY, CLOSEOUT_RECORD_SCHEMA_VERSION, closeout_enforcement_contract,
    evaluate_closeout_record_value,
};
use crate::framework_host_targets::host_targets_supported_host_ids;
use crate::framework_runtime::stdio_dispatch::dispatch_stdio_json_request;
use crate::framework_runtime::{
    FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY, framework_hook_evidence_append,
    write_framework_session_artifacts,
};
use crate::hook_event_routing::{
    HOOK_EVENT_ROUTING_AUTHORITY, HOOK_EVENT_ROUTING_SCHEMA_VERSION, canonical_hook_event,
    hook_event_routing_contract, routable_lifecycle_events,
};
use crate::hosts::host_provider_for_id;
use crate::runtime_registry::load_runtime_registry_json;

fn framework_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn smoke_temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-smoke-{name}-{nonce}"))
}

fn seed_smoke_task_repo(task_id: &str) -> PathBuf {
    let repo_root = smoke_temp_dir("cross-host-evidence");
    let output_dir = repo_root.join("artifacts/current");
    write_framework_session_artifacts(json!({
        "repo_root": repo_root,
        "output_dir": output_dir,
        "task_id": task_id,
        "task": "cross-host evidence smoke",
        "phase": "implementation",
        "status": "in_progress",
        "summary": "seed",
        "focus": true,
        "next_actions": ["Append evidence per host"]
    }))
    .expect("seed session artifacts");
    repo_root
}

fn read_evidence_index(repo_root: &Path, task_id: &str) -> Value {
    let evidence_path = repo_root
        .join("artifacts/current")
        .join(task_id)
        .join("EVIDENCE_INDEX.json");
    serde_json::from_str(&fs::read_to_string(&evidence_path).expect("read EVIDENCE_INDEX"))
        .expect("parse EVIDENCE_INDEX")
}

fn canonical_closeout_fixture() -> Value {
    json!({
        "schema_version": CLOSEOUT_RECORD_SCHEMA_VERSION,
        "task_id": "smoke-cross-host-closeout",
        "summary": "已完成 router-rs smoke",
        "verification_status": "not_run",
    })
}

/// Registry host ids share one closeout evaluator contract (no per-host rule tables).
#[test]
fn cross_host_closeout_behavior_consistency_smoke() {
    let root = framework_repo_root();
    let registry = load_runtime_registry_json(&root).expect("load RUNTIME_REGISTRY");
    let host_ids = host_targets_supported_host_ids(&registry).expect("host_targets.supported");
    assert!(
        !host_ids.is_empty(),
        "expected at least one supported host in RUNTIME_REGISTRY"
    );

    let contract = closeout_enforcement_contract();
    let stdio_contract = dispatch_stdio_json_request("closeout_contract", json!({}))
        .expect("stdio closeout_contract");
    assert_eq!(
        contract, stdio_contract,
        "stdio closeout_contract must match in-process evaluator contract"
    );
    assert_eq!(
        contract["authority"].as_str(),
        Some(CLOSEOUT_ENFORCEMENT_AUTHORITY)
    );
    assert_eq!(
        contract["record_schema_version"].as_str(),
        Some(CLOSEOUT_RECORD_SCHEMA_VERSION)
    );

    let fixture = canonical_closeout_fixture();
    let baseline =
        evaluate_closeout_record_value(fixture.clone()).expect("baseline closeout evaluate");
    assert_eq!(
        baseline["closeout_allowed"],
        json!(false),
        "fixture must block unverified completion"
    );
    assert!(
        baseline["violations"]
            .as_array()
            .expect("violations")
            .iter()
            .any(|v| v["rule"] == "claimed_done_without_evidence")
    );

    let stdio_baseline = dispatch_stdio_json_request("closeout_evaluate", fixture.clone())
        .expect("stdio closeout_evaluate");
    assert_eq!(
        baseline["closeout_allowed"], stdio_baseline["closeout_allowed"],
        "stdio evaluate must match in-process evaluate"
    );
    assert_eq!(
        baseline["authority"], stdio_baseline["authority"],
        "authority must not vary by transport"
    );

    for host_id in &host_ids {
        let provider = host_provider_for_id(host_id)
            .unwrap_or_else(|| panic!("missing HostProvider for registry host {host_id:?}"));
        assert_eq!(
            provider.host_id(),
            host_id.as_str(),
            "HostProvider id must match registry spelling"
        );

        // Closeout schema is host-agnostic: evaluation outcome identical for every supported host.
        let eval = evaluate_closeout_record_value(fixture.clone())
            .unwrap_or_else(|err| panic!("evaluate for host {host_id}: {err}"));
        assert_eq!(
            eval["closeout_allowed"], baseline["closeout_allowed"],
            "closeout_allowed drift for host {host_id}"
        );
        assert_eq!(
            eval["authority"], baseline["authority"],
            "authority drift for host {host_id}"
        );

        let stdio_eval = dispatch_stdio_json_request(
            "closeout_evaluate",
            json!({
                "record": fixture,
                "host_id": host_id,
            }),
        )
        .expect("stdio closeout_evaluate with host_id annotation");
        assert_eq!(
            stdio_eval["closeout_allowed"], baseline["closeout_allowed"],
            "stdio path must ignore host_id for evaluator semantics (host {host_id})"
        );
    }

    let keywords = contract["completion_keywords"]
        .as_array()
        .expect("completion_keywords array");
    assert!(
        !keywords.is_empty(),
        "shared completion keyword table must be non-empty for all hook surfaces"
    );
}

/// Registry host ids share one evidence append contract (schema + authority; no per-host writers).
#[test]
fn cross_host_evidence_write_consistency_smoke() {
    let root = framework_repo_root();
    let registry = load_runtime_registry_json(&root).expect("load RUNTIME_REGISTRY");
    let host_ids = host_targets_supported_host_ids(&registry).expect("host_targets.supported");
    assert!(
        !host_ids.is_empty(),
        "expected at least one supported host in RUNTIME_REGISTRY"
    );

    let task_id = "smoke-cross-host-evidence";
    let repo_root = seed_smoke_task_repo(task_id);
    let repo_display = repo_root.display().to_string();

    let baseline_append = framework_hook_evidence_append(json!({
        "repo_root": repo_root,
        "task_id": task_id,
        "command_preview": "cargo test -p router-rs",
        "exit_code": 0,
        "source": "smoke_baseline",
    }))
    .expect("baseline evidence append");
    assert_eq!(baseline_append["ok"], json!(true));
    assert_eq!(baseline_append["skipped"], json!(false));
    assert_eq!(
        baseline_append["authority"].as_str(),
        Some(FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY)
    );
    assert_eq!(
        baseline_append["schema_version"].as_str(),
        Some("router-rs-hook-evidence-append-v1")
    );

    let baseline_evidence = read_evidence_index(&repo_root, task_id);
    assert_eq!(
        baseline_evidence["schema_version"].as_str(),
        Some("evidence-index-v2")
    );
    let baseline_row = baseline_evidence["artifacts"][0].clone();

    let projections = registry
        .get("host_projections")
        .and_then(Value::as_object)
        .expect("host_projections");

    for host_id in &host_ids {
        let provider = host_provider_for_id(host_id)
            .unwrap_or_else(|| panic!("missing HostProvider for registry host {host_id:?}"));
        let projection = projections
            .get(host_id.as_str())
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing projection for host {host_id}"));
        let registry_closeout = projection
            .get("harness_capabilities")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .any(|v| v.as_str() == Some("closeout_evidence_hooks"))
            })
            .unwrap_or(false)
            && !projection
                .get("harness_capability_exceptions")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter().any(|row| {
                        row.get("cap").and_then(Value::as_str) == Some("closeout_evidence_hooks")
                            && row.get("status").and_then(Value::as_str) == Some("unsupported")
                    })
                })
                .unwrap_or(false);
        assert_eq!(
            provider.closeout_evidence_hooks_supported(),
            registry_closeout,
            "closeout_evidence_hooks drift for host {host_id}"
        );

        let source = format!("{host_id}_smoke");
        let command_preview = format!("cargo test -p router-rs -- smoke-host-{host_id}");
        let append = framework_hook_evidence_append(json!({
            "repo_root": repo_root,
            "task_id": task_id,
            "command_preview": command_preview,
            "exit_code": 0,
            "source": source,
            "host_id": host_id,
        }))
        .unwrap_or_else(|err| panic!("evidence append for host {host_id}: {err}"));
        assert_eq!(append["ok"], baseline_append["ok"], "host {host_id}");
        assert_eq!(
            append["skipped"], baseline_append["skipped"],
            "host {host_id}"
        );
        assert_eq!(
            append["authority"], baseline_append["authority"],
            "authority drift for host {host_id}"
        );
        assert_eq!(
            append["schema_version"], baseline_append["schema_version"],
            "schema_version drift for host {host_id}"
        );

        let stdio_append = dispatch_stdio_json_request(
            "framework_hook_evidence_append",
            json!({
                "repo_root": repo_display,
                "task_id": task_id,
                "command_preview": format!("cargo test -q -- smoke-stdio-{host_id}"),
                "exit_code": 0,
                "source": format!("{host_id}_stdio"),
                "host_id": host_id,
            }),
        )
        .expect("stdio evidence append");
        assert_eq!(
            stdio_append["authority"], baseline_append["authority"],
            "stdio authority drift for host {host_id}"
        );
        assert_eq!(
            stdio_append["schema_version"], baseline_append["schema_version"],
            "stdio schema drift for host {host_id}"
        );
    }

    let evidence = read_evidence_index(&repo_root, task_id);
    assert_eq!(
        evidence["schema_version"].as_str(),
        Some("evidence-index-v2"),
        "EVIDENCE_INDEX schema must not vary by host"
    );
    let artifacts = evidence["artifacts"].as_array().expect("artifacts array");
    assert!(
        artifacts.len() > host_ids.len() * 2,
        "expected baseline + per-host direct/stdio rows"
    );
    for row in artifacts {
        assert_eq!(row["kind"], baseline_row["kind"]);
        assert!(row.get("command_preview").is_some());
        assert!(row.get("recorded_at").is_some());
        assert!(row.get("success").is_some());
    }

    let _ = fs::remove_dir_all(&repo_root);
}

/// Registry host ids share one hook lifecycle routing contract (canonical map + telemetry surface).
#[test]
fn cross_host_hook_event_routing_smoke() {
    let root = framework_repo_root();
    let registry = load_runtime_registry_json(&root).expect("load RUNTIME_REGISTRY");
    let host_ids = host_targets_supported_host_ids(&registry).expect("host_targets.supported");
    assert!(
        !host_ids.is_empty(),
        "expected at least one supported host in RUNTIME_REGISTRY"
    );

    let contract = hook_event_routing_contract();
    let stdio_contract = dispatch_stdio_json_request("hook_event_routing_contract", json!({}))
        .expect("stdio hook_event_routing_contract");
    assert_eq!(
        contract, stdio_contract,
        "stdio hook_event_routing_contract must match in-process contract"
    );
    assert_eq!(
        contract["authority"].as_str(),
        Some(HOOK_EVENT_ROUTING_AUTHORITY)
    );
    assert_eq!(
        contract["schema_version"].as_str(),
        Some(HOOK_EVENT_ROUTING_SCHEMA_VERSION)
    );

    let projections = registry
        .get("host_projections")
        .and_then(Value::as_object)
        .expect("host_projections");

    for host_id in &host_ids {
        let provider = host_provider_for_id(host_id)
            .unwrap_or_else(|| panic!("missing HostProvider for registry host {host_id:?}"));
        let projection = projections
            .get(host_id.as_str())
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("missing projection for host {host_id}"));
        let registry_transport = projection
            .get("transport")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert_eq!(
            provider.hook_telemetry_surface(),
            registry_transport,
            "hook_telemetry_surface drift for host {host_id}"
        );

        let has_native = provider.capabilities().has_native_hook;
        if has_native {
            assert_eq!(
                provider.observation_host_id(),
                Some(host_id.as_str()),
                "observation_host_id drift for native host {host_id}"
            );
            for event in provider.registered_hook_events() {
                let canonical = canonical_hook_event(event).unwrap_or_else(|| {
                    panic!("unmapped registered hook event `{event}` for host {host_id}")
                });
                assert!(
                    routable_lifecycle_events().any(|item| item == canonical),
                    "canonical `{canonical}` not in contract for host {host_id}"
                );
            }
        } else {
            assert!(
                provider.registered_hook_events().is_empty(),
                "anemic host {host_id} must not declare native hook events"
            );
            assert!(
                provider.observation_host_id().is_none(),
                "anemic host {host_id} must not attach hook observation id"
            );
        }
    }
}
