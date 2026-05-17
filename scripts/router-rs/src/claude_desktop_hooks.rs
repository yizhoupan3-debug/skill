//! Claude Desktop MCP agent: `router-rs claude-desktop agent --repo-root …`。
//!
//! MCP 服务器（stdio transport），提供 tools / prompts / resources 三类端点，
//! 替代 Claude Code CLI 的 shell hook 协议（PreToolUse / UserPromptSubmit / PostToolUse / Stop）。
//!
//! 架构约束：MCP 不支持工具拦截，因此 PreToolUse guards（framework/settings path guard、
//! dangerous bash guard）在 Desktop 上不可用，依赖 CLAUDE.md 指令自律。
//! Stop / UserPromptSubmit hard block 降级为 advisory（MCP tool 返回提示而无法阻止代理停止）。
//!
//! 与 CLI 共享同一 L2/L3 数据源（连续性 digest、evidence、goal state、路由），
//! 但出站形态为 MCP JSON-RPC 响应，非 hook JSON。

use crate::framework_runtime::{
    build_automatic_continuity_checkpoint_payload,
    build_framework_continuity_digest_prompt, build_framework_runtime_snapshot_envelope,
    resolve_repo_root_arg, try_append_post_tool_shell_evidence,
};
use crate::route::{load_records, route_task};
use crate::task_state::resolve_task_view;
use serde_json::{json, Value};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "router-rs-framework";
const SERVER_VERSION: &str = "0.1.0-rust";
const MAX_MCP_CONTENT_LENGTH: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum McpTransportMode {
    ContentLength,
    NewlineDelimited,
}

pub fn run_claude_desktop_mcp_loop(repo_root_arg: Option<&Path>) -> Result<(), String> {
    let repo_root = resolve_repo_root_arg(repo_root_arg)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_mcp_stdio(stdin.lock(), stdout.lock(), &repo_root)
}

fn run_mcp_stdio<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    repo_root: &Path,
) -> Result<(), String> {
    let mut transport_mode = None;
    while let Some(message) = read_mcp_message(&mut input, &mut transport_mode)? {
        if let Some(response) = handle_mcp_request(&message, repo_root) {
            write_mcp_response(
                &mut output,
                transport_mode.unwrap_or(McpTransportMode::NewlineDelimited),
                &response,
            )?;
        }
    }
    Ok(())
}

fn read_mcp_message<R: BufRead>(
    input: &mut R,
    transport_mode: &mut Option<McpTransportMode>,
) -> Result<Option<String>, String> {
    let mut first_line = String::new();
    loop {
        first_line.clear();
        let bytes = input
            .read_line(&mut first_line)
            .map_err(|err| format!("read MCP request failed: {err}"))?;
        if bytes == 0 {
            return Ok(None);
        }
        if !first_line.trim().is_empty() {
            break;
        }
    }

    if first_line
        .to_ascii_lowercase()
        .starts_with("content-length:")
    {
        *transport_mode = Some(McpTransportMode::ContentLength);
        let content_length = parse_content_length(&first_line)?;
        if content_length > MAX_MCP_CONTENT_LENGTH {
            return Err(format!(
                "MCP Content-Length {content_length} exceeds max {MAX_MCP_CONTENT_LENGTH}"
            ));
        }
        loop {
            let mut header = String::new();
            let bytes = input
                .read_line(&mut header)
                .map_err(|err| format!("read MCP header failed: {err}"))?;
            if bytes == 0 {
                return Err("MCP header ended before blank line".to_string());
            }
            if header.trim().is_empty() {
                break;
            }
        }
        let mut body = vec![0_u8; content_length];
        input
            .read_exact(&mut body)
            .map_err(|err| format!("read MCP body failed: {err}"))?;
        return String::from_utf8(body)
            .map(Some)
            .map_err(|err| format!("decode MCP body failed: {err}"));
    }

    if transport_mode.is_none() {
        *transport_mode = Some(McpTransportMode::NewlineDelimited);
    }
    Ok(Some(first_line.trim_end().to_string()))
}

