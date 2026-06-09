use router_rs::cli::route_task_with_manifest_fallback;
use router_rs::framework_error::{FrameworkError, FrameworkResult, ResultExt, RouteExitExt};
use router_rs::framework_runtime::{
    build_automatic_continuity_checkpoint_payload_with_task_id,
    build_framework_runtime_snapshot_envelope,
};
use router_rs::route::{filter_records_for_host, load_records_cached_for_stdio};
use router_rs::skill_repo::skill_routing_runtime_json;
use router_rs::session_call_tracker::record_tool_call_and_check_anomalies;
use router_rs::hook_common::is_review_prompt;
use router_rs::review_gate_engine::{claude_independent_reviewer_evidence, fork_context_from_values};
use router_rs::task_state::resolve_task_view;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use super::cache::{
    get_rate_limiter, get_snapshot_cache, get_task_view_cache,
    snapshot_cache_ttl_secs, SnapshotCache,
};
use super::host::{mcp_closeout_hard_block_disabled, mcp_host_hard_block_label, mcp_host_supports_hard_closeout};
use super::host::task_artifact_dir;
const WEB_FETCH_MAX_BYTES_DEFAULT: usize = 50_000;
const WEB_FETCH_TIMEOUT_SECS: u64 = 30;
const WEB_FETCH_CACHE_TTL_SECS: u64 = 120;
const WEB_FETCH_CACHE_MAX: usize = 200;

pub fn handle_tools_list(id: Option<Value>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": [
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
                    "name": "closeout",
                    "description": "Closeout gate (action=gate, default) or write closeout record (action=record_write).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["gate", "record_write"],
                                "description": "gate=自检清单；record_write=写入 closeout record",
                            },
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                    },
                },
                {
                    "name": "goal_state",
                    "description": "读取或管理 GOAL_STATE（action=read 或 operation=start|checkpoint|…）。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["read"],
                                "description": "read 时读取 GOAL_STATE；管理操作使用 operation",
                            },
                            "operation": {
                                "type": "string",
                                "enum": ["start", "checkpoint", "pause", "resume", "complete", "clear", "block"],
                                "description": "管理操作类型",
                            },
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                            "goal": {
                                "type": "string",
                                "description": "任务目标描述（operation=start 时必需）",
                            },
                            "non_goals": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "非目标列表（drive_until_done=true 时必需）",
                            },
                            "done_when": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "完成条件列表，至少 2 项（drive_until_done=true 时必需）",
                            },
                            "validation_commands": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "验证命令列表（drive_until_done=true 时必需）",
                            },
                            "drive_until_done": {
                                "type": "boolean",
                                "description": "是否严格驱动到完成，默认 false；设为 true 时 non_goals/done_when/validation_commands 变为必需",
                                "default": false,
                            },
                            "lifecycle_profile": {
                                "type": "string",
                                "description": "生命周期配置，如 my-light",
                            },
                            "note": {
                                "type": "string",
                                "description": "备注（operation=checkpoint 时使用）",
                            },
                            "blocker": {
                                "type": "string",
                                "description": "阻塞原因（operation=block 时使用）",
                            },
                        },
                    },
                },
                {
                    "name": "rfv_loop",
                    "description": "RFV 循环：status（默认）或 operation=start|append_round。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "enum": ["status"],
                                "description": "status 查看状态；变更使用 operation",
                            },
                            "operation": {
                                "type": "string",
                                "enum": ["start", "append_round"],
                                "description": "管理操作类型",
                            },
                            "task_id": {
                                "type": "string",
                                "description": "task id，默认当前 active task",
                            },
                        },
                    },
                },
                {
                    "name": "web_fetch",
                    "description": "只读 HTTP GET 抓取外部 URL（绕过 Bash 沙箱；Desktop MCP 进程内执行）。返回 status、body 摘要。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "url": {
                                "type": "string",
                                "description": "http(s) URL",
                            },
                            "max_bytes": {
                                "type": "integer",
                                "description": "响应体最大字节数（默认 50000）",
                            },
                        },
                        "required": ["url"],
                    },
                },
                {
                    "name": "web_search",
                    "description": "Search the web. Aggregates multiple engines (SearXNG, StackOverflow, GitHub, HN, Wikipedia, arXiv, Brave). Returns title/url/snippet per result.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query" },
                            "topic": { "type": "string", "enum": ["general", "tech", "news", "knowledge", "academic"], "description": "Optional topic override. Auto-detected if omitted." },
                            "max_results": { "type": "integer", "description": "Max results (default 8, range 1-15)" },
                            "no_cache": { "type": "boolean", "description": "Skip cache" }
                        },
                        "required": ["query"]
                    }
                },
            ],
        },
    })
}

