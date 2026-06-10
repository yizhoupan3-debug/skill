//! MCP stdio server for pptx_tool_rs — exposes `pptx_parse` tool.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::{extract_pptx_structure, format_read_full_text, ZipBundle};

const SERVER_NAME: &str = "mcp-pptx";
const SERVER_VERSION: &str = "0.1.0";
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the MCP stdio server. Reads JSON-RPC lines from stdin, writes to stdout.
pub fn run_stdio_mcp() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(err) => {
                eprintln!("pptx MCP stdin read error: {err}");
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

/// Maximum slides per single MCP request before pagination kicks in.
const MAX_SLIDES_PER_REQUEST: u64 = 50;

/// MCP tool definitions exposed by this server.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "pptx_parse",
            "description": "Parse a PowerPoint (.pptx) file and extract slide content. Supports slide-range selection and pagination for large decks (max 50 slides per request).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the .pptx file"
                    },
                    "slides": {
                        "type": "string",
                        "description": "Slide range: '1-5', '3', '1,3,7-10', or 'all' (default). 1-indexed."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json", "markdown"],
                        "description": "Output format: 'text' (linear text, default), 'json' (full structure), 'markdown' (markdown with headings)"
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
        "pptx_parse" => tool_pptx_parse(&args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn tool_pptx_parse(args: &Value) -> Result<Value, anyhow::Error> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: path"))?;
    let format = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("text");
    let slides_spec = args
        .get("slides")
        .and_then(Value::as_str)
        .unwrap_or("all");

    let input = Path::new(path);
    let bundle = ZipBundle::from_path(input)?;
    let structure = extract_pptx_structure(&bundle, input, false, None)?;

    let total_slides = structure
        .get("slide_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let file_name = structure
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    // Parse slide range
    let requested_slides = parse_slide_range(slides_spec, total_slides)?;
    let truncated_slides = requested_slides.len() as u64 > MAX_SLIDES_PER_REQUEST;
    let effective_slides: Vec<u64> = if truncated_slides {
        requested_slides[..MAX_SLIDES_PER_REQUEST as usize].to_vec()
    } else {
        requested_slides
    };

    // Filter structure to only include requested slides
    let filtered_structure = filter_slides(&structure, &effective_slides);

    let text = match format {
        "json" => serde_json::to_string_pretty(&filtered_structure)?,
        "markdown" => format_structure_markdown(&filtered_structure),
        _ => format_read_full_text(&filtered_structure),
    };

    let mut warnings = Vec::new();
    if truncated_slides {
        warnings.push(format!(
            "pagination: showing slides {} of {} total (max {} per request)",
            effective_slides.len(),
            total_slides,
            MAX_SLIDES_PER_REQUEST,
        ));
    }

    let next_slide = if truncated_slides {
        effective_slides.last().map(|s| s + 1)
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
            "file": file_name,
            "total_slides": total_slides,
            "slides_requested": slides_spec,
            "slides_extracted": effective_slides.len(),
            "format": format,
            "truncated_slides": truncated_slides,
            "next_slide": next_slide,
            "warnings": warnings,
        }
    }))
}

/// Parse a slide range spec like "1-5", "3", "1,3,7-10", "all" into sorted unique slide numbers.
fn parse_slide_range(spec: &str, total_slides: u64) -> Result<Vec<u64>, anyhow::Error> {
    if spec == "all" || spec.is_empty() {
        return Ok((1..=total_slides).collect());
    }
    let mut slides = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let start: u64 = start.trim().parse().map_err(|_| anyhow::anyhow!("Invalid slide range start: {start}"))?;
            let end: u64 = end.trim().parse().map_err(|_| anyhow::anyhow!("Invalid slide range end: {end}"))?;
            if start == 0 || end == 0 || start > end || end > total_slides {
                return Err(anyhow::anyhow!("Slide range {start}-{end} out of bounds (total: {total_slides})"));
            }
            slides.extend(start..=end);
        } else {
            let slide: u64 = part.parse().map_err(|_| anyhow::anyhow!("Invalid slide number: {part}"))?;
            if slide == 0 || slide > total_slides {
                return Err(anyhow::anyhow!("Slide {slide} out of bounds (total: {total_slides})"));
            }
            slides.push(slide);
        }
    }
    slides.sort();
    slides.dedup();
    if slides.is_empty() {
        return Err(anyhow::anyhow!("No valid slides specified"));
    }
    Ok(slides)
}