fn parse_content_length(line: &str) -> Result<usize, String> {
    let (_, value) = line
        .split_once(':')
        .ok_or_else(|| format!("invalid MCP header: {line}"))?;
    value
        .trim()
        .parse::<usize>()
        .map_err(|err| format!("invalid MCP content length '{value}': {err}"))
}

fn write_mcp_response<W: Write>(
    output: &mut W,
    transport_mode: McpTransportMode,
    response: &Value,
) -> Result<(), String> {
    let encoded = serde_json::to_string(response)
        .map_err(|err| format!("serialize MCP response failed: {err}"))?;
    match transport_mode {
        McpTransportMode::ContentLength => {
            write!(
                output,
                "Content-Length: {}\r\n\r\n{encoded}",
                encoded.len()
            )
            .map_err(|err| format!("write MCP response failed: {err}"))?;
        }
        McpTransportMode::NewlineDelimited => {
            writeln!(output, "{encoded}")
                .map_err(|err| format!("write MCP response failed: {err}"))?;
        }
    }
    Ok(())
}

fn handle_mcp_request(message: &str, repo_root: &Path) -> Option<Value> {
    let request: Value = match serde_json::from_str(message) {
        Ok(v) => v,
        Err(err) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "error": {"code": -32700, "message": format!("Parse error: {err}")},
            }));
        }
    };
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");

    match method {
        "initialize" => Some(handle_initialize(id)),
        "notifications/initialized" => None,
        "tools/list" => Some(handle_tools_list(id)),
        "tools/call" => Some(handle_tools_call(id, &request, repo_root)),
        "prompts/list" => Some(handle_prompts_list(id)),
        "prompts/get" => Some(handle_prompts_get(id, &request, repo_root)),
        "resources/list" => Some(handle_resources_list(id, repo_root)),
        "resources/read" => Some(handle_resources_read(id, &request, repo_root)),
        "ping" => Some(json!({"jsonrpc": "2.0", "id": id, "result": {}})),
        _ => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": format!("Method not found: {method}")},
        })),
    }
}

fn handle_initialize(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION,
            },
            "capabilities": {
                "tools": {},
                "prompts": {},
                "resources": {},
            },
        },
    })
}

fn handle_tools_list(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
                {
                    "name": "framework_digest",
                    "description": "返回当前会话的连续性 digest（与 Claude Code CLI SessionStart 同源），含 active task、evidence count、goal state、verification status 等。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "max_lines": {
                                "type": "integer",
                                "description": "最大返回行数，默认 40",
                                "default": 40,
                                "minimum": 1,
                                "maximum": 120,
                            },
                        },
                    },
                },
                {
                    "name": "framework_snapshot",
                    "description": "返回当前仓库的框架运行时快照（与 `router-rs framework snapshot` 同源），含完整连续性视图。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                    },
                },
                {
                    "name": "skill_route",
                    "description": "传入自然语言查询，返回匹配的 skill 路由结果（与热路由 `SKILL_ROUTING_RUNTIME.json` 同源）。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "自然语言查询",
                            },
                        },
                        "required": ["query"],
                    },
                },
                {
                    "name": "record_evidence",
                    "description": "追加一条 evidence 记录到当前 task 的 EVIDENCE_INDEX（与 CLI PostToolUse 自动追加同形）。agent 应在执行验证类命令后主动调用。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "tool_name": {
                                "type": "string",
                                "description": "工具名（如 Bash、Read、Write）",
                            },
                            "command": {
                                "type": "string",
                                "description": "执行的命令或操作描述",
                            },
                            "exit_code": {
                                "type": "integer",
                                "description": "exit code，0 表示成功",
                            },
                            "output": {
                                "type": "string",
                                "description": "命令输出摘要（可选，最多 2000 字符）",
                            },
                        },
                        "required": ["tool_name", "command"],
                    },
                },
                {
                    "name": "session_checkpoint",
                    "description": "写入 SESSION_SUMMARY 和 NEXT_ACTIONS checkpoint（与 CLI Stop 自动写入同形）。agent 应在完成阶段性工作时主动调用。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "summary": {
                                "type": "string",
                                "description": "当前会话进展摘要",
                            },
                            "next_actions": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "下一步行动列表",
                            },
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                        "required": ["summary"],
                    },
                },
                {
                    "name": "closeout_gate",
                    "description": "返回 closeout 状态与缺失项清单（与 CLI Stop 门控同源但为 advisory —— MCP 不可硬拦）。agent 应在收尾前调用以自检。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                    },
                },
                {
                    "name": "goal_state_read",
                    "description": "读取当前 task 的 GOAL_STATE.json 内容。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                    },
                },
            ],
        },
    })
}