pub fn handle_tools_call(id: Option<Value>, request: &Value, repo_root: &Path, host_id: &str) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let default_args = json!({});
    let arguments = params.get("arguments").unwrap_or(&default_args);

    // Check rate limit before processing
    {
        let limiter = get_rate_limiter();
        if let Some(mut guard) = poison_safe_lock!(limiter) {
            if let Err(e) = guard.check_and_record(tool_name) {
                return json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Rate limit: {}. Consider batching operations.", e) }],
                        "isError": true,
                    },
                });
            }
        }
    }

    // HX-5: MCP pre-guard (mcp-tool-safety + protected-path); panic → block + log.
    let pre_guard = router_rs::mcp_pre_guard::evaluate_mcp_pre_guard_safe(tool_name, arguments, repo_root);
    if pre_guard.blocked {
        let reason = pre_guard
            .reason
            .unwrap_or_else(|| "MCP pre-guard blocked this tool call.".to_string());
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Error: {reason}") }],
                "isError": true,
            },
        });
    }

    // Track every tool call for anomaly detection.
    // Use the combined function to avoid a second flock + disk read for check_anomalies.
    let skill_route_first_turn = if tool_name == "skill_route" {
        router_rs::session_call_tracker::skill_route_first_turn(repo_root)
    } else {
        false
    };
    let tracker_anomaly_warnings: Vec<String> = match record_tool_call_and_check_anomalies(repo_root, tool_name) {
        Ok(((), warnings)) => warnings,
        Err(e) => {
            eprintln!("[router-rs warning] record_tool_call failed: {e}");
            Vec::new()
        }
    };

    // MCP advisory closeout (Antigravity App; all modes advisory)
    if mcp_host_supports_hard_closeout(host_id) {
        let host_name = mcp_host_hard_block_label(host_id);

        if tool_name == "goal_state_manage" {
            if let Some("complete") = arguments.get("operation").and_then(Value::as_str) {
                match evaluate_mcp_closeout_gate(arguments, repo_root, host_id, None) {
                    Ok(_verdict) => {
                        // Advisory-only: hard_block is always false in MCP transport.
                        // Closeout gate verdict is reported in the response text; no hard block here.
                    }
                    Err(e) => {
                        return json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "content": [{ "type": "text", "text": format!("Error during pre-closeout check: {e}") }],
                                "isError": true,
                            },
                        });
                    }
                }
            }
        }
    }

    let mut result = match tool_name {
        "framework_snapshot" => tool_framework_snapshot(repo_root),
        "skill_route" => tool_skill_route(arguments, repo_root, host_id, skill_route_first_turn),
        "record_evidence" => tool_record_evidence(arguments, repo_root),
        "session_checkpoint" => tool_session_checkpoint(arguments, repo_root),
        "closeout" | "closeout_gate" | "closeout_record_write" => {
            tool_closeout(arguments, repo_root, host_id, tool_name)
        }
        "rfv_loop" | "rfv_loop_status" | "rfv_loop_manage" => {
            tool_rfv_loop(arguments, repo_root, tool_name)
        }
        "goal_state" | "goal_state_read" | "goal_state_manage" => {
            tool_goal_state(arguments, repo_root, tool_name)
        }
        "web_fetch" => tool_web_fetch(arguments),
            "web_search" => router_rs::hosts::mcp_common::tools_web::tool_web_search(arguments),
        _ => Err(FrameworkError::validation(format!("Unknown tool: {tool_name}"))),
    };

    let closeout_gate_invocation = matches!(
        tool_name,
        "closeout" | "closeout_gate"
    ) && !matches!(
        arguments.get("action").and_then(Value::as_str),
        Some("record_write")
    ) && tool_name != "closeout_record_write";
    // MCP advisory closeout: hard_block is always false in MCP transport.
    // The closeout gate verdict is reported in response text; no hard block applied.
    if closeout_gate_invocation {
        // Suppress unused variable warning
        let _ = mcp_host_supports_hard_closeout(host_id);
    }

    match result {
        Ok(content) => {
            // Anomaly warnings already captured during record_tool_call (single flock cycle).
            let warnings = tracker_anomaly_warnings;

            let final_content = if warnings.is_empty() {
                content
            } else {
                let warning_text = warnings.join("; ");
                format!("{}\n\n[Session Warning] {}", content, warning_text)
            };

            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": final_content }],
                },
            })
        }
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

pub fn tool_framework_snapshot(repo_root: &Path) -> FrameworkResult<String> {
    let ttl_secs = snapshot_cache_ttl_secs();
    // Try to read from cache (configurable TTL, default 30 seconds)
    {
        let cache = get_snapshot_cache();
        if let Some(guard) = poison_safe_lock!(cache) {
            if let Some(ref cached) = *guard {
                if cached.is_valid() {
                    return Ok(cached.content.clone());
                }
            }
        }
    }

    // Cache miss: recompute
    let envelope = build_framework_runtime_snapshot_envelope(repo_root, None, None)?;
    let content = serde_json::to_string_pretty(&envelope)?;

    // Update cache with configurable TTL
    {
        let cache = get_snapshot_cache();
        if let Some(mut guard) = poison_safe_lock!(cache) {
            *guard = Some(SnapshotCache {
                content: content.clone(),
                expires_at: Instant::now() + Duration::from_secs(ttl_secs),
            });
        }
    }

    Ok(content)
}

