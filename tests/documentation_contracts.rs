mod common;

use common::{project_root, read_text};
use regex::Regex;
use serde_json::Value;

#[test]
fn compaction_contract_freezes_required_sections() {
    let text = runtime_compaction_contract();
    for heading in [
        "# Runtime Compaction Contract",
        "## Contract 1: Snapshot Schema",
        "## Contract 2: Delta Replay Contract",
        "## Contract 3: Generation Rollover Policy",
        "## Contract 4: Artifact Ref Strategy",
        "## Contract 5: Consistency Invariants",
        "## Current Minimal Implementation Status",
    ] {
        assert!(text.contains(heading), "missing heading: {heading}");
    }
}

#[test]
fn compaction_contract_snapshot_and_delta_fields_are_explicit() {
    let text = runtime_compaction_contract();
    for field in [
        "schema_version",
        "generation",
        "snapshot_id",
        "parent_generation",
        "parent_snapshot_id",
        "session_id",
        "job_id",
        "created_at",
        "watermark_event_id",
        "state_digest",
        "artifact_index_ref",
        "state_ref",
        "delta_cursor",
        "summary",
        "delta_id",
        "seq",
        "ts",
        "kind",
        "payload",
        "artifact_refs",
        "applies_to",
        "artifact_id",
        "uri",
        "digest",
        "size_bytes",
        "producer",
    ] {
        assert!(
            text.contains(&format!("`{field}:")) || text.contains(&format!("`{field}`")),
            "missing field: {field}"
        );
    }
}

#[test]
fn compaction_contract_generation_rules_cover_inheritance_and_recovery() {
    let text = runtime_compaction_contract();
    for rule in [
        "new generation inherits only the minimal necessary state",
        "session identity",
        "job identity",
        "old generation must remain readable for audit and recovery",
        "one rollover produces exactly one successor generation",
        "generation numbers must be monotonic",
        "parent_snapshot_id",
        "latest stable snapshot",
        "artifact refs",
        "must not require scanning the full historical stream",
    ] {
        assert!(text.contains(rule), "missing rule: {rule}");
    }
}

