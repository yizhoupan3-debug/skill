//! MCP stdio server for gh_source_gate_rs — exposes `gh_source_gate` tool.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

const SERVER_NAME: &str = "mcp-gh-source-gate";
const SERVER_VERSION: &str = "0.1.0";
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the MCP stdio server. Reads JSON-RPC lines from stdin, writes to stdout.
pub fn run_stdio_mcp(_repo_root: &Path) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(err) => {
                eprintln!("gh-source-gate MCP stdin read error: {err}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(err) => {
                let resp = json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {err}")},
                });
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        };
        if let Some(response) = handle_request(&request) {
            writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn handle_request(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "notifications/initialized" | "notifications/cancelled" => None,
        "initialize" => Some(json!({
            "jsonrpc": "2.0", "id": id,
            "result": {
                "protocolVersion": PROTOCOL_VERSION,
                "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                "capabilities": {"tools": {"listChanged": false}},
            }
        })),
        "ping" => Some(json!({"jsonrpc": "2.0", "id": id, "result": {}})),
        "tools/list" => Some(json!({
            "jsonrpc": "2.0", "id": id,
            "result": {"tools": tool_definitions()}
        })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match dispatch_tool_call(&params) {
                Ok(result) => Some(json!({
                    "jsonrpc": "2.0", "id": id, "result": result,
                })),
                Err(err) => Some(json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": {"code": -32000, "message": err.to_string()},
                })),
            }
        }
        _ => Some(json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": -32601, "message": format!("Method not found: {method}")},
        })),
    }
}

/// MCP tool definitions exposed by this server.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "gh_source_gate",
            "description": "Inspect failing GitHub Actions checks, fetch PR review comments, or run doctor diagnostics for a repository.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "GitHub PR URL or repo path (absolute). If empty, uses current directory."
                    },
                    "action": {
                        "type": "string",
                        "enum": ["validate", "audit", "check"],
                        "description": "Operation: 'check' inspects failing PR checks, 'audit' fetches review comments and threads, 'validate' runs doctor diagnostics.",
                        "default": "check"
                    },
                    "pr": {
                        "type": "string",
                        "description": "PR number or URL override (optional, auto-detected if omitted)"
                    },
                    "open_only": {
                        "type": "boolean",
                        "description": "For audit action: only return unresolved, non-outdated review threads.",
                        "default": false
                    }
                },
                "required": ["url"]
            }
        }),
    ]
}

fn dispatch_tool_call(params: &Value) -> Result<Value, anyhow::Error> {
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match tool_name {
        "gh_source_gate" => tool_gh_source_gate(&args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn tool_gh_source_gate(args: &Value) -> Result<Value, anyhow::Error> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or(".");
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("check");
    let pr = args.get("pr").and_then(Value::as_str);
    let open_only = args
        .get("open_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // Resolve repo root: if url is a path, use it; otherwise current dir
    let path = Path::new(url);
    let repo_root = if path.is_dir() {
        crate::find_git_root(path).unwrap_or_else(|_| path.to_path_buf())
    } else if path.exists() {
        path.parent()
            .map(|p| crate::find_git_root(p).unwrap_or_else(|_| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    } else {
        // Could be a URL; use current dir
        std::env::current_dir().unwrap_or_default()
    };

    // For PR URLs, extract the URL and pass as pr param
    let effective_pr = if url.contains("github.com") && url.contains("/pull/") {
        Some(url)
    } else {
        pr
    };

    match action {
        "check" => {
            let result =
                crate::inspect_pr_checks_json(&repo_root, effective_pr, 160, 30)?;
            let text = serde_json::to_string_pretty(&result)?;
            Ok(json!({
                "content": [{"type": "text", "text": text}],
                "metadata": {
                    "action": "check",
                    "failing_check_count": result.get("failing_check_count"),
                }
            }))
        }
        "audit" => {
            let result =
                crate::fetch_comments_json(&repo_root, effective_pr, open_only)?;
            let text = serde_json::to_string_pretty(&result)?;
            Ok(json!({
                "content": [{"type": "text", "text": text}],
                "metadata": {
                    "action": "audit",
                    "summary": result.get("summary"),
                }
            }))
        }
        "validate" => {
            let result = crate::doctor_json(&repo_root)?;
            let text = serde_json::to_string_pretty(&result)?;
            Ok(json!({
                "content": [{"type": "text", "text": text}],
                "metadata": {
                    "action": "validate",
                    "status": result.get("status"),
                }
            }))
        }
        _ => Err(anyhow::anyhow!(
            "Unknown action: {action}. Expected one of: check, audit, validate"
        )),
    }
}