/// Invalidate evidence-dependent caches (snapshot / task view).
fn invalidate_evidence_caches() {
    // Clear snapshot cache
    if let Some(mut guard) = poison_safe_lock!(get_snapshot_cache()) {
        *guard = None;
    }
    // Clear task view cache
    if let Some(mut guard) = poison_safe_lock!(get_task_view_cache()) {
        *guard = None;
    }
}

pub fn tool_skill_route(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
    first_turn: bool,
) -> FrameworkResult<String> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: query"))?;

    let runtime_path = skill_routing_runtime_json(repo_root);
    let records = load_records_cached_for_stdio(Some(&runtime_path), None).map_route_exit()?;
    let records = filter_records_for_host(records.as_ref(), Some(host_id)).map_route_exit()?;
    let decision = route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        None,
        Some(host_id),
        query,
        "session",
        true, // allow_overlay: true
        first_turn,
    )
    .map_route_exit()?;
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
        "match_reason": decision.reasons.first().cloned().unwrap_or_default(),
    })
    .to_string())
}

pub fn build_evidence_entry(arguments: &Value) -> Result<Map<String, Value>, FrameworkError> {
    let tool_name = arguments
        .get("tool_name")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: tool_name"))?;
    let command = arguments
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: command"))?;
    let exit_code = arguments.get("exit_code").and_then(Value::as_i64);
    let output = arguments.get("output").and_then(Value::as_str);

    let mut entry = Map::new();
    entry.insert("kind".to_string(), json!("mcp_record_evidence"));
    entry.insert("source".to_string(), json!("mcp_record_evidence"));
    entry.insert(
        "trust_tier".to_string(),
        json!(framework_core::state_manager::EVIDENCE_TRUST_SELF_ATTESTED),
    );
    entry.insert("tool_name".to_string(), json!(tool_name));
    entry.insert("command_preview".to_string(), json!(command));
    entry.insert(
        "recorded_at".to_string(),
        json!(router_rs::framework_runtime::current_local_timestamp()),
    );
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), json!(ec));
        entry.insert("success".to_string(), json!(ec == 0));
    }
    if let Some(text) = output {
        let max_chars = evidence_output_max_chars();
        let trimmed: String = text.chars().take(max_chars).collect();
        entry.insert("output".to_string(), json!(trimmed));
    }
    Ok(entry)
}

pub fn tool_record_evidence(arguments: &Value, repo_root: &Path) -> FrameworkResult<String> {
    let entry = build_evidence_entry(arguments)?;
    let tool_name = entry
        .get("tool_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    let tool_name_display = tool_name.as_deref().unwrap_or("");
    let command = entry
        .get("command_preview")
        .and_then(Value::as_str)
        .map(str::to_string);
    let command_display = command.as_deref().unwrap_or("");
    let exit_code = arguments.get("exit_code").and_then(Value::as_i64);
    let output = arguments.get("output").and_then(Value::as_str);

    router_rs::framework_runtime::append_evidence_index_merged_row(repo_root, None, entry)?;

    // H2 FIX: Invalidate caches after evidence is written to ensure fresh data on next read
    invalidate_evidence_caches();

    let exit_display = exit_code
        .map(|ec| ec.to_string())
        .unwrap_or_else(|| "null".to_string());
    let honor_note = " (honor-system: not bound to host tool execution — verify independently)";
    if let Some(text) = output {
        let max_chars = evidence_output_max_chars();
        let trimmed = text.chars().take(max_chars).collect::<String>();
        Ok(format!(
            "Evidence recorded{honor_note}: {tool_name_display} '{command_display}' -> exit={exit_display}\n{trimmed}"
        ))
    } else {
        Ok(format!(
            "Evidence recorded{honor_note}: {tool_name_display} '{command_display}' -> exit={exit_display}"
        ))
    }
}

/// 获取 evidence output 的最大字符数配置。
/// 默认 2000 字符，可通过 `ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS` 环境变量覆盖。
fn evidence_output_max_chars() -> usize {
    static CACHED: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(2000)
    })
}

