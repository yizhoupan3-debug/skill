//! MCP stdio server for pdf_tool_rs — exposes `pdf_read` and `pdf_info` tools.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

const SERVER_NAME: &str = "mcp-pdf";
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
                eprintln!("pdf MCP stdin read error: {err}");
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
            "name": "pdf_read",
            "description": "Extract text content from a PDF file. Returns plain text with page count and metadata.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the PDF file"},
                    "max_chars": {"type": "integer", "description": "Maximum characters to extract (default 8000)", "default": 8000}
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "pdf_info",
            "description": "Show PDF metadata (page count, size, content class) without full text extraction.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the PDF file"},
                    "preview_chars": {"type": "integer", "description": "Characters of text preview (default 200)", "default": 200}
                },
                "required": ["path"]
            }
        }),
    ]
}

fn dispatch_tool_call(params: &Value) -> Result<Value, anyhow::Error> {
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match tool_name {
        "pdf_read" => tool_pdf_read(&args),
        "pdf_info" => tool_pdf_info(&args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn tool_pdf_read(args: &Value) -> Result<Value, anyhow::Error> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: path"))?;
    let max_chars = args
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(8000) as usize;
    let opts = crate::read::ReadOptions {
        max_chars,
        text_out_dir: None,
    };
    let out = crate::read::read_pdf(Path::new(path), &opts)?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": out.text,
        }],
        "metadata": {
            "path": path,
            "sha256": out.file_sha256,
            "page_count": out.page_count,
            "content_class": out.content_class.as_str().to_string(),
            "char_count": out.text.chars().count(),
            "truncated": out.truncated,
            "warnings": out.warnings,
        }
    }))
}

fn tool_pdf_info(args: &Value) -> Result<Value, anyhow::Error> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: path"))?;
    let preview_chars = args
        .get("preview_chars")
        .and_then(Value::as_u64)
        .unwrap_or(200) as usize;
    let meta = crate::info::pdf_info(Path::new(path), preview_chars)?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!(
                "path: {}\nsha256: {}\npages: {}\nsize_bytes: {}\ncontent_class: {}\ntext_preview_chars: {}",
                meta.path, meta.sha256, meta.page_count, meta.file_size_bytes,
                meta.content_class, meta.text_preview_chars
            ),
        }],
        "metadata": {
            "path": meta.path,
            "sha256": meta.sha256,
            "page_count": meta.page_count,
            "file_size_bytes": meta.file_size_bytes,
            "content_class": meta.content_class,
            "warnings": meta.warnings,
        }
    }))
}