fn handle_tools_call(id: Option<Value>, request: &Value, repo_root: &Path) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let default_args = json!({});
    let arguments = params
        .get("arguments")
        .unwrap_or(&default_args);

    let result = match tool_name {
        "framework_digest" => tool_framework_digest(arguments, repo_root),
        "framework_snapshot" => tool_framework_snapshot(repo_root),
        "skill_route" => tool_skill_route(arguments, repo_root),
        "record_evidence" => tool_record_evidence(arguments, repo_root),
        "session_checkpoint" => tool_session_checkpoint(arguments, repo_root),
        "closeout_gate" => tool_closeout_gate(arguments, repo_root),
        "goal_state_read" => tool_goal_state_read(arguments, repo_root),
        _ => Err(format!("Unknown tool: {tool_name}")),
    };

    match result {
        Ok(content) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": content }],
            },
        }),
        Err(err) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Error: {err}") }],
                "isError": true,
            },
        }),
    }
}

fn tool_framework_digest(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let max_lines = arguments
        .get("max_lines")
        .and_then(Value::as_u64)
        .unwrap_or(40)
        .clamp(1, 120) as usize;
    build_framework_continuity_digest_prompt(repo_root, max_lines)
}

fn tool_framework_snapshot(repo_root: &Path) -> Result<String, String> {
    let envelope = build_framework_runtime_snapshot_envelope(repo_root, None, None)?;
    serde_json::to_string_pretty(&envelope).map_err(|e| e.to_string())
}

fn tool_skill_route(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: query")?;
    let records = load_records(Some(repo_root), None)?;
    let decision = route_task(&records, query, "session", false, true)?;
    if decision.selected_skill == "none" || decision.selected_skill.is_empty() {
        return Ok(json!({
            "routed": false,
            "skill_slug": null,
            "skill_path": null,
            "match_reason": "no match",
        })
        .to_string());
    }
    Ok(json!({
        "routed": true,
        "skill_slug": decision.selected_skill,
        "skill_path": decision.selected_skill_path,
        "match_reason": decision.reasons.get(0).cloned().unwrap_or_default(),
    })
    .to_string())
}

fn tool_record_evidence(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let tool_name = arguments
        .get("tool_name")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: tool_name")?;
    let command = arguments
        .get("command")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: command")?;
    let exit_code = arguments.get("exit_code").and_then(Value::as_i64);
    let output = arguments
        .get("output")
        .and_then(Value::as_str);

    let mut synthetic = json!({
        "tool_name": tool_name,
        "tool_input": { "command": command },
    });
    if let Some(ec) = exit_code {
        synthetic["exit_code"] = json!(ec);
    }

    try_append_post_tool_shell_evidence(repo_root, &synthetic, "mcp_record_evidence")?;
    if let Some(text) = output {
        let trimmed = text.chars().take(2000).collect::<String>();
        Ok(format!("Evidence recorded: {tool_name} '{command}' -> exit={exit_code:?}\n{trimmed}"))
    } else {
        Ok(format!("Evidence recorded: {tool_name} '{command}' -> exit={exit_code:?}"))
    }
}