pub fn tool_session_checkpoint(arguments: &Value, repo_root: &Path) -> FrameworkResult<String> {
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: summary"))?;
    let next_actions: Vec<String> = arguments
        .get("next_actions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let task_id = arguments.get("task_id").and_then(Value::as_str);

    let payload = build_automatic_continuity_checkpoint_payload_with_task_id(
        repo_root,
        summary,
        &next_actions.join(", "),
        task_id,
        true,
        false,
    );
    router_rs::framework_runtime::write_framework_session_artifacts(payload)
        .map_err(|e| FrameworkError::other(format!("Checkpoint write failed: {e}")))?;

    // H2 FIX: Invalidate caches after checkpoint is written to ensure fresh data on next read
    invalidate_evidence_caches();

    Ok(format!(
        "Checkpoint written: summary={}, next_actions_count={}",
        summary.chars().count(),
        next_actions.len()
    ))
}

fn goal_suggests_review_work(goal_state: &framework_core::types::GoalState) -> bool {
    if is_review_prompt(&goal_state.goal) {
        return true;
    }
    goal_state
        .done_when
        .iter()
        .filter_map(Value::as_str)
        .any(is_review_prompt)
}

fn desktop_review_evidence_attested(arguments: &Value, repo_root: &Path, task_id: &str) -> bool {
    // 自动扫描 artifacts/current/<task_id>/review-lanes 目录下的 Markdown 证据工件
    let review_lanes_dir = task_artifact_dir(
        repo_root,
        if task_id.is_empty() {
            None
        } else {
            Some(task_id)
        },
    )
    .join("review-lanes");

    if review_lanes_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&review_lanes_dir) {
            let mut valid_findings_found = false;
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if !content.trim().is_empty() {
                            valid_findings_found = true;
                            break;
                        }
                    }
                }
            }
            if valid_findings_found {
                return true;
            }
        }
    }

    let lane = arguments
        .get("reviewer_lane")
        .or_else(|| arguments.get("subagent_type"))
        .or_else(|| arguments.get("agent_type"))
        .and_then(Value::as_str);
    let Some(lane) = lane else {
        return false;
    };
    let review_lane =
        router_rs::runtime_registry::is_claude_reviewer_lane_from_registry(lane, Some(repo_root));
    let fork = fork_context_from_values(arguments, None);
    claude_independent_reviewer_evidence(review_lane, fork)
}

#[derive(Debug, Clone)]
pub struct McpCloseoutGateVerdict {
    pub all_clear: bool,
    pub checkpoint_only: bool,
    pub hard_block: bool,
    pub formatted: String,
    /// The resolved task view used during evaluation (avoids redundant resolve_task_view calls).
    pub task_view: router_rs::task_state::ResolvedTaskView,
}





pub fn evaluate_mcp_closeout_gate(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
    cached_task_view: Option<&router_rs::task_state::ResolvedTaskView>,
) -> Result<McpCloseoutGateVerdict, FrameworkError> {
    let task_id_override = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let task_view = match cached_task_view {
        Some(tv) => tv.clone(),
        None => resolve_task_view(repo_root, task_id_override),
    };
    let mut findings: Vec<String> = Vec::new();
    let host_name = mcp_host_hard_block_label(host_id);

    findings.push(format!(
        "review_gate: {host_name} has no hook REVIEW_GATE — reviewer evidence is honor-system / self-attested (prompts/review_gate)"
    ));

    let goal_present = task_view.goal_state.is_some();
    if !goal_present {
        findings.push("goal_state: no GOAL_STATE.json".to_string());
    } else {
        findings.push("goal_state: present".to_string());
    }

    let evidence_success = task_view
        .evidence
        .as_ref()
        .map(|e| e.has_successful_verification)
        .unwrap_or(false);
    let task_id = task_view.task_id.as_deref().unwrap_or("");

    if !evidence_success {
        findings.push("evidence: no successful EVIDENCE_INDEX records".to_string());
    } else {
        findings.push("evidence: successful records present".to_string());
        if !task_id.is_empty()
            && router_rs::goal_state::task_evidence_success_only_self_attested(repo_root, task_id)
        {
            findings.push(
                "WARN: evidence: only self-attested MCP record_evidence rows — verify independently"
                    .to_string(),
            );
        }
    }
    let summary_path = task_artifact_dir(repo_root, if task_id.is_empty() { None } else { Some(task_id) })
        .join("SESSION_SUMMARY.md");
    let has_summary = summary_path.is_file();
    if !has_summary {
        findings.push(format!(
            "checkpoint: missing SESSION_SUMMARY at {}",
            summary_path.display()
        ));
    } else {
        findings.push("checkpoint: SESSION_SUMMARY.md on disk".to_string());
    }

    let review_goal = task_view
        .goal_state
        .as_ref()
        .is_some_and(goal_suggests_review_work);
    let has_review_evidence = desktop_review_evidence_attested(arguments, repo_root, task_id);

    if review_goal && !has_review_evidence {
        findings.push(format!(
            "WARN: review_gate: GOAL suggests review work but no hook-level reviewer evidence on {host_name} — spawn claude_reviewer_lanes with fork_context=false and write review-lanes/*.md, or pass reviewer_lane + fork_context=false in closeout_gate args"
        ));
    } else if review_goal {
        findings.push(
            "review_gate: GOAL suggests review; reviewer evidence attested in closeout_gate args or review-lanes"
                .to_string(),
        );
    }

    let mut all_clear = goal_present && evidence_success && has_summary;
    if review_goal && !has_review_evidence {
        all_clear = false;
    }

    let checkpoint_only =
        !all_clear && goal_present && evidence_success && (!review_goal || has_review_evidence);

    let verdict_label = if all_clear {
        "PASS: all closeout gates satisfied"
    } else if checkpoint_only {
        "ADVISORY: checkpoint missing — call session_checkpoint before complete"
    } else {
        "ADVISORY: closeout gates not satisfied"
    };

    let formatted = format!(
        "[Closeout Gate] {verdict_label}\n\n{}",
        findings.join("\n")
    );

    Ok(McpCloseoutGateVerdict {
        all_clear,
        checkpoint_only,
        hard_block: false,
        formatted,
        task_view,
    })
}

