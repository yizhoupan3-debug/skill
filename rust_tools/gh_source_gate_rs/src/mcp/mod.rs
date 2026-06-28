//! MCP tool definitions and dispatch for gh_source_gate_rs.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

use crate::{doctor_json, fetch_comments_json, find_git_root, inspect_pr_checks_json};

/// MCP tool definitions exposed by this server.
pub fn tool_definitions() -> Vec<Value> {
    vec![json!({
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
    })]
}

/// Dispatch a tool call by name and arguments.
pub fn dispatch(tool_name: &str, args: &Value) -> Result<Value> {
    match tool_name {
        "gh_source_gate" => tool_gh_source_gate(args),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn tool_gh_source_gate(args: &Value) -> Result<Value, anyhow::Error> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: url"))?;
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
        find_git_root(path).unwrap_or_else(|_| path.to_path_buf())
    } else if path.exists() {
        path.parent()
            .map(|p| find_git_root(p).unwrap_or_else(|_| p.to_path_buf()))
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
            let result = inspect_pr_checks_json(&repo_root, effective_pr, 160, 30)?;
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
            let result = fetch_comments_json(&repo_root, effective_pr, open_only)?;
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
            let result = doctor_json(&repo_root)?;
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
