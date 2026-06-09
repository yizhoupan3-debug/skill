//! MCP tool schema + dispatch (Roadmap v5 §2.8 W2 / CG-2).

use crate::db::node_ops::{ResolveOutcome, SymbolFilter};
use crate::CodeGraphIndex;
use anyhow::Context;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "mcp-codegraph";
const SERVER_VERSION: &str = "0.1.0";

/// Open index, run incremental sync, and spawn filesystem watcher (W3).
pub fn prepare_index(repo_root: &Path) -> anyhow::Result<(CodeGraphIndex, crate::graph::IndexWatcher)> {
    let index = CodeGraphIndex::open(repo_root).context("open codegraph index")?;
    index
        .incremental_sync(repo_root, false)
        .context("initial incremental sync")?;
    let watcher = index
        .spawn_watcher(repo_root.to_path_buf())
        .context("spawn index watcher")?;
    Ok((index, watcher))
}

pub fn run_stdio_mcp(repo_root: &Path) -> anyhow::Result<()> {
    let (index, _watcher) = prepare_index(repo_root)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("codegraph MCP stdin read error: {err}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {err}")},
                });
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
                continue;
            }
        };
        if let Some(response) = handle_request(&request, &index) {
            writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn handle_request(request: &Value, index: &CodeGraphIndex) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "notifications/initialized" | "notifications/cancelled" => None,
        "initialize" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": PROTOCOL_VERSION,
                "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                "capabilities": {"tools": {"listChanged": false}},
            }
        })),
        "ping" => Some(json!({"jsonrpc": "2.0", "id": id, "result": {}})),
        "tools/list" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"tools": tool_definitions()}
        })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match dispatch_tool_call(&params, index) {
                Ok(result) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result,
                })),
                Err(err) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32000, "message": err.to_string()},
                })),
            }
        }
        _ => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": format!("Method not found: {method}")},
        })),
    }
}

pub fn tool_definitions() -> Vec<Value> {
    let node_array = json!({"type": "array", "items": node_schema()});
    vec![
        tool_def(
            "codegraph_search",
            "Search indexed symbols",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "kind": {"type": "string"},
                    "language": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100}
                },
                "required": ["query"]
            }),
            node_array.clone(),
        ),
        tool_def(
            "codegraph_callers",
            "Find callers of a symbol; use file_path or node_id when symbol is ambiguous",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "depth": {"type": "integer", "minimum": 1, "maximum": 8},
                    "file_path": {"type": "string", "description": "Disambiguate duplicate symbols"},
                    "node_id": {"type": "string", "description": "Exact node id filter"}
                },
                "required": ["symbol"]
            }),
            node_array.clone(),
        ),
        tool_def(
            "codegraph_callees",
            "Find callees of a symbol; use file_path or node_id when symbol is ambiguous",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "file_path": {"type": "string", "description": "Disambiguate duplicate symbols"},
                    "node_id": {"type": "string", "description": "Exact node id filter"}
                },
                "required": ["symbol"]
            }),
            node_array.clone(),
        ),
        tool_def(
            "codegraph_impact",
            "Impact radius for a symbol; use file_path or node_id when symbol is ambiguous",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "depth": {"type": "integer", "minimum": 1, "maximum": 8},
                    "file_path": {"type": "string", "description": "Disambiguate duplicate symbols"},
                    "node_id": {"type": "string", "description": "Exact node id filter"}
                },
                "required": ["symbol"]
            }),
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "depth": {"type": "integer"},
                    "callers": node_array.clone(),
                    "callees": node_array
                }
            }),
        ),
        tool_def(
            "codegraph_node",
            "Resolve node by id or symbol; ambiguous symbol returns candidates",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "symbol": {"type": "string"},
                    "file_path": {"type": "string", "description": "Disambiguate duplicate symbols"},
                    "node_id": {"type": "string", "description": "Exact node id filter"}
                }
            }),
            json!({
                "type": "object",
                "properties": {
                    "node": node_schema(),
                    "candidates": {"type": "array", "items": node_schema()}
                }
            }),
        ),
        tool_def(
            "codegraph_status",
            "Index statistics and optional file list",
            json!({
                "type": "object",
                "properties": {
                    "include_files": {"type": "boolean"}
                }
            }),
            json!({
                "type": "object",
                "properties": {
                    "schema_version": {"type": "string"},
                    "stats": {"type": "object"},
                    "files": {"type": "array"}
                }
            }),
        ),
    ]
}