pub fn tool_closeout_gate(arguments: &Value, repo_root: &Path, host_id: &str) -> FrameworkResult<String> {
    Ok(evaluate_mcp_closeout_gate(arguments, repo_root, host_id, None)?.formatted)
}

pub fn tool_closeout_record_write(arguments: &Value, repo_root: &Path, host_id: &str) -> FrameworkResult<String> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: task_id"))?;
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: summary"))?;
    let verification_status = arguments
        .get("verification_status")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: verification_status"))?;

    let mut record = Map::new();
    record.insert(
        "schema_version".to_string(),
        json!(router_rs::closeout_enforcement::CLOSEOUT_RECORD_SCHEMA_VERSION),
    );
    record.insert("task_id".to_string(), json!(task_id));
    record.insert(
        "ended_at".to_string(),
        json!(router_rs::framework_runtime::current_local_timestamp()),
    );
    record.insert("summary".to_string(), json!(summary));
    record.insert(
        "verification_status".to_string(),
        json!(verification_status),
    );

    if let Some(files) = arguments.get("changed_files").and_then(Value::as_array) {
        record.insert("changed_files".to_string(), json!(files));
    }
    if let Some(cmds) = arguments.get("commands_run").and_then(Value::as_array) {
        record.insert("commands_run".to_string(), json!(cmds));
    }
    if let Some(blockers) = arguments.get("blockers").and_then(Value::as_array) {
        if !blockers.is_empty() {
            record.insert("blockers".to_string(), json!(blockers));
        }
    }
    if let Some(risks) = arguments.get("risks").and_then(Value::as_array) {
        if !risks.is_empty() {
            record.insert("risks".to_string(), json!(risks));
        }
    }
    if let Some(notes) = arguments.get("notes").and_then(Value::as_str) {
        if !notes.is_empty() {
            record.insert("notes".to_string(), json!(notes));
        }
    }

    // Ensure parent directory exists
    let record_path = router_rs::framework_runtime::closeout_record_path_for_task(repo_root, task_id)
        .map_err(|e| FrameworkError::validation(format!("invalid task_id: {e}")))?;
    if let Some(parent) = record_path.parent() {
        fs::create_dir_all(parent).map_err(|e| FrameworkError::other(format!("create closeout directory failed: {e}")))?;
    }

    // Write the record
    let content = serde_json::to_string_pretty(&record)?;
    fs::write(&record_path, &content).map_err(|e| FrameworkError::other(format!("write closeout record failed: {e}")))?;

    // Evaluate the record
    let eval_result = router_rs::framework_runtime::evaluate_closeout_record_file_for_task(
        repo_root,
        task_id,
        &record_path,
    );
    let eval = match eval_result {
        Ok(v) => v,
        Err(e) => json!({"error": e.to_string()}),
    };

    let closeout_allowed = eval
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let violations: Vec<String> = eval
        .get("violations")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|v| {
                    let rule = v.get("rule").and_then(Value::as_str).unwrap_or("unknown");
                    let detail = v
                        .get("detail")
                        .and_then(Value::as_str)
                        .unwrap_or("no detail");
                    format!("[{rule}] {detail}")
                })
                .collect()
        })
        .unwrap_or_default();

    let mut result = json!({
        "closeout_allowed": closeout_allowed,
        "record_path": record_path.to_string_lossy().to_string(),
        "violations": violations,
    });

    if let Ok(mcp_verdict) = evaluate_mcp_closeout_gate(
        &json!({ "task_id": task_id }),
        repo_root,
        host_id, None,
    ) {
        let task_view = &mcp_verdict.task_view;
        let lifecycle_profile = task_view
            .goal_state
            .as_ref()
            .and_then(|g| g.extra.get("lifecycle_profile").and_then(Value::as_str))
            .unwrap_or("");
        let hard_block_disabled = mcp_closeout_hard_block_disabled(repo_root, lifecycle_profile);
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "mcp_closeout_gate".to_string(),
                json!({
                    "all_clear": mcp_verdict.all_clear,
                    "checkpoint_only": mcp_verdict.checkpoint_only,
                    "hard_block": mcp_verdict.hard_block,
                }),
            );
        }
    }

    Ok(serde_json::to_string_pretty(&result)?)
}

