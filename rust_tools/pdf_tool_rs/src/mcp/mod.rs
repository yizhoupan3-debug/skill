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

/// Maximum pages/slides per single MCP request before pagination kicks in.
const MAX_PAGES_PER_REQUEST: u64 = 50;

/// MCP tool definitions exposed by this server.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "pdf_read",
            "description": "Extract text content from a PDF file. Supports page-range selection and pagination for large documents (max 50 pages per request).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to the PDF file"},
                    "pages": {"type": "string", "description": "Page range: '1-5', '3', '1,3,7-10', or 'all' (default). 1-indexed."},
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
    let pages_spec = args
        .get("pages")
        .and_then(Value::as_str)
        .unwrap_or("all");

    let pdf_path = Path::new(path);
    let total_pages = crate::read::page_count(pdf_path)? as u64;

    // Parse page range
    let requested_pages = parse_page_range(pages_spec, total_pages)?;
    let truncated_pages = requested_pages.len() as u64 > MAX_PAGES_PER_REQUEST;
    let effective_pages: Vec<u64> = if truncated_pages {
        requested_pages[..MAX_PAGES_PER_REQUEST as usize].to_vec()
    } else {
        requested_pages
    };

    // Extract text by pages using pdf-extract's per-page API
    let all_pages = pdf_extract::extract_text_by_pages(pdf_path)
        .map_err(|e| anyhow::anyhow!("PDF extraction error: {e:#}"))?;

    let mut selected_text = String::new();
    let mut pages_extracted = 0u64;
    for &page_idx in &effective_pages {
        // page_idx is 1-based, Vec is 0-based
        if let Some(page_text) = all_pages.get((page_idx - 1) as usize) {
            if !selected_text.is_empty() {
                selected_text.push_str("\n\n--- Page {page_idx} ---\n\n");
            }
            selected_text.push_str(page_text);
            pages_extracted += 1;
        }
    }

    let char_count = selected_text.chars().count();
    let truncated_chars = char_count > max_chars;
    let text: String = selected_text.chars().take(max_chars).collect();

    let mut warnings = Vec::new();
    if truncated_pages {
        warnings.push(format!(
            "pagination: showing pages {} of {} total (max {} per request)",
            effective_pages.len(),
            total_pages,
            MAX_PAGES_PER_REQUEST,
        ));
    }
    if truncated_chars {
        warnings.push("text_truncated_by_max_chars".to_string());
    }

    // Build pagination token if there are more pages
    let next_page = if truncated_pages {
        effective_pages.last().map(|p| p + 1)
    } else {
        None
    };

    Ok(json!({
        "content": [{
            "type": "text",
            "text": text,
        }],
        "metadata": {
            "path": path,
            "total_pages": total_pages,
            "pages_requested": pages_spec,
            "pages_extracted": pages_extracted,
            "char_count": char_count,
            "truncated_chars": truncated_chars,
            "truncated_pages": truncated_pages,
            "next_page": next_page,
            "warnings": warnings,
        }
    }))
}

/// Parse a page range spec like "1-5", "3", "1,3,7-10", "all" into sorted unique page numbers.
fn parse_page_range(spec: &str, total_pages: u64) -> Result<Vec<u64>, anyhow::Error> {
    if spec == "all" || spec.is_empty() {
        return Ok((1..=total_pages).collect());
    }
    let mut pages = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let start: u64 = start.trim().parse().map_err(|_| anyhow::anyhow!("Invalid page range start: {start}"))?;
            let end: u64 = end.trim().parse().map_err(|_| anyhow::anyhow!("Invalid page range end: {end}"))?;
            if start == 0 || end == 0 || start > end || end > total_pages {
                return Err(anyhow::anyhow!("Page range {start}-{end} out of bounds (total: {total_pages})"));
            }
            pages.extend(start..=end);
        } else {
            let page: u64 = part.parse().map_err(|_| anyhow::anyhow!("Invalid page number: {part}"))?;
            if page == 0 || page > total_pages {
                return Err(anyhow::anyhow!("Page {page} out of bounds (total: {total_pages})"));
            }
            pages.push(page);
        }
    }
    pages.sort();
    pages.dedup();
    if pages.is_empty() {
        return Err(anyhow::anyhow!("No valid pages specified"));
    }
    Ok(pages)
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
