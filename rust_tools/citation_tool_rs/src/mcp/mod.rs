//! MCP tool definitions and dispatch for citation_tool_rs.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

use crate::{audit_report_to_markdown, claim_findings_to_markdown, lint_claims, make_report,
            parse_bibtex, read_text};

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

/// Dispatch a tool call by name and arguments.
pub fn dispatch(tool_name: &str, args: &Value) -> Result<Value> {
    match tool_name {
        "citation_audit" => tool_citation_audit(args),
        "citation_lint" => tool_citation_lint(args),
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
        .filter(|&v| v >= 1)
        .unwrap_or(3) as usize;

    let entries = parse_bibtex(&read_text(Path::new(bib_path))?)?;
    let manuscript_text = manuscript_path
        .map(read_text)
        .transpose()
        .map_err(|e| anyhow::anyhow!("failed to read manuscript: {e}"))?;
    let report = make_report(&entries, manuscript_text.as_deref(), cluster_threshold)?;

    let text = match format_str {
        "markdown" => audit_report_to_markdown(&report),
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
        .filter(|&v| v >= 1)
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