const WEB_FETCH_MAX_REDIRECTS: usize = 5;

static WEB_FETCH_CLIENT: std::sync::OnceLock<reqwest::blocking::Client> = std::sync::OnceLock::new();
static WEB_FETCH_CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (String, std::time::Instant)>>> = std::sync::OnceLock::new();

fn get_web_client() -> &'static reqwest::blocking::Client {
    WEB_FETCH_CLIENT.get_or_init(|| {
        let mut builder = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(WEB_FETCH_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("router-rs-framework/0.7")
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(4);
        for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY"] {
            if let Ok(proxy_url) = std::env::var(key) {
                let trimmed = proxy_url.trim();
                if !trimmed.is_empty() {
                    if let Ok(proxy) = reqwest::Proxy::all(trimmed) {
                        builder = builder.proxy(proxy);
                        break;
                    }
                }
            }
        }
        builder.build().expect("web_fetch HTTP client init failed")
    })
}

fn check_fetch_cache(key: &str) -> Option<String> {
    let cache = WEB_FETCH_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((body, ts)) = guard.get(key) {
        if ts.elapsed().as_secs() < WEB_FETCH_CACHE_TTL_SECS {
            return Some(body.clone());
        }
    }
    None
}

fn store_fetch_cache(key: &str, value: &str) {
    let cache = WEB_FETCH_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if guard.len() >= WEB_FETCH_CACHE_MAX {
        if let Some(oldest_key) = guard.iter().min_by_key(|(_, (_, ts))| ts).map(|(k, _)| k.clone()) {
            guard.remove(&oldest_key);
        }
    }
    guard.insert(key.to_string(), (value.to_string(), std::time::Instant::now()));
}

pub fn tool_web_fetch(arguments: &Value) -> FrameworkResult<String> {
    let url = arguments
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| FrameworkError::validation("Missing required argument: url"))?;

    let (parsed_url, initial_addrs) = router_rs::web_fetch_guard::validate_and_resolve_web_fetch_url(url).map_framework_err()?;

    let max_bytes = arguments
        .get("max_bytes")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(WEB_FETCH_MAX_BYTES_DEFAULT)
        .clamp(1, WEB_FETCH_MAX_BYTES_DEFAULT * 2);

    let format = arguments.get("format").and_then(Value::as_str).unwrap_or("markdown");
    let no_cache = arguments.get("no_cache").and_then(Value::as_bool).unwrap_or(false);

    let cache_key = format!("{}:{}", format, url);
    if !no_cache {
        if let Some(cached) = check_fetch_cache(&cache_key) {
            return Ok(cached);
        }
    }

    let client = get_web_client();
    let mut current_url = url.to_string();
    let mut response = None;

    // Redirect loop with DNS pinning at each hop
    let mut active_client = std::borrow::Cow::Borrowed(client);
    for hop in 0..=WEB_FETCH_MAX_REDIRECTS {
        let resp = active_client
            .get(&current_url)
            .send()
            .map_err(|err| FrameworkError::other(format!("web_fetch request failed: {err}")))?;

        if resp.status().is_redirection() {
            if hop >= WEB_FETCH_MAX_REDIRECTS {
                return Err(FrameworkError::other(format!("web_fetch exceeded {WEB_FETCH_MAX_REDIRECTS} redirects")));
            }
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| FrameworkError::other(format!("web_fetch redirect missing Location header (status {})", resp.status())))?;
            let base = reqwest::Url::parse(&current_url)
                .map_err(|err| FrameworkError::other(format!("web_fetch redirect base URL invalid: {err}")))?;
            current_url = router_rs::web_fetch_guard::resolve_web_fetch_redirect(&base, location).map_framework_err()?;
            let (redirect_parsed, redirect_addrs) = router_rs::web_fetch_guard::validate_and_resolve_web_fetch_url(&current_url).map_framework_err()?;
            let rp_host = redirect_parsed.host_str()
                .ok_or_else(|| FrameworkError::other(format!("web_fetch redirect URL missing host: {current_url}")))?;
            let mut rb = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(WEB_FETCH_TIMEOUT_SECS))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("router-rs-framework/0.7");
            for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY"] {
                if let Ok(proxy_url) = std::env::var(key) {
                    let trimmed = proxy_url.trim();
                    if !trimmed.is_empty() {
                        if let Ok(proxy) = reqwest::Proxy::all(trimmed) {
                            rb = rb.proxy(proxy);
                            break;
                        }
                    }
                }
            }
            for addr in &redirect_addrs {
                rb = rb.resolve(rp_host, *addr);
            }
            active_client = std::borrow::Cow::Owned(rb.build()
                .map_err(|err| FrameworkError::other(format!("web_fetch client rebuild failed: {err}")))?);
            continue;
        }
        response = Some(resp);
        break;
    }

    let response = response.ok_or_else(|| FrameworkError::other("web_fetch: no response"))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let cache_control = response.headers().get(reqwest::header::CACHE_CONTROL).and_then(|v| v.to_str().ok()).unwrap_or("");
    let upstream_no_store = cache_control.contains("no-store") || cache_control.contains("no-cache");

    let body = response.bytes().map_err(|err| FrameworkError::other(format!("web_fetch read body failed: {err}")))?;
    let truncated = body.len() > max_bytes;
    let body_slice = if truncated { &body[..max_bytes] } else { &body };

    let is_html = content_type.contains("text/html") || content_type.contains("application/xhtml");
    let is_json = content_type.contains("application/json");

    let body_text = if format == "markdown" && is_html {
        let html_str = String::from_utf8_lossy(body_slice).into_owned();
        let extracted = router_rs::content_extract::extract_readable_content(&html_str);
        router_rs::html_to_markdown::html_to_markdown(&extracted, Some(&current_url))
    } else if format == "text" && is_html {
        let html_str = String::from_utf8_lossy(body_slice).into_owned();
        router_rs::html_to_markdown::html_to_markdown(&router_rs::content_extract::extract_readable_content(&html_str), Some(&current_url))
    } else if is_json {
        match serde_json::from_slice::<serde_json::Value>(body_slice) {
            Ok(val) => serde_json::to_string_pretty(&val).unwrap_or_else(|_| String::from_utf8_lossy(body_slice).into_owned()),
            Err(_) => String::from_utf8_lossy(body_slice).into_owned(),
        }
    } else {
        String::from_utf8_lossy(body_slice).into_owned()
    };

    let payload = json!({
        "url": current_url, "status": status, "content_type": content_type,
        "content_length": body.len(), "truncated": truncated, "format": format,
        "cached": false, "body": body_text,
    });
    let result = serde_json::to_string_pretty(&payload)?;

    if !no_cache && !upstream_no_store && !truncated {
        store_fetch_cache(&cache_key, &result);
    }
    Ok(result)
}

