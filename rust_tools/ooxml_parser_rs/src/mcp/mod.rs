//! MCP tool definitions and dispatch for ooxml_parser_rs.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

use crate::{OoxmlKind, detect_ooxml_kind};

/// MCP tool definitions exposed by this server.
pub fn tool_definitions() -> Vec<Value> {
    vec![json!({
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
    })]
}

/// Dispatch a tool call by name and arguments.
pub fn dispatch(tool_name: &str, args: &Value) -> Result<Value> {
    match tool_name {
        "ooxml_parse" => tool_ooxml_parse(args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn tool_ooxml_parse(args: &Value) -> Result<Value, anyhow::Error> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: path"))?;

    let format = args.get("format").and_then(Value::as_str).unwrap_or("text");

    let as_json = format == "json";

    let file_path = Path::new(path);
    let kind = detect_ooxml_kind(file_path);

    if !file_path.is_file() {
        return Err(anyhow::anyhow!("File not found: {path}"));
    }

    let sha = crate::file_sha256(file_path)?;

    match kind {
        OoxmlKind::Docx => {
            let output = crate::read_docx_content(file_path)?;
            let char_count = output
                .blocks
                .iter()
                .map(|b| match b {
                    crate::DocxBlock::Paragraph { text, .. } => text.len(),
                    crate::DocxBlock::Table { rows } => {
                        rows.iter().flatten().map(|c| c.len()).sum()
                    }
                    crate::DocxBlock::Image => 7,
                })
                .sum::<usize>();

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
        OoxmlKind::Xlsx => {
            let max_rows = args
                .get("max_rows")
                .and_then(Value::as_u64)
                .unwrap_or(10000) as usize;
            let sheets: Vec<String> = args
                .get("sheets")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
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
        OoxmlKind::Pptx => {
            let structure = crate::read_pptx_content(file_path)?;
            let slide_count = structure.slide_count;
            let canonical_path = structure.path.clone();

            if as_json {
                Ok(json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&structure)?}],
                    "metadata": {
                        "path": canonical_path,
                        "sha256": sha,
                        "kind": "pptx",
                        "slide_count": slide_count,
                    }
                }))
            } else {
                let text = crate::pptx_read_text_string(&structure);
                Ok(json!({
                    "content": [{"type": "text", "text": text}],
                    "metadata": {
                        "path": canonical_path,
                        "sha256": sha,
                        "kind": "pptx",
                        "slide_count": slide_count,
                    }
                }))
            }
        }
        OoxmlKind::Unsupported => Err(anyhow::anyhow!(
            "Unsupported file extension for: {path} (expected .docx, .xlsx, or .pptx)"
        )),
    }
}
