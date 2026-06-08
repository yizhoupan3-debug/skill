//! W4 / B1 integration: MCP graph tools (callers, callees, impact) over parsed Rust sources.

use codegraph_rs::mcp::{dispatch_tool_call, prepare_index};
use serde_json::json;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_repo(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("codegraph-w4-{name}-{suffix}"));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn w4_mcp_callers_callees_impact_roundtrip() {
    let root = temp_repo("graph");
    fs::write(
        root.join("chain.rs"),
        r#"
fn leaf_helper() {}

fn middle_target() {
    leaf_helper();
}

fn top_caller() {
    middle_target();
}
"#,
    )
    .unwrap();

    let (index, _watcher) = prepare_index(&root).expect("prepare_index");

    let callers = dispatch_tool_call(
        &json!({"name": "codegraph_callers", "arguments": {"symbol": "middle_target", "depth": 2}}),
        &index,
    )
    .expect("callers");
    let caller_nodes = callers["structuredContent"]["nodes"]
        .as_array()
        .expect("caller nodes");
    assert!(
        caller_nodes.iter().any(|n| n["symbol"] == "top_caller"),
        "expected top_caller in callers of middle_target"
    );

    let callees = dispatch_tool_call(
        &json!({"name": "codegraph_callees", "arguments": {"symbol": "middle_target"}}),
        &index,
    )
    .expect("callees");
    let callee_nodes = callees["structuredContent"]["nodes"]
        .as_array()
        .expect("callee nodes");
    assert!(
        callee_nodes.iter().any(|n| n["symbol"] == "leaf_helper"),
        "expected leaf_helper as callee of middle_target"
    );

    let impact = dispatch_tool_call(
        &json!({"name": "codegraph_impact", "arguments": {"symbol": "middle_target", "depth": 2}}),
        &index,
    )
    .expect("impact");
    let report = &impact["structuredContent"];
    assert_eq!(report["symbol"], "middle_target");
    assert!(
        report["callers"]
            .as_array()
            .is_some_and(|nodes| nodes.iter().any(|n| n["symbol"] == "top_caller"))
    );
    assert!(
        report["callees"]
            .as_array()
            .is_some_and(|nodes| nodes.iter().any(|n| n["symbol"] == "leaf_helper"))
    );

    let _ = fs::remove_dir_all(root);
}
