//! MCP tool schema + dispatch.

use crate::CodeGraphIndex;
use crate::db::node_ops::{ResolveOutcome, SymbolFilter};
use anyhow::Context;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::Instant;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "mcp-codegraph";
const SERVER_VERSION: &str = "0.2.0";

/// Open index, run incremental sync, and spawn filesystem watcher (W3).
pub fn prepare_index(
    repo_root: &Path,
) -> anyhow::Result<(CodeGraphIndex, crate::graph::IndexWatcher)> {
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
            "Search indexed code symbols by name using full-text search. Returns matching symbols with their kind (fn/struct/class/const etc), language, file path and line number. Use optional kind/language filters to narrow results. Examples: search for 'handle_request' to find handler functions, search with kind='struct' to find data types.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Symbol name or prefix to search for"},
                    "kind": {"type": "string", "description": "Filter by symbol kind: fn, struct, enum, trait, class, const, function, method, interface, type"},
                    "language": {"type": "string", "description": "Filter by language: rust, typescript, python, go"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "Max results (default 20)"}
                },
                "required": ["query"]
            }),
            node_array.clone(),
        ),
        tool_def(
            "codegraph_callers",
            "Find all upstream callers of a symbol using BFS traversal (up to 8 hops). Returns every function/method that directly or transitively calls the given symbol. Use file_path or node_id to disambiguate when multiple symbols share the same name across files.",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Symbol name to find callers of"},
                    "depth": {"type": "integer", "minimum": 1, "maximum": 8, "description": "BFS depth (default 1 = direct callers only)"},
                    "file_path": {"type": "string", "description": "Disambiguate by restricting to this file"},
                    "node_id": {"type": "string", "description": "Exact node id filter (from codegraph_search results)"}
                },
                "required": ["symbol"]
            }),
            node_array.clone(),
        ),
        tool_def(
            "codegraph_callees",
            "Find all downstream callees of a symbol using BFS traversal (up to 8 hops). Returns every function/method that the given symbol directly or transitively calls. Use file_path or node_id to disambiguate when multiple symbols share the same name.",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Symbol name to find callees of"},
                    "depth": {"type": "integer", "minimum": 1, "maximum": 8, "description": "BFS depth (default 1 = direct callees only)"},
                    "file_path": {"type": "string", "description": "Disambiguate by restricting to this file"},
                    "node_id": {"type": "string", "description": "Exact node id filter"}
                },
                "required": ["symbol"]
            }),
            node_array.clone(),
        ),
        tool_def(
            "codegraph_impact",
            "Impact radius analysis: combines callers (upstream BFS) and callees (downstream BFS) to show the full blast radius of changing a symbol. Use this to assess the risk of refactoring a function or type.",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Symbol to analyze impact of"},
                    "depth": {"type": "integer", "minimum": 1, "maximum": 8, "description": "BFS depth for both directions (default 2)"},
                    "file_path": {"type": "string", "description": "Disambiguate duplicate symbol names"},
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
            "Resolve a single node by exact id or symbol name. If the symbol is ambiguous (exists in multiple files), returns a candidates list instead — pick one and retry with file_path or node_id.",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Exact node id (from search results, format: path:line:symbol)"},
                    "symbol": {"type": "string", "description": "Symbol name to resolve"},
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
            "Index health and statistics: shows node/edge/file counts, index size on disk, last indexed timestamp, and optionally the full file list. Use this to check if the index is up to date before relying on search results.",
            json!({
                "type": "object",
                "properties": {
                    "include_files": {"type": "boolean", "description": "Include the full list of indexed files"}
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
        tool_def(
            "codegraph_dead_code",
            "Detect dead code by finding all function/method symbols with zero callers (in-degree = 0 in the call graph). Optionally filter by language and minimum line number. Returns symbols, file paths, line numbers, and caller counts. Use this to identify unused functions that are candidates for removal.",
            json!({
                "type": "object",
                "properties": {
                    "language": {"type": "string", "description": "Filter by language: rust, typescript, python, go"},
                    "min_lines": {"type": "integer", "minimum": 1, "description": "Filter: only return functions whose line number >= this value (useful to focus on larger functions)"}
                }
            }),
            json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "symbol": {"type": "string"},
                        "kind": {"type": "string"},
                        "language": {"type": "string"},
                        "file_path": {"type": "string"},
                        "line": {"type": "integer"},
                        "callers_count": {"type": "integer"}
                    }
                }
            }),
        ),
        tool_def(
            "codegraph_goto_definition",
            "Find the definition location of a symbol in the codebase. Returns the file path, line number, and column range for each definition found. Prioritizes definition-kind nodes (function, struct, class, enum, trait, interface, type) over usage/reference nodes. Use file_path to disambiguate when multiple files define the same symbol.",
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string", "description": "Symbol name to find definition of"},
                    "file_path": {"type": "string", "description": "Restrict search to this file"}
                },
                "required": ["symbol"]
            }),
            json!({
                "type": "object",
                "properties": {
                    "symbol": {"type": "string"},
                    "definitions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string"},
                                "symbol": {"type": "string"},
                                "kind": {"type": "string"},
                                "language": {"type": "string"},
                                "file_path": {"type": "string"},
                                "line": {"type": "integer"},
                                "start_col": {"type": "integer"},
                                "end_line": {"type": "integer"},
                                "end_col": {"type": "integer"}
                            }
                        }
                    },
                    "count": {"type": "integer"}
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
    let start = Instant::now();
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
            json!({"nodes": nodes, "depth": depth})
        }
        "codegraph_callees" => {
            let symbol = require_str(&args, "symbol")?;
            let depth = args
                .get("depth")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, 8) as u32;
            let filter = symbol_filter_from_args(&args);
            ensure_symbol_resolved(index, symbol, &filter)?;
            let nodes = index.find_callees(symbol, depth, &filter)?;
            json!({"nodes": nodes, "depth": depth})
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
        "codegraph_dead_code" => {
            let language = optional_str(&args, "language");
            let min_lines = args
                .get("min_lines")
                .and_then(Value::as_u64)
                .map(|v| v.clamp(1, 100_000) as u32);
            let nodes = index.find_dead_code(language, min_lines)?;
            json!({"dead_functions": nodes, "count": nodes.len()})
        }
        "codegraph_goto_definition" => {
            let symbol = require_str(&args, "symbol")?;
            let file_path = optional_str(&args, "file_path");
            let defs = index.find_definition(symbol, file_path)?;
            json!({"symbol": symbol, "definitions": defs, "count": defs.len()})
        }
        other => anyhow::bail!("unknown tool: {other}"),
    };
    let elapsed_ms = start.elapsed().as_millis();
    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&payload)?}],
        "structuredContent": payload,
        "_meta": {"tool": name, "elapsed_ms": elapsed_ms},
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
            // Return structured JSON error with candidate list
            let candidate_objs: Vec<Value> = candidates
                .iter()
                .map(|node| {
                    json!({
                        "id": node.id,
                        "symbol": node.symbol,
                        "file_path": node.file_path,
                        "kind": node.kind,
                        "line": node.line,
                    })
                })
                .collect();
            anyhow::bail!(
                "{}",
                serde_json::to_string(&json!({
                    "error": "ambiguous_symbol",
                    "symbol": symbol,
                    "hint": "Pass file_path or node_id to disambiguate",
                    "candidates": candidate_objs,
                }))?
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
            .expect("system time since epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-mcp-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp directory");
        let index = CodeGraphIndex::open(&root).expect("create temp directory");
        (root, index)
    }

    #[test]
    fn exposes_eight_mcp_tools_with_schemas() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 8);
        let expected = [
            "codegraph_search",
            "codegraph_callers",
            "codegraph_callees",
            "codegraph_impact",
            "codegraph_node",
            "codegraph_status",
            "codegraph_dead_code",
            "codegraph_goto_definition",
        ];
        for name in expected {
            let tool = tools
                .iter()
                .find(|t| t.get("name").and_then(|v| v.as_str()) == Some(name))
                .unwrap_or_else(|| panic!("missing tool {name}"));
            assert!(tool.get("inputSchema").is_some());
            assert!(tool.get("outputSchema").is_some());
            // Descriptions should be meaningful (> 50 chars)
            let desc = tool
                .get("description")
                .and_then(|v| v.as_str())
                .expect("should succeed");
            assert!(desc.len() > 50, "tool {name} description too short: {desc}");
        }
    }

    #[test]
    fn dispatch_status_returns_stats_with_db_size() {
        let (root, index) = temp_index();
        let result = dispatch_tool_call(
            &json!({"name": "codegraph_status", "arguments": {}}),
            &index,
        )
        .expect("should succeed");
        let structured = result.get("structuredContent").expect("get should succeed");
        assert_eq!(
            structured.get("schema_version").and_then(|v| v.as_str()),
            Some(crate::SCHEMA_VERSION)
        );
        // Should include db_size_bytes in stats
        let stats = structured.get("stats").expect("get should succeed");
        assert!(stats.get("db_size_bytes").is_some());
        // Should include _meta with elapsed_ms
        let meta = result.get("_meta").expect("get should succeed");
        assert!(meta.get("elapsed_ms").is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_search_callers_callees_after_index() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time since epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-dispatch-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp directory");
        std::fs::write(
            root.join("lib.rs"),
            "fn callee() {}\nfn caller() { callee(); }\n",
        )
        .expect("should succeed");
        let index = CodeGraphIndex::open(&root).expect("open test index");
        index.build_full_index(&root).expect("open test index");

        let search = dispatch_tool_call(
            &json!({"name": "codegraph_search", "arguments": {"query": "caller"}}),
            &index,
        )
        .expect("should succeed");
        let nodes = search["structuredContent"]["nodes"]
            .as_array()
            .expect("as_array should succeed");
        assert!(nodes.iter().any(|n| n["symbol"] == "caller"));

        let callers = dispatch_tool_call(
            &json!({"name": "codegraph_callers", "arguments": {"symbol": "callee"}}),
            &index,
        )
        .expect("should succeed");
        assert_eq!(callers["structuredContent"]["nodes"][0]["symbol"], "caller");

        let callees = dispatch_tool_call(
            &json!({"name": "codegraph_callees", "arguments": {"symbol": "caller"}}),
            &index,
        )
        .expect("should succeed");
        assert_eq!(callees["structuredContent"]["nodes"][0]["symbol"], "callee");

        let impact = dispatch_tool_call(
            &json!({"name": "codegraph_impact", "arguments": {"symbol": "callee", "depth": 1}}),
            &index,
        )
        .expect("should succeed");
        assert_eq!(
            impact["structuredContent"]["callers"][0]["symbol"],
            "caller"
        );

        let node_id = nodes[0]["id"].as_str().expect("as_str should succeed");
        let node = dispatch_tool_call(
            &json!({"name": "codegraph_node", "arguments": {"id": node_id}}),
            &index,
        )
        .expect("should succeed");
        assert_eq!(node["structuredContent"]["node"]["symbol"], "caller");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn codegraph_node_returns_candidates_for_ambiguous_symbol() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time since epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-ambig-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp directory");
        std::fs::write(root.join("a.rs"), "fn shared() {}\n").expect("create temp directory");
        std::fs::write(root.join("b.rs"), "fn shared() {}\n").expect("create temp directory");
        let index = CodeGraphIndex::open(&root).expect("create temp directory");
        index.build_full_index(&root).expect("write test file");

        let result = dispatch_tool_call(
            &json!({"name": "codegraph_node", "arguments": {"symbol": "shared"}}),
            &index,
        )
        .expect("should succeed");
        let candidates = result["structuredContent"]["candidates"]
            .as_array()
            .expect("expected candidates for ambiguous symbol");
        assert_eq!(candidates.len(), 2);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn codegraph_dead_code_dispatch_works() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time since epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-dead-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp directory");
        std::fs::write(
            root.join("lib.rs"),
            "fn callee() {}\nfn caller() { callee(); }\n",
        )
        .expect("should succeed");
        let index = CodeGraphIndex::open(&root).expect("open test index");
        index.build_full_index(&root).expect("build full index");

        let result = dispatch_tool_call(
            &json!({"name": "codegraph_dead_code", "arguments": {}}),
            &index,
        )
        .expect("should succeed");
        let structured = result
            .get("structuredContent")
            .expect("get structuredContent");
        let dead = structured["dead_functions"]
            .as_array()
            .expect("expected dead_functions array");
        // caller has no callers → dead
        // callee has 1 caller → not dead
        let symbols: Vec<&str> = dead.iter().map(|n| n["symbol"].as_str().unwrap()).collect();
        assert!(symbols.contains(&"caller"), "caller should be dead code");
        assert!(!symbols.contains(&"callee"), "callee is called");
        assert_eq!(structured["count"].as_u64().unwrap(), dead.len() as u64);

        // Test with language filter
        let filtered = dispatch_tool_call(
            &json!({"name": "codegraph_dead_code", "arguments": {"language": "rust"}}),
            &index,
        )
        .expect("should succeed");
        let filtered_dead = filtered["structuredContent"]["dead_functions"]
            .as_array()
            .expect("expected array");
        assert!(!filtered_dead.is_empty());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn handle_request_tools_list_returns_eight_tools() {
        let (root, index) = temp_index();
        let response = super::handle_request(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
            &index,
        )
        .expect("should succeed");
        assert_eq!(
            response["result"]["tools"]
                .as_array()
                .expect("as_array should succeed")
                .len(),
            8
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prepare_index_runs_sync_and_spawns_watcher() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time since epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-prepare-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp directory");
        std::fs::write(root.join("lib.rs"), "fn gamma() {}\n").expect("create temp directory");
        let (index, _watcher) = super::prepare_index(&root).expect("create temp directory");
        let stats = index.index_stats().expect("create temp directory");
        assert!(stats.node_count >= 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ambiguous_symbol_error_is_structured_json() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time since epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-err-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp directory");
        std::fs::write(root.join("a.rs"), "fn dup() {}\n").expect("create temp directory");
        std::fs::write(root.join("b.rs"), "fn dup() {}\n").expect("create temp directory");
        let index = CodeGraphIndex::open(&root).expect("create temp directory");
        index.build_full_index(&root).expect("write test file");

        let result = dispatch_tool_call(
            &json!({"name": "codegraph_callers", "arguments": {"symbol": "dup"}}),
            &index,
        );
        // Should fail with structured JSON error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        let parsed: serde_json::Value =
            serde_json::from_str(&err_msg).expect("error should be valid JSON");
        assert_eq!(parsed["error"], "ambiguous_symbol");
        assert!(
            !parsed["candidates"]
                .as_array()
                .expect("as_array should succeed").is_empty()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dispatch_goto_definition_returns_definitions() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time since epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("codegraph-goto-{suffix}"));
        std::fs::create_dir_all(&root).expect("create temp directory");
        std::fs::write(root.join("lib.rs"), "fn target() {}\n").expect("write test file");
        let index = crate::CodeGraphIndex::open(&root).expect("open index");
        index.build_full_index(&root).expect("build index");

        let result = dispatch_tool_call(
            &json!({"name": "codegraph_goto_definition", "arguments": {"symbol": "target"}}),
            &index,
        )
        .expect("should succeed");
        let defs = result["structuredContent"]["definitions"]
            .as_array()
            .expect("expected definitions array");
        assert_eq!(defs.len(), 1, "should find 1 definition");
        assert_eq!(defs[0]["symbol"], "target");
        assert_eq!(defs[0]["kind"], "function");
        assert_eq!(defs[0]["file_path"], "lib.rs");
        assert!(defs[0]["line"].as_u64().unwrap_or(0) >= 1);

        let _ = std::fs::remove_dir_all(root);
    }
}
