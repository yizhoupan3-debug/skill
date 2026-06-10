//! MCP stdio server for citation_tool_rs — exposes `citation_audit` and `citation_lint` tools.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::{audit_report_to_markdown, claim_findings_to_markdown, lint_claims, make_report,
            parse_bibtex, read_text};

const SERVER_NAME: &str = "mcp-citation";
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
                eprintln!("citation MCP stdin read error: {err}");
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
            "name": "citation_audit",
            "description": "Audit a BibTeX bibliography for duplicates, missing required fields, missing DOI, and optional manuscript cross-reference consistency. Returns a structured report in JSON or Markdown.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "bib_path": {
                        "type": "string",
                        "description": "Absolute path to the .bib file"
                    },
                    "manuscript_path": {
                        "type": "string",
                        "description": "Optional absolute path to a manuscript file for cross-reference consistency check"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "markdown"],
                        "description": "Output format (default: json)",
                        "default": "json"
                    },
                    "cluster_threshold": {
                        "type": "integer",
                        "description": "Minimum citations in a sentence to flag as dense cluster (default: 3)",
                        "default": 3
                    }
                },
                "required": ["bib_path"]
            }
        }),
        json!({
            "name": "citation_lint",
            "description": "Lint a manuscript for dense citation clusters and sentence-ending stacked citations. Returns flagged sentences with reasons.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "manuscript_path": {
                        "type": "string",
                        "description": "Absolute path to the manuscript file"
                    },
                    "threshold": {
                        "type": "integer",
                        "description": "Minimum citations in a sentence to flag (default: 3)",
                        "default": 3
                    }
                },
                "required": ["manuscript_path"]
            }
        }),
    ]
}

fn dispatch_tool_call(params: &Value) -> Result<Value, anyhow::Error> {
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    match tool_name {
        "citation_audit" => tool_citation_audit(&args),
        "citation_lint" => tool_citation_lint(&args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn tool_citation_audit(args: &Value) -> Result<Value, anyhow::Error> {
    let bib_path = args
        .get("bib_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: bib_path"))?;
    let manuscript_path = args
        .get("manuscript_path")
        .and_then(Value::as_str)
        .map(Path::new);
    let format_str = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("json");
    let cluster_threshold = args
        .get("cluster_threshold")
        .and_then(Value::as_u64)
        .unwrap_or(3) as usize;

    let entries = parse_bibtex(&read_text(Path::new(bib_path))?)?;
    let manuscript_text = manuscript_path
        .map(read_text)
        .transpose()
        .map_err(|e| anyhow::anyhow!("failed to read manuscript: {e}"))?;
    let report = make_report(&entries, manuscript_text.as_deref(), cluster_threshold)?;

    let text = match format_str {
        "markdown" | "md" => audit_report_to_markdown(&report),
        _ => serde_json::to_string_pretty(&report)?,
    };

    Ok(json!({
        "content": [{"type": "text", "text": text}],
        "metadata": {
            "total_entries": report.summary.total_entries,
            "duplicate_groups": report.summary.duplicate_groups,
            "blocking_issues": report.summary.blocking_issue_count,
            "warning_issues": report.summary.warning_issue_count,
        }
    }))
}

fn tool_citation_lint(args: &Value) -> Result<Value, anyhow::Error> {
    let manuscript_path = args
        .get("manuscript_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: manuscript_path"))?;
    let threshold = args
        .get("threshold")
        .and_then(Value::as_u64)
        .unwrap_or(3) as usize;

    let text = read_text(Path::new(manuscript_path))?;
    let findings = lint_claims(&text, threshold)?;
    let rendered = claim_findings_to_markdown(&findings);

    Ok(json!({
        "content": [{"type": "text", "text": rendered}],
        "metadata": {
            "finding_count": findings.len(),
            "threshold": threshold,
        }
    }))
}
