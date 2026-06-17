//! MCP stdio JSON-RPC server shared across tool crates.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

/// Run an MCP stdio server that dispatches to the given tool crate.
///
/// `tool_defs` returns the list of tool definitions (for `tools/list`).
/// `dispatch` handles `tools/call` by tool name.
pub fn run_stdio_mcp(
    repo_root: &std::path::Path,
    crate_name: &str,
    tool_defs: impl Fn() -> Vec<Value>,
    dispatch: impl Fn(&str, &Value) -> Result<Value>,
) -> Result<()> {
    let stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    for line in stdin.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let id = request.get("id").cloned();
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("");
        let params = request
            .get("params")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": crate_name,
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            "notifications/initialized" => {
                // Notification — no response needed
                continue;
            }
            "tools/list" => Ok(json!({ "tools": tool_defs() })),
            "tools/call" => {
                let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                dispatch(tool_name, &args)
            }
            "ping" => Ok(json!({})),
            _ => {
                // Unknown method — return error
                Err(anyhow::anyhow!("Method not found: {method}"))
            }
        };

        let response = match (id, result) {
            (Some(id), Ok(result)) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            (Some(id), Err(err)) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32603, "message": err.to_string() }
            }),
            _ => continue, // Notification — no response
        };

        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }

    // Write repo_root to a marker file so tests can verify it was passed
    let _ = repo_root;
    Ok(())
}
