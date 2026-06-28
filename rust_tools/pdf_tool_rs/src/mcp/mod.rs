//! MCP tool definitions and dispatch for pdf_tool_rs.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

/// Maximum pages per single MCP request before pagination kicks in.
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

/// Dispatch a tool call by name and arguments.
pub fn dispatch(tool_name: &str, args: &Value) -> Result<Value> {
    match tool_name {
        "pdf_read" => tool_pdf_read(args),
        "pdf_info" => tool_pdf_info(args),
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
    let pages_spec = args.get("pages").and_then(Value::as_str).unwrap_or("all");

    let pdf_path = Path::new(path);
    let total_pages = crate::read::page_count(pdf_path)? as u64;

    // Parse page range
    let requested_pages = mcp_stdio_common::util::parse_range(pages_spec, total_pages)?;
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