fn tool_session_checkpoint(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: summary")?;
    let next_actions: Vec<String> = arguments
        .get("next_actions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let _task_id = arguments
        .get("task_id")
        .and_then(Value::as_str);

    let payload = build_automatic_continuity_checkpoint_payload(
        repo_root,
        summary,
        &next_actions.join(", "),
    );
    let _ = crate::framework_runtime::write_framework_session_artifacts(payload);
    Ok(format!(
        "Checkpoint written: summary={}, next_actions_count={}",
        summary.chars().count(),
        next_actions.len()
    ))
}

fn tool_closeout_gate(_arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let task_view = resolve_task_view(repo_root, None);
    let mut findings: Vec<String> = Vec::new();

    let goal_present = task_view.goal_state.is_some();
    if !goal_present {
        findings.push("goal_state: no GOAL_STATE.json".to_string());
    } else {
        findings.push("goal_state: present".to_string());
    }

    let evidence_count = task_view
        .evidence
        .as_ref()
        .map(|e| if e.evidence_rows_non_empty { 1u64 } else { 0u64 })
        .unwrap_or(0);
    if evidence_count == 0 {
        findings.push("evidence: no EVIDENCE_INDEX records".to_string());
    } else {
        findings.push(format!("evidence: {evidence_count} records"));
    }

    let has_summary = task_view.resolution_notes.iter().any(|n| n.contains("checkpoint") || n.contains("summary"));
    if !has_summary {
        findings.push("checkpoint: no SESSION_SUMMARY".to_string());
    } else {
        findings.push("checkpoint: SESSION_SUMMARY present".to_string());
    }

    let all_clear = goal_present && evidence_count > 0 && has_summary;
    let verdict = if all_clear {
        "PASS: all closeout gates satisfied (advisory)"
    } else {
        "ADVISORY: some gates not satisfied (MCP cannot hard-block, self-discipline required)"
    };

    Ok(format!("[Closeout Gate] {verdict}\n\n{}", findings.join("\n")))
}

fn tool_goal_state_read(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let state = crate::autopilot_goal::read_goal_state(repo_root, task_id);
    serde_json::to_string_pretty(&state).map_err(|e| e.to_string())
}

fn handle_prompts_list(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "prompts": [
                {
                    "name": "continuity_digest",
                    "description": "digest for session continuity",
                    "arguments": [
                        {
                            "name": "max_lines",
                            "description": "max lines (default 40)",
                            "required": false,
                        },
                    ],
                },
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

fn handle_prompts_get(id: Option<Value>, request: &Value, repo_root: &Path) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let prompt_name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let default_args = json!({});
    let arguments = params.get("arguments").unwrap_or(&default_args);

    let description = match prompt_name {
        "continuity_digest" => "continuity digest",
        "framework_routing" => "framework routing",
        "review_gate" => "review gate advisory",
        "closeout_checklist" => "closeout checklist",
        _ => "",
    };

    let text = match prompt_name {
        "continuity_digest" => {
            let max_lines = arguments
                .get("max_lines")
                .and_then(Value::as_u64)
                .unwrap_or(40)
                .clamp(1, 120) as usize;
            build_framework_continuity_digest_prompt(repo_root, max_lines)
                .unwrap_or_else(|e| format!("Digest unavailable: {e}"))
        }
        "framework_routing" => {
            let source_rel = "skills/SKILL_ROUTING_RUNTIME.json";
            format!(
                "Use this repo shared framework runtime.\n\n\
                 1) Start from AGENTS.md.\n\
                 2) Route via {source_rel}.\n\
                 3) Read only the matched skill_path.\n\n\
                 Framework root: scripts/router-rs/"
            )
        }
        "review_gate" => {
            "[Review Gate -- Claude Desktop advisory]\n\n\
             This host uses MCP transport, no shell hook observation.\n\n\
             When user requests review:\n\
             1) Use subagent with fork_context=false for independent reviewer\n\
             2) If no subagent, decompose review dimensions locally\n\
             3) Call closeout_gate after review\n\n\
             Desktop review gate is advisory only."
                .to_string()
        }
        "closeout_checklist" => {
            "[Closeout Checklist]\n\n\
             Before ending task:\n\
             - [ ] GOAL_STATE exists\n\
             - [ ] EVIDENCE_INDEX has >=1 record\n\
             - [ ] SESSION_SUMMARY written\n\
             - [ ] Verification evidence recorded\n\
             - [ ] Blockers in NEXT_ACTIONS\n\n\
             Call closeout_gate for machine-readable check."
                .to_string()
        }
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

fn handle_resources_list(id: Option<Value>, repo_root: &Path) -> Value {
    let task_view = resolve_task_view(repo_root, None);

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
        .map(|e| if e.evidence_rows_non_empty { 1u64 } else { 0u64 })
        .unwrap_or(0);
    if evidence_count > 0 {
        resources.push(json!({
            "uri": "framework://evidence_index",
            "name": "Evidence Index",
            "description": format!("evidence index ({evidence_count} records)"),
            "mimeType": "application/json",
        }));
    }

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "resources": resources },
    })
}

fn handle_resources_read(id: Option<Value>, request: &Value, repo_root: &Path) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .unwrap_or("");

    let (text, mime_type) = match uri {
        "framework://active_task" => {
            let task_view = resolve_task_view(repo_root, None);
            let content = json!({
                "active_task_id": task_view.pointers.active_task_id,
                "focus_task_id": task_view.pointers.focus_task_id,
                "known_task_ids": Vec::<String>::new(),
            });
            (serde_json::to_string_pretty(&content).unwrap_or_default(),
             "application/json")
        }
        "framework://goal_state" => {
            let state = crate::autopilot_goal::read_goal_state(repo_root, None);
            (serde_json::to_string_pretty(&state).unwrap_or_default(),
             "application/json")
        }
        "framework://evidence_index" => {
            let current = repo_root.join("artifacts/current");
            let evidence_path = current.join("EVIDENCE_INDEX.json");
            let content = if evidence_path.is_file() {
                fs::read_to_string(&evidence_path).unwrap_or_else(|e| format!("Read error: {e}"))
            } else {
                "{}".to_string()
            };
            (content, "application/json")
        }
        "framework://session_summary" => {
            let current = repo_root.join("artifacts/current");
            let summary_path = current.join("SESSION_SUMMARY.md");
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn unique_test_repo(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "router-rs-claude-desktop-mcp-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn initialize_returns_capabilities() {
        let response = handle_initialize(Some(json!(1)));
        let result = &response["result"];
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
        let caps = &result["capabilities"];
        assert!(caps.get("tools").is_some());
        assert!(caps.get("prompts").is_some());
        assert!(caps.get("resources").is_some());
    }

    #[test]
    fn tools_list_returns_all_expected_tools() {
        let response = handle_tools_list(Some(json!(1)));
        let tools = response["result"]["tools"].as_array().expect("tools array");
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"framework_digest"));
        assert!(names.contains(&"framework_snapshot"));
        assert!(names.contains(&"skill_route"));
        assert!(names.contains(&"record_evidence"));
        assert!(names.contains(&"session_checkpoint"));
        assert!(names.contains(&"closeout_gate"));
        assert!(names.contains(&"goal_state_read"));
    }

    #[test]
    fn prompts_list_returns_all_expected_prompts() {
        let response = handle_prompts_list(Some(json!(1)));
        let prompts = response["result"]["prompts"].as_array().expect("prompts array");
        let names: Vec<&str> = prompts
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"continuity_digest"));
        assert!(names.contains(&"framework_routing"));
        assert!(names.contains(&"review_gate"));
        assert!(names.contains(&"closeout_checklist"));
    }

    #[test]
    fn ping_returns_empty_result() {
        let response = handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
            &unique_test_repo("ping"),
        )
        .unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert!(response.get("error").is_none());
    }

    #[test]
    fn unknown_method_returns_error() {
        let response = handle_mcp_request(
            r#"{"jsonrpc":"2.0","id":2,"method":"nonexistent"}"#,
            &unique_test_repo("unknown-method"),
        )
        .unwrap();
        assert_eq!(response["error"]["code"], -32601);
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("nonexistent"));
    }
}