fn tool_def(name: &str, description: &str, input: Value, output: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input,
        "outputSchema": output,
    })
}

fn node_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": {"type": "string"},
            "symbol": {"type": "string"},
            "kind": {"type": "string"},
            "language": {"type": "string"},
            "file_path": {"type": "string"},
            "line": {"type": "integer"}
        }
    })
}

pub fn dispatch_tool_call(params: &Value, index: &CodeGraphIndex) -> anyhow::Result<Value> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .context("missing tool name")?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let payload = match name {
        "codegraph_search" => {
            let query = require_str(&args, "query")?;
            let kind = optional_str(&args, "kind");
            let language = optional_str(&args, "language");
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(20)
                .clamp(1, 100) as usize;
            let nodes = index.search_symbols(query, kind, language, limit)?;
            json!({"nodes": nodes})
        }
        "codegraph_callers" => {
            let symbol = require_str(&args, "symbol")?;
            let depth = args
                .get("depth")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 8) as u32;
            let filter = symbol_filter_from_args(&args);
            ensure_symbol_resolved(index, symbol, &filter)?;
            let nodes = index.find_callers(symbol, depth, &filter)?;
            json!({"nodes": nodes})
        }
        "codegraph_callees" => {
            let symbol = require_str(&args, "symbol")?;
            let filter = symbol_filter_from_args(&args);
            ensure_symbol_resolved(index, symbol, &filter)?;
            let nodes = index.find_callees(symbol, &filter)?;
            json!({"nodes": nodes})
        }
        "codegraph_impact" => {
            let symbol = require_str(&args, "symbol")?;
            let depth = args
                .get("depth")
                .and_then(Value::as_u64)
                .unwrap_or(2)
                .clamp(1, 8) as u32;
            let filter = symbol_filter_from_args(&args);
            ensure_symbol_resolved(index, symbol, &filter)?;
            let report = index.impact_radius(symbol, depth, &filter)?;
            json!(report)
        }
        "codegraph_node" => {
            if let Some(id) = optional_str(&args, "id") {
                let node = index.get_node_by_id(id)?;
                json!({"node": node})
            } else {
                let symbol = optional_str(&args, "symbol")
                    .ok_or_else(|| anyhow::anyhow!("codegraph_node requires id or symbol"))?;
                let filter = symbol_filter_from_args(&args);
                match index.resolve_symbol_filtered(symbol, &filter)? {
                    ResolveOutcome::Found(node) => json!({"node": node}),
                    ResolveOutcome::Ambiguous(candidates) => json!({"candidates": candidates}),
                    ResolveOutcome::NotFound => json!({"node": null}),
                }
            }
        }
        "codegraph_status" => {
            let include_files = args
                .get("include_files")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let stats = index.index_stats()?;
            let mut body = json!({
                "schema_version": crate::SCHEMA_VERSION,
                "stats": stats,
            });
            if include_files {
                body["files"] = json!(index.list_files()?);
            }
            body
        }
        other => anyhow::bail!("unknown tool: {other}"),
    };
    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&payload)?}],
        "structuredContent": payload,
    }))
}

fn symbol_filter_from_args(args: &Value) -> SymbolFilter {
    SymbolFilter::from_options(
        optional_str(args, "file_path"),
        optional_str(args, "node_id"),
    )
}

fn ensure_symbol_resolved(
    index: &CodeGraphIndex,
    symbol: &str,
    filter: &SymbolFilter,
) -> anyhow::Result<()> {
    if filter.file_path.is_some() || filter.node_id.is_some() {
        return Ok(());
    }
    match index.resolve_symbol_filtered(symbol, filter)? {
        ResolveOutcome::Ambiguous(candidates) => {
            let summary: Vec<String> = candidates
                .iter()
                .map(|node| format!("{} ({}, {})", node.id, node.file_path, node.kind))
                .collect();
            anyhow::bail!(
                "ambiguous symbol '{symbol}': pass file_path or node_id; candidates: {}",
                summary.join("; ")
            );
        }
        ResolveOutcome::NotFound => anyhow::bail!("symbol not found: {symbol}"),
        ResolveOutcome::Found(_) => Ok(()),
    }
}

fn require_str<'a>(value: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .with_context(|| format!("missing required argument: {key}"))
}

