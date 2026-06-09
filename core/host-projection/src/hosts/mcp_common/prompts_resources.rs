use serde_json::{json, Value};
use std::fs;
use std::path::Path;

use super::cache::get_cached_task_view;
use super::host::mcp_host_hard_block_label;
use super::host::{list_known_task_ids, task_artifact_dir};

pub fn handle_prompts_list(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "prompts": [
                {
                    "name": "framework_routing",
                    "description": "framework routing guidance",
                    "arguments": [],
                },
                {
                    "name": "review_gate",
                    "description": "review gate advisory",
                    "arguments": [],
                },
                {
                    "name": "closeout_checklist",
                    "description": "closeout checklist",
                    "arguments": [],
                },
            ],
        },
    })
}

pub fn handle_prompts_get(id: Option<Value>, request: &Value, repo_root: &Path, host_id: &str) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let prompt_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let default_args = json!({});
    let _arguments = params.get("arguments").unwrap_or(&default_args);

    let description = match prompt_name {
        "framework_routing" => "framework routing",
        "review_gate" => "review gate advisory",
        "closeout_checklist" => "closeout checklist",
        _ => "",
    };

    let text = match prompt_name {
        "framework_routing" => {
            let source_rel = "skills/SKILL_ROUTING_RUNTIME.json";
            format!(
                "面向用户的回复必须使用简体中文（代码/路径/命令/第三方原文除外）。\n\n\
                 Use this repo shared framework runtime.\n\n\
                 1) Start from AGENTS.md.\n\
                 2) Route via {source_rel}.\n\
                 3) Read only the matched skill_path.\n\n\
                 Framework root: core/router-rs/"
            )
        }
        "review_gate" => {
            let host_name = mcp_host_hard_block_label(host_id);
            let gate_mode = format!(
                "{host_name} closeout is advisory — MCP tool layer reports findings but does not block goal_state_manage complete."
            );
            {
                let lanes = router_rs::runtime_registry::claude_reviewer_lanes_sorted(Some(repo_root));
                let lane_lines = if lanes.is_empty() {
                    "- (registry claude_reviewer_lanes unavailable)\n".to_string()
                } else {
                    lanes
                        .iter()
                        .map(|lane| format!("- {lane}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                format!(
                    "[Review Gate -- {host_name} gating]\n\n\
                     This host uses MCP transport; there is no shell hook REVIEW_GATE observation.\n\n\
                     Countable independent reviewer lanes (RUNTIME_REGISTRY review_gate.claude_reviewer_lanes):\n\
                     {lane_lines}\n\
                     explore / explorer does NOT count toward review evidence.\n\
                     Requires fork_context=false for independent reviewer credit (review-lanes/*.md on disk).\n\n\
                     When user requests review:\n\
                     1) Spawn a read-only reviewer in a claude_reviewer_lanes lane with fork_context=false\n\
                     2) If no subagent, decompose review dimensions locally and document findings\n\
                     3) Call closeout_gate before claiming review complete (review-lanes/*.md or reviewer_lane in args)\n\n\
                     {gate_mode}"
                )
            }
        }
        "closeout_checklist" => "[Closeout Checklist]\n\n\
             Before ending task:\n\
             - [ ] GOAL_STATE exists\n\
             - [ ] EVIDENCE_INDEX has >=1 record\n\
             - [ ] SESSION_SUMMARY written\n\
             - [ ] Verification evidence recorded\n\
             - [ ] Blockers in NEXT_ACTIONS\n\n\
             Call closeout_gate for machine-readable check."
            .to_string(),
        _ => format!("Unknown prompt: {prompt_name}"),
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "description": description,
            "messages": [
                {
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": text,
                    },
                },
            ],
        },
    })
}

pub fn handle_resources_list(id: Option<Value>, repo_root: &Path) -> Value {
    let task_view = get_cached_task_view(repo_root);

    let mut resources = vec![
        json!({
            "uri": "framework://active_task",
            "name": "Active Task",
            "description": "current active task pointer",
            "mimeType": "application/json",
        }),
        json!({
            "uri": "framework://goal_state",
            "name": "Goal State",
            "description": "goal state for current task",
            "mimeType": "application/json",
        }),
    ];

    let evidence_count = task_view
        .evidence
        .as_ref()
        .map(|e| {
            if e.evidence_rows_non_empty {
                1u64
            } else {
                0u64
            }
        })
        .unwrap_or(0);
    if evidence_count > 0 {
        resources.push(json!({
            "uri": "framework://evidence_index",
            "name": "Evidence Index",
            "description": format!("evidence index ({evidence_count} records)"),
            "mimeType": "application/json",
        }));
    }

    // session_summary is always listed as a resource if SESSION_SUMMARY.md exists
    let task_id = task_view
        .pointers
        .active_task_id
        .as_deref()
        .or(task_view.pointers.focus_task_id.as_deref());
    let summary_path = task_artifact_dir(repo_root, task_id).join("SESSION_SUMMARY.md");
    if summary_path.is_file() {
        resources.push(json!({
            "uri": "framework://session_summary",
            "name": "Session Summary",
            "description": "session checkpoint summary",
            "mimeType": "text/markdown",
        }));
    }

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "resources": resources },
    })
}

pub fn handle_resources_read(id: Option<Value>, request: &Value, repo_root: &Path) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");

    let (text, mime_type) = match uri {
        "framework://active_task" => {
            let task_view = get_cached_task_view(repo_root);
            let content = json!({
                "active_task_id": task_view.pointers.active_task_id,
                "focus_task_id": task_view.pointers.focus_task_id,
                "known_task_ids": list_known_task_ids(repo_root),
            });
            (
                serde_json::to_string_pretty(&content).unwrap_or_default(),
                "application/json",
            )
        }
        "framework://goal_state" => {
            let state = router_rs::goal_state::read_goal_state(repo_root, None);
            (
                serde_json::to_string_pretty(&state).unwrap_or_default(),
                "application/json",
            )
        }
        "framework://evidence_index" => {
            let task_view = get_cached_task_view(repo_root);
            let task_id = task_view
                .task_id
                .as_deref()
                .or(task_view.pointers.active_task_id.as_deref());
            let evidence_path = task_artifact_dir(repo_root, task_id).join("EVIDENCE_INDEX.json");
            let content = if evidence_path.is_file() {
                fs::read_to_string(&evidence_path).unwrap_or_else(|e| format!("Read error: {e}"))
            } else {
                "{}".to_string()
            };
            (content, "application/json")
        }
        "framework://session_summary" => {
            let task_view = get_cached_task_view(repo_root);
            let task_id = task_view
                .task_id
                .as_deref()
                .or(task_view.pointers.active_task_id.as_deref());
            let summary_path = task_artifact_dir(repo_root, task_id).join("SESSION_SUMMARY.md");
            let content = if summary_path.is_file() {
                fs::read_to_string(&summary_path).unwrap_or_else(|e| format!("Read error: {e}"))
            } else {
                String::new()
            };
            (content, "text/markdown")
        }
        _ => (format!("Unknown resource: {uri}"), "text/plain"),
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "contents": [
                {
                    "uri": uri,
                    "mimeType": mime_type,
                    "text": text,
                },
            ],
        },
    })
}