/// Filter the PPTX structure JSON to only include slides with 1-based indices in the selection.
fn filter_slides(structure: &Value, selected: &[u64]) -> Value {
    let mut filtered = structure.clone();
    if let Some(slides) = filtered.get_mut("slides").and_then(|s| s.as_array_mut()) {
        slides.retain(|slide| {
            let idx = slide.get("index").and_then(Value::as_u64).unwrap_or(0);
            // index is 0-based, selected is 1-based
            selected.contains(&(idx + 1))
        });
    }
    // Update slide_count to reflect filtered count
    if let Some(obj) = filtered.as_object_mut() {
        if let Some(slides) = obj.get("slides").and_then(Value::as_array) {
            obj.insert("slide_count".to_string(), json!(slides.len()));
        }
    }
    filtered
}

/// Format structure as markdown with slide headings and bullet points.
fn format_structure_markdown(structure: &Value) -> String {
    let file = structure
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("deck.pptx");
    let slide_count = structure
        .get("slide_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut out = format!("# {file}\n\n**Slides:** {slide_count}\n\n---\n\n");

    let Some(slides) = structure.get("slides").and_then(Value::as_array) else {
        return out;
    };

    for slide in slides {
        let index = slide.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let slide_no = index + 1;
        out.push_str(&format!("## Slide {slide_no}\n\n"));

        if let Some(layout) = slide.get("layout").and_then(Value::as_str) {
            if !layout.is_empty() {
                out.push_str(&format!("*Layout: {layout}*\n\n"));
            }
        }

        if let Some(elements) = slide.get("elements").and_then(Value::as_array) {
            for element in elements {
                let name = element
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("shape");
                let element_type = element
                    .get("type")
                    .or_else(|| element.get("element_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("shape");

                // Text content
                if let Some(text) = element
                    .get("text")
                    .and_then(|t| t.get("fullText"))
                    .and_then(Value::as_str)
                {
                    if !text.trim().is_empty() {
                        let lower = name.to_lowercase();
                        if lower.contains("title") || lower.contains("subtitle") {
                            out.push_str(&format!("### {}\n\n", text.trim()));
                        } else {
                            for line in text.trim().lines() {
                                out.push_str(&format!("- {}\n", line));
                            }
                            out.push('\n');
                        }
                    }
                }

                // Table content
                if let Some(table) = element.get("table") {
                    if let Some(data) = table.get("data").and_then(Value::as_array) {
                        let cols = table.get("cols").and_then(Value::as_u64).unwrap_or(0);
                        if let Some(first_row) = data.first().and_then(Value::as_array) {
                            out.push_str("| ");
                            for cell in first_row {
                                let val = cell.as_str().unwrap_or("");
                                out.push_str(&format!("{val} | "));
                            }
                            out.push_str("\n| ");
                            for _ in 0..cols {
                                out.push_str("--- | ");
                            }
                            out.push('\n');
                            for row in data.iter().skip(1) {
                                if let Some(cells) = row.as_array() {
                                    out.push_str("| ");
                                    for cell in cells {
                                        let val = cell.as_str().unwrap_or("");
                                        out.push_str(&format!("{val} | "));
                                    }
                                    out.push('\n');
                                }
                            }
                            out.push('\n');
                        }
                    }
                }

                // Image note
                if element_type == "image" {
                    out.push_str(&format!("*Image: {name}*\n\n"));
                }

                // Group children (recursive)
                if element_type == "group" {
                    if let Some(children) = element.get("children").and_then(Value::as_array) {
                        for child in children {
                            if let Some(text) = child
                                .get("text")
                                .and_then(|t| t.get("fullText"))
                                .and_then(Value::as_str)
                            {
                                if !text.trim().is_empty() {
                                    out.push_str(&format!("- {}\n", text.trim()));
                                }
                            }
                        }
                        out.push('\n');
                    }
                }
            }
        }

        // Notes
        match slide.get("notes").and_then(Value::as_str) {
            Some(notes) if !notes.trim().is_empty() => {
                out.push_str(&format!("> **Notes:** {}\n\n", notes.trim()));
            }
            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "pptx_parse");
        assert!(tools[0]["inputSchema"]["properties"]["path"].is_object());
        assert!(tools[0]["inputSchema"]["properties"]["format"].is_object());
    }

    #[test]
    fn test_format_structure_markdown() {
        let structure = json!({
            "file": "test.pptx",
            "slide_count": 1,
            "slides": [{
                "index": 0,
                "layout": "Title Slide",
                "elements": [{
                    "index": 1,
                    "name": "Title 1",
                    "type": "shape",
                    "position": {"x": 0.0, "y": 0.0, "w": 10.0, "h": 1.0},
                    "text": {
                        "fullText": "Hello World",
                        "paragraphs": [{"text": "Hello World"}]
                    }
                }],
                "notes": null
            }],
            "available_layouts": []
        });
        let md = format_structure_markdown(&structure);
        assert!(md.contains("# test.pptx"));
        assert!(md.contains("## Slide 1"));
        assert!(md.contains("### Hello World"));
    }
}