fn optional_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{dispatch_tool_call, tool_definitions};
    use crate::CodeGraphIndex;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_index() -> (std::path::PathBuf, CodeGraphIndex) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-mcp-{suffix}"));
        std::fs::create_dir_all(&root).unwrap();
        let index = CodeGraphIndex::open(&root).unwrap();
        (root, index)
    }

    #[test]
    fn exposes_six_mcp_tools_with_schemas() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 6);
        let expected = [
            "codegraph_search",
            "codegraph_callers",
            "codegraph_callees",
            "codegraph_impact",
            "codegraph_node",
            "codegraph_status",
        ];
        for name in expected {
            let tool = tools
                .iter()
                .find(|t| t.get("name").and_then(|v| v.as_str()) == Some(name))
                .unwrap_or_else(|| panic!("missing tool {name}"));
            assert!(tool.get("inputSchema").is_some());
            assert!(tool.get("outputSchema").is_some());
        }
    }

    #[test]
    fn dispatch_status_returns_stats() {
        let (root, index) = temp_index();
        let result = dispatch_tool_call(
            &json!({"name": "codegraph_status", "arguments": {}}),
            &index,
        )
        .unwrap();
        let structured = result.get("structuredContent").unwrap();
        assert_eq!(
            structured.get("schema_version").and_then(|v| v.as_str()),
            Some(crate::SCHEMA_VERSION)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_search_callers_callees_after_index() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-dispatch-{suffix}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("lib.rs"),
            "fn callee() {}\nfn caller() { callee(); }\n",
        )
        .unwrap();
        let index = CodeGraphIndex::open(&root).unwrap();
        index.build_full_index(&root).unwrap();

        let search = dispatch_tool_call(
            &json!({"name": "codegraph_search", "arguments": {"query": "caller"}}),
            &index,
        )
        .unwrap();
        let nodes = search["structuredContent"]["nodes"].as_array().unwrap();
        assert!(nodes.iter().any(|n| n["symbol"] == "caller"));

        let callers = dispatch_tool_call(
            &json!({"name": "codegraph_callers", "arguments": {"symbol": "callee"}}),
            &index,
        )
        .unwrap();
        assert_eq!(
            callers["structuredContent"]["nodes"][0]["symbol"],
            "caller"
        );

        let callees = dispatch_tool_call(
            &json!({"name": "codegraph_callees", "arguments": {"symbol": "caller"}}),
            &index,
        )
        .unwrap();
        assert_eq!(
            callees["structuredContent"]["nodes"][0]["symbol"],
            "callee"
        );

        let impact = dispatch_tool_call(
            &json!({"name": "codegraph_impact", "arguments": {"symbol": "callee", "depth": 1}}),
            &index,
        )
        .unwrap();
        assert_eq!(impact["structuredContent"]["callers"][0]["symbol"], "caller");

        let node_id = nodes[0]["id"].as_str().unwrap();
        let node = dispatch_tool_call(
            &json!({"name": "codegraph_node", "arguments": {"id": node_id}}),
            &index,
        )
        .unwrap();
        assert_eq!(node["structuredContent"]["node"]["symbol"], "caller");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn codegraph_node_returns_candidates_for_ambiguous_symbol() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-ambig-{suffix}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.rs"), "fn shared() {}\n").unwrap();
        std::fs::write(root.join("b.rs"), "fn shared() {}\n").unwrap();
        let index = CodeGraphIndex::open(&root).unwrap();
        index.build_full_index(&root).unwrap();

        let result = dispatch_tool_call(
            &json!({"name": "codegraph_node", "arguments": {"symbol": "shared"}}),
            &index,
        )
        .unwrap();
        let candidates = result["structuredContent"]["candidates"]
            .as_array()
            .expect("expected candidates for ambiguous symbol");
        assert_eq!(candidates.len(), 2);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn handle_request_tools_list_returns_six_tools() {
        let (root, index) = temp_index();
        let response = super::handle_request(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
            &index,
        )
        .unwrap();
        assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 6);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prepare_index_runs_sync_and_spawns_watcher() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-prepare-{suffix}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("lib.rs"), "fn gamma() {}\n").unwrap();
        let (index, _watcher) = super::prepare_index(&root).unwrap();
        let stats = index.index_stats().unwrap();
        assert!(stats.node_count >= 1);
        let _ = std::fs::remove_dir_all(root);
    }
}