#[test]
fn compaction_contract_consistency_rules_are_non_negotiable() {
    let text = runtime_compaction_contract();
    for marker in [
        "replay must be deterministic",
        "idempotent",
        "fail closed",
        "cross-generation mutable aliasing",
        "state_digest",
    ] {
        assert!(text.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn compaction_contract_minimal_implementation_status_is_explicit() {
    let text = runtime_compaction_contract();
    for marker in [
        "supports_compaction",
        "supports_snapshot_delta",
        "capability catalog",
        "payload SHA-256 digest",
        "`verify_text`",
        "`verified`",
        "consistent append",
        "WAL-backed durability",
        "one `backend_family`",
        "`aligned` / `compaction_eligible`",
        "one stable snapshot for the old generation",
        "exactly one successor generation",
        "latest stable snapshot plus generation-local deltas",
        "fail-closed / no-op",
    ] {
        assert!(text.contains(marker), "missing marker: {marker}");
    }
}

#[test]
fn runtime_sandbox_contract_schema_freezes_control_plane_semantics() {
    let schema = load_sandbox_contract_schema();
    assert_eq!(schema["schema_version"], "runtime-sandbox-contract-v1");
    assert_eq!(
        schema["lifecycle_states"],
        serde_json::json!(["created", "warm", "busy", "draining", "recycled", "failed"])
    );
    assert_eq!(
        schema["allowed_transitions"],
        serde_json::json!([
            ["created", "warm"],
            ["warm", "busy"],
            ["busy", "draining"],
            ["draining", "recycled"],
            ["draining", "failed"],
            ["warm", "failed"],
            ["busy", "failed"],
            ["recycled", "warm"]
        ])
    );
    assert_eq!(
        schema["tool_capability_categories"],
        serde_json::json!(["read_only", "workspace_mutating", "networked", "high_risk"])
    );
    assert_eq!(
        schema["resource_budgets"],
        serde_json::json!(["cpu", "memory", "wall_clock", "output_size"])
    );
    assert_eq!(
        schema["recoverability_boundary"],
        serde_json::json!({
            "recoverable": [
                "transient timeout",
                "transient kill request",
                "cleanup retry after a failed async cleanup attempt",
                "takeover after control-plane interruption when policy-compliant"
            ],
            "non_recoverable": [
                "repeated cleanup failure",
                "policy violation that invalidates the sandbox profile",
                "contamination of sandbox-local state that cannot be deterministically cleared",
                "any state where reuse would require privilege expansion or hidden host repair"
            ]
        })
    );
}

#[test]
fn runtime_sandbox_contract_text_mentions_required_policy_boundaries() {
    let text = read_text(&project_root().join("docs/runtime_sandbox_contract.md")).to_lowercase();
    for phrase in [
        "async cleanup",
        "failure isolation",
        "recoverability boundary",
        "deny-by-default",
        "high-risk tools must use a dedicated sandbox profile",
        "budgets are part of the contract",
    ] {
        assert!(text.contains(phrase), "missing phrase: {phrase}");
    }
}

#[test]
fn rust_contracts_doc_keeps_the_three_part_status_ledger() {
    let text = rust_contracts_doc();
    for heading in [
        "## Current Status Ledger",
        "### 已实现",
        "### 已退休",
        "### 下一 safe slice",
    ] {
        assert!(text.contains(heading), "missing heading: {heading}");
    }
}

#[test]
fn rust_contracts_doc_no_longer_uses_stale_transition_wording() {
    let text = rust_contracts_doc();
    for stale_phrase in [
        "escape hatch",
        "not live yet",
        "implementation remains pending",
        "hidden behind an escape hatch",
    ] {
        assert!(
            !text.contains(stale_phrase),
            "stale phrase present: {stale_phrase}"
        );
    }
}

#[test]
fn rust_contracts_doc_records_current_minimal_implementation_truth() {
    let text = rust_contracts_doc();
    for required_phrase in [
        "Compatibility live fallback request surface has been removed",
        "framework_runtime/` Python package is retired",
        "SQLite is the strongest local backend for WAL",
        "Sandbox lifecycle contract is frozen",
        "Any new Python runtime, routing, artifact, hook, or host-integration implementation is a regression",
    ] {
        assert!(
            text.contains(required_phrase),
            "missing phrase: {required_phrase}"
        );
    }
}

#[test]
fn top_level_docs_do_not_revive_retired_python_work_as_active() {
    let root = project_root();
    let scoped_docs = [
        "rust_checklist.md",
        "rust_next_phase_checklist.md",
        "audit_report.md",
        "omc_checklist.md",
        "docs/rust_contracts.md",
        "docs/framework_profile_contract.md",
        "docs/host_adapter_contracts.md",
        "docs/runtime_observability_contract.md",
        "docs/runtime_sandbox_contract.md",
        "aionrs_fusion_docs/codex_dual_entry_rust_checklist.md",
        "aionrs_fusion_docs/codex_dual_entry_next_phase_checklist.md",
    ];
    let joined = scoped_docs
        .iter()
        .map(|path| read_text(&root.join(path)))
        .collect::<Vec<_>>()
        .join("\n");
    for stale_phrase in [
        "keep-temporarily",
        "pending-removal",
        "Python artifact emitter 已支持",
        "Python artifact emitter 已外显",
        "Python / Rust parity tests",
        "framework_runtime/src/framework_runtime",
        "scripts/materialize_cli_host_entrypoints.py 管理",
        "runtime durable state: `framework_runtime/data",
    ] {
        assert!(
            !joined.contains(stale_phrase),
            "stale active-doc phrase present: {stale_phrase}"
        );
    }
}

fn runtime_compaction_contract() -> String {
    read_text(&project_root().join("docs/runtime_compaction_contract.md"))
}

fn rust_contracts_doc() -> String {
    read_text(&project_root().join("docs/rust_contracts.md"))
}

fn load_sandbox_contract_schema() -> Value {
    let text = read_text(&project_root().join("docs/runtime_sandbox_contract.md"));
    let pattern = Regex::new(r"(?s)```json sandbox-contract-v1\n(.*?)\n```").unwrap();
    let captures = pattern
        .captures(&text)
        .expect("sandbox contract schema block is missing");
    serde_json::from_str(captures.get(1).unwrap().as_str()).unwrap()
}
