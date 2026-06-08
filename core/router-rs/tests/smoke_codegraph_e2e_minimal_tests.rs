//! CG deferred minimal E2E: router-rs `codegraph_mcp` re-export + live index roundtrip.
//!
//! Bridges `codegraph-rs` prepare_index/dispatch with the router thin shell (no stdio subprocess).

#[cfg(feature = "codegraph")]
mod codegraph_e2e {
    use crate::codegraph_mcp::dispatch_tool_call;
    use codegraph_rs::mcp::prepare_index;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-e2e-{name}-{suffix}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// prepare_index → dispatch_tool_call(search + status) via router-rs codegraph_mcp surface.
    #[test]
    fn codegraph_prepare_index_tool_roundtrip_smoke() {
        let root = temp_repo("router-roundtrip");
        fs::write(root.join("lib.rs"), "fn e2e_anchor_symbol() {}\n").unwrap();

        let (index, _watcher) = prepare_index(&root).expect("prepare_index");

        let search = dispatch_tool_call(
            &json!({"name": "codegraph_search", "arguments": {"query": "e2e_anchor_symbol"}}),
            &index,
        )
        .expect("codegraph_search");
        let nodes = search["structuredContent"]["nodes"]
            .as_array()
            .expect("search nodes");
        assert!(
            nodes.iter().any(|n| n["symbol"] == "e2e_anchor_symbol"),
            "expected e2e_anchor_symbol after prepare_index"
        );

        let status = dispatch_tool_call(
            &json!({"name": "codegraph_status", "arguments": {}}),
            &index,
        )
        .expect("codegraph_status");
        assert!(
            status["structuredContent"]["stats"]["node_count"]
                .as_u64()
                .unwrap_or(0)
                >= 1,
            "status must report indexed nodes"
        );

        let _ = fs::remove_dir_all(root);
    }
}
