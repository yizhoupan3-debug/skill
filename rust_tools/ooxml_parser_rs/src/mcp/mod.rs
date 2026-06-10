//! MCP stdio server for ooxml_parser_rs -- exposes `ooxml_parse` tool.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

const SERVER_NAME: &str = "mcp-ooxml";
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
                eprintln!("ooxml MCP stdin read error: {err}");
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
            "name": "ooxml_parse",
            "description": "Parse OOXML documents (.docx, .xlsx, .pptx). Auto-detects format by extension. Returns structured text (markdown tables for xlsx, linear text for docx) or JSON. Includes metadata such as sheet names, headings, dimensions, and SHA-256 hash.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the .docx, .xlsx, or .pptx file"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json", "markdown"],
                        "default": "text",
                        "description": "Output format. 'text' and 'markdown' produce human-readable markdown; 'json' returns structured data."
                    },
                    "max_rows": {
                        "type": "integer",
                        "default": 10000,
                        "description": "Maximum rows per sheet (xlsx only)"
                    },
                    "sheets": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Sheet names to include (xlsx only; empty = all)"
                    }
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
        "ooxml_parse" => tool_ooxml_parse(&args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn detect_kind(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".docx") || lower.ends_with(".docm") {
        "docx"
    } else if lower.ends_with(".xlsx") || lower.ends_with(".xlsm") {
        "xlsx"
    } else if lower.ends_with(".pptx") || lower.ends_with(".pptm") {
        "pptx"
    } else {
        "unknown"
    }
}

fn tool_ooxml_parse(args: &Value) -> Result<Value, anyhow::Error> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: path"))?;

    let format = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("text");

    let as_json = format == "json";

    let kind = detect_kind(path);
    let file_path = Path::new(path);

    if !file_path.is_file() {
        return Err(anyhow::anyhow!("File not found: {path}"));
    }

    let sha = crate::file_sha256(file_path)?;

    match kind {
        "docx" => {
            let output = crate::read_docx_content(file_path)?;
            let char_count = output.blocks.iter().map(|b| match b {
                crate::DocxBlock::Paragraph { text, .. } => text.len(),
                crate::DocxBlock::Table { rows } => rows.iter().flatten().map(|c| c.len()).sum(),
                crate::DocxBlock::Image => 7,
            }).sum::<usize>();

            if as_json {
                Ok(json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&output)?}],
                    "metadata": {
                        "path": output.path,
                        "sha256": sha,
                        "kind": "docx",
                        "block_count": output.blocks.len(),
                        "char_count": char_count,
                        "footnote_count": output.footnotes.len(),
                        "endnote_count": output.endnotes.len(),
                        "comment_count": output.comments.len(),
                    }
                }))
            } else {
                let text = crate::docx_read_text_string(&output);
                Ok(json!({
                    "content": [{"type": "text", "text": text}],
                    "metadata": {
                        "path": output.path,
                        "sha256": sha,
                        "kind": "docx",
                        "block_count": output.blocks.len(),
                        "char_count": char_count,
                        "footnote_count": output.footnotes.len(),
                        "endnote_count": output.endnotes.len(),
                        "comment_count": output.comments.len(),
                    }
                }))
            }
        }
        "xlsx" => {
            let max_rows = args.get("max_rows").and_then(Value::as_u64).unwrap_or(10000) as usize;
            let sheets: Vec<String> = args
                .get("sheets")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let output = crate::read_xlsx_content(file_path, max_rows, &sheets)?;
            let total_rows: usize = output.sheets.iter().map(|s| s.rows.len()).sum();
            let truncated = output.sheets.iter().any(|s| s.truncated);

            if as_json {
                Ok(json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&output)?}],
                    "metadata": {
                        "path": output.path,
                        "sha256": sha,
                        "kind": "xlsx",
                        "sheet_count": output.sheets.len(),
                        "total_rows": total_rows,
                        "truncated": truncated,
                    }
                }))
            } else {
                let text = crate::xlsx_read_text_string(&output);
                Ok(json!({
                    "content": [{"type": "text", "text": text}],
                    "metadata": {
                        "path": output.path,
                        "sha256": sha,
                        "kind": "xlsx",
                        "sheet_count": output.sheets.len(),
                        "total_rows": total_rows,
                        "truncated": truncated,
                    }
                }))
            }
        }
        "pptx" => {
            let file = std::fs::File::open(file_path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut slide_count = 0usize;
            for i in 0..archive.len() {
                let f = archive.by_index(i)?;
                if f.name().starts_with("ppt/slides/slide") && f.name().ends_with(".xml") {
                    slide_count += 1;
                }
            }

            let summary_text = format!(
                "PPTX: {}\nSlides: {}\nSHA-256: {}",
                path, slide_count, sha
            );

            Ok(json!({
                "content": [{"type": "text", "text": summary_text}],
                "metadata": {
                    "path": path,
                    "sha256": sha,
                    "kind": "pptx",
                    "slide_count": slide_count,
                }
            }))
        }
        _ => Err(anyhow::anyhow!(
            "Unsupported file extension for: {path} (expected .docx, .xlsx, or .pptx)"
        )),
    }
}
