//! W3 integration: incremental sync + watcher bootstrap + MCP dispatch roundtrip.

use codegraph_rs::mcp::{dispatch_tool_call, prepare_index};
use serde_json::json;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_repo(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("codegraph-w3-{name}-{suffix}"));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn w3_prepare_index_sync_watcher_mcp_search_roundtrip() {
    let root = temp_repo("roundtrip");
    fs::write(root.join("lib.rs"), "fn seed_symbol() {}\n").unwrap();

    let (index, _watcher) = prepare_index(&root).expect("prepare_index");

    let seeded = dispatch_tool_call(
        &json!({"name": "codegraph_search", "arguments": {"query": "seed_symbol"}}),
        &index,
    )
    .expect("search seeded symbol");
    let seeded_nodes = seeded["structuredContent"]["nodes"]
        .as_array()
        .expect("nodes array");
    assert!(
        seeded_nodes.iter().any(|n| n["symbol"] == "seed_symbol"),
        "expected seed_symbol in index after prepare_index"
    );

    fs::write(root.join("extra.rs"), "fn delta_symbol() {}\n").unwrap();
    let report = index
        .incremental_sync(&root, false)
        .expect("incremental sync after file add");
    assert!(
        report.files_updated >= 1,
        "expected at least one updated file, got {:?}",
        report
    );

    let delta = dispatch_tool_call(
        &json!({"name": "codegraph_search", "arguments": {"query": "delta_symbol"}}),
        &index,
    )
    .expect("search delta symbol");
    let delta_nodes = delta["structuredContent"]["nodes"]
        .as_array()
        .expect("nodes array");
    assert!(
        delta_nodes.iter().any(|n| n["symbol"] == "delta_symbol"),
        "expected delta_symbol after incremental sync"
    );

    let status = dispatch_tool_call(
        &json!({"name": "codegraph_status", "arguments": {}}),
        &index,
    )
    .expect("status");
    assert!(
        status["structuredContent"]["stats"]["file_count"]
            .as_u64()
            .unwrap_or(0)
            >= 2
    );

    let _ = fs::remove_dir_all(root);
}