pub fn tool_goal_state_read(arguments: &Value, repo_root: &Path) -> FrameworkResult<String> {
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let state = router_rs::goal_state::read_goal_state(repo_root, task_id);
    Ok(serde_json::to_string_pretty(&state)?)
}

pub fn tool_goal_state(
    arguments: &Value,
    repo_root: &Path,
    tool_name: &str,
) -> FrameworkResult<String> {
    let action = arguments
        .get("action")
        .or_else(|| arguments.get("operation"))
        .and_then(Value::as_str);
    let read = tool_name == "goal_state_read"
        || action == Some("read")
        || (tool_name == "goal_state" && action.is_none() && arguments.get("operation").is_none());
    if read {
        tool_goal_state_read(arguments, repo_root)
    } else {
        tool_goal_state_manage(arguments, repo_root)
    }
}

pub fn tool_rfv_loop(
    arguments: &Value,
    repo_root: &Path,
    tool_name: &str,
) -> FrameworkResult<String> {
    let action = arguments
        .get("action")
        .or_else(|| arguments.get("operation"))
        .and_then(Value::as_str);
    if tool_name == "rfv_loop_status"
        || action == Some("status")
        || (tool_name == "rfv_loop"
            && action.is_none()
            && arguments.get("operation").is_none())
    {
        tool_rfv_loop_status(arguments, repo_root)
    } else {
        tool_rfv_loop_manage(arguments, repo_root)
    }
}

pub fn tool_closeout(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
    tool_name: &str,
) -> FrameworkResult<String> {
    let action = arguments.get("action").and_then(Value::as_str);
    if tool_name == "closeout_record_write" || action == Some("record_write") {
        tool_closeout_record_write(arguments, repo_root, host_id)
    } else {
        tool_closeout_gate(arguments, repo_root, host_id)
    }
}
pub fn tool_rfv_loop_status(arguments: &Value, repo_root: &Path) -> FrameworkResult<String> {
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let state = router_rs::rfv_loop::read_rfv_loop_state(repo_root, task_id)?;
    Ok(serde_json::to_string_pretty(&state)?)
}

fn parse_rfv_round_argument(value: Option<&Value>) -> Result<u64, FrameworkError> {
    let Some(v) = value else {
        return Err(FrameworkError::validation("append_round requires 'round' argument (integer)"));
    };
    if let Some(n) = v.as_u64() {
        return Ok(n);
    }
    if let Some(n) = v.as_i64() {
        if n >= 0 {
            return Ok(n as u64);
        }
    }
    Err(FrameworkError::validation("append_round requires 'round' argument (integer)"))
}

pub fn tool_rfv_loop_manage(arguments: &Value, repo_root: &Path) -> FrameworkResult<String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: operation (string)"))?;
    let task_id = arguments.get("task_id").and_then(Value::as_str);

    // repo_root is a &Path, convert to string for the payload
    let repo_root_str = repo_root.to_string_lossy().to_string();

    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
    });
    if let Some(tid) = task_id {
        payload["task_id"] = json!(tid);
    }

    // Per-operation required fields
    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("start requires 'goal' argument (string)"))?;
            payload["goal"] = json!(goal);
            if let Some(mr) = arguments.get("max_rounds").and_then(Value::as_u64) {
                payload["max_rounds"] = json!(mr);
            }
            if let Some(er) = arguments
                .get("allow_external_research")
                .and_then(Value::as_bool)
            {
                payload["allow_external_research"] = json!(er);
            }
        }
        "append_round" => {
            let round = parse_rfv_round_argument(arguments.get("round"))?;
            payload["round"] = json!(round);

            // Validate required string arguments with specific error messages
            let review_summary = arguments
                .get("review_summary")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'review_summary' argument (string)"))?;
            payload["review_summary"] = json!(review_summary);

            let fix_summary = arguments
                .get("fix_summary")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'fix_summary' argument (string)"))?;
            payload["fix_summary"] = json!(fix_summary);

            let verify_result = arguments
                .get("verify_result")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'verify_result' argument (string)"))?;
            payload["verify_result"] = json!(verify_result);

            let supervisor_decision = arguments
                .get("supervisor_decision")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'supervisor_decision' argument (string)"))?;
            payload["supervisor_decision"] = json!(supervisor_decision);

            let reason = arguments
                .get("reason")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("append_round requires 'reason' argument (string)"))?;
            payload["reason"] = json!(reason);
        }
        _ => {
            return Err(FrameworkError::validation(format!(
                "Unknown RFV loop operation: {operation}. Valid operations: start, append_round"
            )))
        }
    }

    let result = router_rs::rfv_loop::framework_rfv_loop(payload).map_framework_err()?;
    Ok(serde_json::to_string_pretty(&result)?)
}

pub fn tool_goal_state_manage(arguments: &Value, repo_root: &Path) -> FrameworkResult<String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::validation("Missing required argument: operation"))?;
    let task_id = arguments.get("task_id").and_then(Value::as_str);

    let repo_root_str = repo_root.to_string_lossy().to_string();

    let mut payload = json!({
        "repo_root": repo_root_str,
        "operation": operation,
    });
    if let Some(tid) = task_id {
        payload["task_id"] = json!(tid);
    }

    match operation {
        "start" => {
            let goal = arguments
                .get("goal")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("start requires 'goal' argument (string)"))?;
            payload["goal"] = json!(goal);
            if let Some(ng) = arguments.get("non_goals").and_then(Value::as_array) {
                payload["non_goals"] = json!(ng);
            }
            if let Some(dw) = arguments.get("done_when").and_then(Value::as_array) {
                payload["done_when"] = json!(dw);
            }
            if let Some(vc) = arguments
                .get("validation_commands")
                .and_then(Value::as_array)
            {
                payload["validation_commands"] = json!(vc);
            }
            if let Some(dud) = arguments.get("drive_until_done").and_then(Value::as_bool) {
                payload["drive_until_done"] = json!(dud);
            }
        }
        "checkpoint" => {
            let note = arguments
                .get("note")
                .and_then(Value::as_str)
                .ok_or_else(|| FrameworkError::validation("checkpoint requires 'note' argument (string)"))?;
            payload["note"] = json!(note);
        }
        "block" => {
            let blocker = arguments
                .get("blocker")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| FrameworkError::validation("block requires 'blocker' argument (string)"))?;
            payload["blocker"] = json!(blocker);
        }
        "append_round" => {
            // append_round is handled in rfv_loop, not here
            return Err(
                FrameworkError::validation("append_round is not a valid goal_state_manage operation. \
                 Use rfv_loop_manage with operation=append_round instead."),
            );
        }
        "pause" | "resume" | "complete" | "clear" => {
            // No additional required args
        }
        _ => return Err(FrameworkError::validation(format!("Unknown goal operation: {operation}. Valid operations: start, checkpoint, pause, resume, complete, clear, block"))),
    }

    let result = router_rs::goal_state::framework_goal_drive(payload).map_framework_err()?;

    // Invalidate snapshot/task_view caches after goal state write (H3 FIX)
    let op = arguments.get("operation").and_then(|v| v.as_str()).unwrap_or("");
    if op != "status" {
        invalidate_evidence_caches();
    }
    Ok(serde_json::to_string_pretty(&result)?)
}
