//! MCP tool implementations (tool_* functions).
//!
//! Extracted from mcp_stdio_harness.rs to keep file size ≤2000 lines.

use super::*;
use framework_kernel::skill_repo::skill_routing_runtime_json;
use routing_engine::route::{
    build_search_results_payload, filter_record_indices_for_host, load_records_cached_for_stdio,
    search_skills_subset,
};
use serde_json::{Map, Value, json};
use std::path::Path;

pub(super) fn handle_tools_call(
    id: Option<Value>,
    request: &Value,
    repo_root: &Path,
    host_id: &str,
    connection_session_id: &str,
) -> Value {
    let default_params = json!({});
    let params = request.get("params").unwrap_or(&default_params);
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let default_args = json!({});
    let arguments = params.get("arguments").unwrap_or(&default_args);

    // Check rate limit before processing
    {
        let limiter = get_rate_limiter();
        if let Some(mut guard) = poison_safe_lock!(limiter)
            && let Err(e) = guard.check_and_record(tool_name) {
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

    // HX-5: MCP pre-guard (mcp-tool-safety); panic → allow + log.
    let pre_guard = crate::hooks::evaluate_mcp_pre_guard_safe(tool_name, arguments, repo_root);
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
    if let Err(e) = record_tool_call(repo_root, tool_name, None) {
        eprintln!("[router-rs warning] record_tool_call failed: {e}");
    }

    let result = match tool_name {
        "framework_snapshot" => tool_framework_snapshot(arguments, repo_root),
        "skill_route" => tool_skill_route(arguments, repo_root, host_id),
        "skill_search" => tool_skill_search(arguments, repo_root, host_id),
        "skill_read" => tool_skill_read(arguments, repo_root),
        "skill_route_status" => tool_skill_route_status(repo_root),
        "record_evidence" => tool_record_evidence(arguments, repo_root),
        "session_checkpoint" => tool_session_checkpoint(arguments, repo_root),
        "closeout_gate" => tool_closeout_gate(arguments, repo_root, host_id),
        "rfv_loop_status" => tool_rfv_loop_status(arguments, repo_root),
        "rfv_loop_manage" => tool_rfv_loop_manage(arguments, repo_root, connection_session_id),
        "goal_state_manage" => tool_goal_state_manage(arguments, repo_root, connection_session_id),
        "goal_state_read" => tool_goal_state_read(arguments, repo_root),
        "closeout_record_write" => tool_closeout_record_write(arguments, repo_root, host_id),
        "routing_evolution" => tool_routing_evolution(arguments, repo_root),
        "web_fetch" => tool_web_fetch(arguments),
        _ => Err(format!("Unknown tool: {tool_name}")),
    };

    match result {
        Ok(content) => {
            // Check for anomalies and append warnings if detected
            let warnings = check_anomalies(repo_root).unwrap_or_default();

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

pub(super) fn tool_framework_snapshot(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, String> {
    let detail_level = arguments
        .get("detail_level")
        .and_then(Value::as_str)
        .unwrap_or("summary");
    if detail_level != "summary" && detail_level != "full" {
        return Err(format!(
            "Invalid detail_level: {detail_level}. Must be 'summary' or 'full'."
        ));
    }
    let is_full = detail_level == "full";
    let ttl_secs = snapshot_cache_ttl_secs();
    // Try to read from cache (configurable TTL, default 30 seconds)
    // Only use cache for summary mode; full mode always recomputes
    if !is_full {
        let cache = get_snapshot_cache();
        if let Some(guard) = poison_safe_read_lock!(cache)
            && let Some(ref cached) = *guard
                && cached.is_valid() {
                    return Ok(cached.content.clone());
                }
    }

    // Cache miss: recompute
    let envelope = crate::hooks::build_framework_runtime_snapshot_envelope_with_level(
        repo_root,
        None,
        None,
        detail_level,
    )?;
    let content = serde_json::to_string_pretty(&envelope).map_err(|e| e.to_string())?;

    // Update cache with configurable TTL (summary mode only)
    if !is_full {
        let cache = get_snapshot_cache();
        if let Some(mut guard) = poison_safe_write_lock!(cache) {
            *guard = Some(SnapshotCache {
                content: content.clone(),
                expires_at: Instant::now() + Duration::from_secs(ttl_secs),
            });
        }
    }

    Ok(content)
}

/// Invalidate evidence-dependent caches (snapshot / task view).
pub(super) fn invalidate_evidence_caches() {
    // Clear snapshot cache
    if let Some(mut guard) = poison_safe_write_lock!(get_snapshot_cache()) {
        *guard = None;
    }
    // Clear task view cache
    if let Some(mut guard) = poison_safe_write_lock!(get_task_view_cache()) {
        guard.clear();
    }
}

pub(super) fn tool_skill_route(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, String> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: query")?;

    // Dynamically determine first_turn: true only if no routing tools have been called yet.
    // This prevents stale routing behavior on subsequent calls within the same session.
    let first_turn = read_tracker_state(repo_root)
        .map(|state| {
            let per_tool = state.get("per_tool").and_then(|v| v.as_object());
            let has_routing = per_tool
                .map(|m| m.contains_key("skill_route"))
                .unwrap_or(false);
            !has_routing
        })
        .unwrap_or(true); // Default to first_turn=true on error

    let runtime_path = skill_routing_runtime_json(repo_root);
    let manifest_path = skill_manifest_path(repo_root);
    let records = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))?;
    let records = filter_records_for_host(records.as_ref(), Some(host_id))?;
    let decision = crate::hooks::route_task_with_manifest_fallback(
        &records,
        Some(&runtime_path),
        Some(&manifest_path),
        Some(host_id),
        query,
        "session",
        true, // allow_overlay: true
        first_turn,
    )?;
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

pub(super) fn tool_skill_search(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, String> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: query")?;
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .clamp(1, 50) as usize;
    // Allow caller to override host filter; fall back to connection-level host_id.
    let effective_host = arguments
        .get("hostId")
        .and_then(Value::as_str)
        .unwrap_or(host_id);

    let runtime_path = skill_routing_runtime_json(repo_root);
    let manifest_path = skill_manifest_path(repo_root);
    if !runtime_path.is_file() && !manifest_path.is_file() {
        return Err(format!(
            "Missing repository skill runtime ({}) and manifest ({})",
            runtime_path.display(),
            manifest_path.display()
        ));
    }
    let records = load_records_cached_for_stdio(Some(&runtime_path), Some(&manifest_path))?;
    let host_indices = filter_record_indices_for_host(records.as_ref(), Some(effective_host))?;
    let rows = search_skills_subset(records.as_ref(), Some(&host_indices), query, limit);
    let results = build_search_results_payload(query, rows);
    serde_json::to_string(&results).map_err(|e| e.to_string())
}

pub(super) fn tool_skill_read(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let slug = arguments
        .get("skill")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: skill")?;
    let max_chars = arguments
        .get("maxChars")
        .and_then(Value::as_u64)
        .unwrap_or(20_000)
        .clamp(1, 50_000) as usize;

    let path = skill_body_path(repo_root, slug)?;
    let content = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let truncated = content.chars().count() > max_chars;
    let truncated_content: String = content.chars().take(max_chars).collect();

    Ok(json!({
        "schema_version": "cowork-skill-read-v1",
        "authority": "router-rs-framework",
        "skill": slug,
        "path": path.to_string_lossy(),
        "content": truncated_content,
        "truncated": truncated,
    })
    .to_string())
}

fn skill_manifest_path(repo_root: &Path) -> PathBuf {
    repo_root.join("skills/SKILL_MANIFEST.json")
}

fn skill_runtime_available(repo_root: &Path) -> bool {
    skill_routing_runtime_json(repo_root).is_file() && repo_root.join("skills").is_dir()
}

fn skill_body_path(repo_root: &Path, slug: &str) -> Result<PathBuf, String> {
    let clean = slug.trim();
    if clean.is_empty()
        || clean.contains('/')
        || clean.contains('\\')
        || clean.contains("..")
        || clean.starts_with('.')
    {
        return Err(format!("invalid skill slug: {slug}"));
    }

    let manifest_path = skill_manifest_path(repo_root);
    if manifest_path.is_file()
        && let Some(path) = skill_body_path_from_manifest(repo_root, &manifest_path, clean)? {
            return Ok(path);
        }

    let path = repo_root.join("skills").join(clean).join("SKILL.md");
    if !path.is_file() {
        return Err(format!("skill body not found: {}", path.display()));
    }
    Ok(path)
}

fn skill_body_path_from_manifest(
    repo_root: &Path,
    manifest_path: &Path,
    slug: &str,
) -> Result<Option<PathBuf>, String> {
    let payload = routing_engine::route::read_json(manifest_path)?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", manifest_path.display()))?;
    let key_index = keys
        .iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect::<std::collections::HashMap<_, _>>();
    let idx_slug = *key_index
        .get("slug")
        .ok_or_else(|| format!("manifest missing slug key: {}", manifest_path.display()))?;
    let Some(idx_skill_path) = key_index.get("skill_path").copied() else {
        return Ok(None);
    };
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", manifest_path.display()))?;
    for row in rows.iter().filter_map(Value::as_array) {
        if row.get(idx_slug).and_then(Value::as_str) != Some(slug) {
            continue;
        }
        let Some(skill_path) = row.get(idx_skill_path).and_then(Value::as_str) else {
            continue;
        };
        if skill_path.starts_with('/')
            || skill_path.contains("..")
            || !skill_path.ends_with("SKILL.md")
        {
            return Err(format!("invalid skill_path for {slug}: {skill_path}"));
        }
        let path = repo_root.join(skill_path);
        if !path.is_file() {
            return Err(format!("skill body not found: {}", path.display()));
        }
        return Ok(Some(path));
    }
    Ok(None)
}

pub(super) fn tool_skill_route_status(repo_root: &Path) -> Result<String, String> {
    let runtime_path = skill_routing_runtime_json(repo_root);
    let manifest_path = skill_manifest_path(repo_root);
    let mut remediation = Vec::new();
    if !runtime_path.is_file() {
        remediation.push(format!(
            "generate repository runtime artifacts so {} exists",
            runtime_path.to_string_lossy()
        ));
    }
    if !manifest_path.is_file() {
        remediation.push(format!(
            "generate repository runtime artifacts so {} exists",
            manifest_path.to_string_lossy()
        ));
    }
    remediation.push("call framework_snapshot for runtime details".to_string());
    Ok(json!({
        "schema_version": "cowork-skill-route-status-v1",
        "authority": "router-rs-framework",
        "repo_root": repo_root.to_string_lossy(),
        "skills_dir_exists": repo_root.join("skills").is_dir(),
        "runtime_path": runtime_path.to_string_lossy(),
        "runtime_exists": runtime_path.is_file(),
        "manifest_path": manifest_path.to_string_lossy(),
        "manifest_exists": manifest_path.is_file(),
        "routing_tools_exposed": skill_runtime_available(repo_root),
        "remediation": remediation,
    })
    .to_string())
}

pub fn build_evidence_entry(arguments: &Value) -> Result<Map<String, Value>, String> {
    let tool_name = arguments
        .get("tool_name")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: tool_name")?;
    let command = arguments
        .get("command")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: command")?;
    let exit_code = arguments.get("exit_code").and_then(Value::as_i64);
    let output = arguments.get("output").and_then(Value::as_str);

    let mut entry = Map::new();
    entry.insert("kind".to_string(), json!("mcp_record_evidence"));
    entry.insert("source".to_string(), json!("mcp_record_evidence"));
    entry.insert("tool_name".to_string(), json!(tool_name));
    entry.insert("command_preview".to_string(), json!(command));
    entry.insert(
        "recorded_at".to_string(),
        json!(crate::hooks::current_local_timestamp()),
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

pub(super) fn tool_record_evidence(arguments: &Value, repo_root: &Path) -> Result<String, String> {
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

    crate::hooks::append_evidence_index(repo_root, None, entry)?;

    // H2 FIX: Invalidate caches after evidence is written to ensure fresh data on next read
    invalidate_evidence_caches();

    let exit_display = exit_code
        .map(|ec| ec.to_string())
        .unwrap_or_else(|| "null".to_string());
    let honor_note = " (honor-system: not bound to host tool execution — verify independently)";
    Ok(format!(
        "Evidence recorded{honor_note}: {tool_name_display} '{command_display}' -> exit={exit_display}"
    ))
}

/// 获取 evidence output 的最大字符数配置。
/// 默认 2000 字符，可通过 `ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS` 环境变量覆盖。
pub(super) fn evidence_output_max_chars() -> usize {
    static CACHED: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(2000)
    })
}

pub(super) fn tool_session_checkpoint(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, String> {
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
    let task_id = arguments.get("task_id").and_then(Value::as_str);

    let payload = crate::hooks::build_automatic_continuity_checkpoint_payload(
        repo_root,
        summary,
        &next_actions.join(", "),
        task_id,
        true,
        false,
    );
    crate::hooks::write_framework_session_artifacts(payload)
        .map_err(|e| format!("Checkpoint write failed: {e}"))?;

    // H2 FIX: Invalidate caches after checkpoint is written to ensure fresh data on next read
    invalidate_evidence_caches();

    Ok(format!(
        "Checkpoint written: summary={}, next_actions_count={}",
        summary.chars().count(),
        next_actions.len()
    ))
}

pub(super) fn goal_suggests_review_work(goal_state: &Value) -> bool {
    if goal_state
        .get("goal")
        .and_then(Value::as_str)
        .is_some_and(is_review_prompt)
    {
        return true;
    }
    goal_state
        .get("done_when")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().filter_map(Value::as_str).any(is_review_prompt))
}

pub(super) fn task_lifecycle_profile(task_view: &core_state::task_state::ResolvedTaskView) -> &str {
    task_view
        .goal_state
        .as_ref()
        .and_then(|g| g.get("lifecycle_profile"))
        .and_then(Value::as_str)
        .unwrap_or("my-light")
}

/// Returns true when the lifecycle_profile string represents an interactive profile
/// (either "my-light" — deprecated alias — or "interactive").
pub(super) fn is_interactive_lifecycle_profile(profile: &str) -> bool {
    profile == "my-light" || profile == "interactive"
}

pub(super) fn mcp_closeout_gate_mode_narrative(
    repo_root: &Path,
    host_id: &str,
    host_name: &str,
    lifecycle_profile: &str,
) -> String {
    if is_interactive_lifecycle_profile(lifecycle_profile) {
        return "interactive/my-light: MCP hard block disabled — closeout_gate reports findings only (advisory).".to_string();
    }
    framework_kernel::runtime_registry::harness_capability_exception_rationale(
        repo_root,
        host_id,
        "closeout_evidence_hooks",
    )
    .unwrap_or_else(|| {
        format!(
            "{host_name}: no shell closeout_evidence_hooks — MCP tool layer evaluates closeout (see RUNTIME_REGISTRY harness_capability_exceptions)."
        )
    })
}

pub(super) fn mcp_closeout_hard_block_metadata(
    repo_root: &Path,
    host_id: &str,
    lifecycle_profile: &str,
    all_clear: bool,
) -> bool {
    if all_clear || is_interactive_lifecycle_profile(lifecycle_profile) {
        return false;
    }
    framework_kernel::runtime_registry::closeout_evidence_hooks_unsupported_on_host(
        repo_root, host_id,
    )
}

pub(super) fn desktop_review_evidence_attested(
    arguments: &Value,
    repo_root: &Path,
    task_id: &str,
) -> bool {
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

    if review_lanes_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&review_lanes_dir) {
            let mut valid_findings_found = false;
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md")
                    && let Ok(content) = std::fs::read_to_string(&path)
                        && !content.trim().is_empty() {
                            valid_findings_found = true;
                            break;
                        }
            }
            if valid_findings_found {
                return true;
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
        core_policy::registry_review_gate::is_reviewer_lane_from_registry(lane, Some(repo_root));
    let fork = fork_context_from_values(arguments, None);
    review_independent_reviewer_evidence(fork, review_lane)
}

#[derive(Debug, Clone)]
pub struct McpCloseoutGateVerdict {
    pub formatted: String,
}

pub fn evaluate_mcp_closeout_gate(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<McpCloseoutGateVerdict, String> {
    let task_id_override = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let task_view = resolve_task_view(repo_root, task_id_override);
    let mut findings: Vec<String> = Vec::new();
    let host_name = mcp_host_display_label(host_id);
    let lifecycle_profile = task_lifecycle_profile(&task_view);

    if let Some(rationale) =
        framework_kernel::runtime_registry::harness_capability_exception_rationale(
            repo_root,
            host_id,
            "closeout_evidence_hooks",
        )
    {
        findings.push(format!("harness: closeout_evidence_hooks — {rationale}"));
    }
    if let Some(rationale) =
        framework_kernel::runtime_registry::harness_capability_exception_rationale(
            repo_root,
            host_id,
            "review_gate_router_observation",
        )
    {
        findings.push(format!(
            "harness: review_gate_router_observation — {rationale}"
        ));
    }

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
            && core_state::state_manager::task_evidence_success_only_self_attested(
                repo_root, task_id,
            )
        {
            findings.push(
                "WARN: evidence: only self-attested MCP record_evidence rows — verify independently"
                    .to_string(),
            );
        }
    }
    let summary_path = task_artifact_dir(
        repo_root,
        if task_id.is_empty() {
            None
        } else {
            Some(task_id)
        },
    )
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
            "WARN: review_gate: GOAL suggests review work but no hook-level reviewer evidence on {host_name} — spawn reviewer_lanes with fork_context=false and write review-lanes/*.md, or pass reviewer_lane + fork_context=false in closeout_gate args"
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

    let formatted = format!("[Closeout Gate] {verdict_label}\n\n{}", findings.join("\n"));

    let _hard_block =
        mcp_closeout_hard_block_metadata(repo_root, host_id, lifecycle_profile, all_clear);

    // §4.1: 持久化 review gate 状态到 artifacts/current/<task_id>/review_gate.json
    persist_review_gate_status(repo_root, task_id, all_clear, &findings, lifecycle_profile);

    Ok(McpCloseoutGateVerdict { formatted })
}

/// §4.1: 持久化 review gate 状态到 task artifact 目录。
pub(super) fn persist_review_gate_status(
    repo_root: &Path,
    task_id: &str,
    cleared: bool,
    findings: &[String],
    lifecycle_profile: &str,
) {
    if task_id.is_empty() {
        return;
    }
    let dir = task_artifact_dir(repo_root, Some(task_id));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[router-rs] review_gate persist: mkdir failed: {e}");
        return;
    }
    let path = dir.join("review_gate.json");
    let payload = json!({
        "task_id": task_id,
        "cleared": cleared,
        "cleared_at": if cleared { Some(crate::hooks::current_local_timestamp()) } else { None },
        "lifecycle_profile": lifecycle_profile,
        "findings": findings,
        "recorded_at": crate::hooks::current_local_timestamp(),
    });
    if let Ok(text) = serde_json::to_string_pretty(&payload)
        && let Err(e) = std::fs::write(&path, format!("{text}\n")) {
            eprintln!("[router-rs] review_gate persist: write failed: {e}");
        }
}

pub fn tool_closeout_gate(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, String> {
    Ok(evaluate_mcp_closeout_gate(arguments, repo_root, host_id)?.formatted)
}

pub(super) fn tool_closeout_record_write(
    arguments: &Value,
    repo_root: &Path,
    _host_id: &str,
) -> Result<String, String> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: task_id")?;
    let summary = arguments
        .get("summary")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: summary")?;
    let verification_status = arguments
        .get("verification_status")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: verification_status")?;
    match verification_status {
        "passed" | "failed" | "partial" | "not_run" => {},
        _ => return Err(format!(
            "Invalid verification_status: {verification_status}. Must be one of: passed, failed, partial, not_run"
        )),
    }

    let mut record = Map::new();
    record.insert(
        "schema_version".to_string(),
        json!(crate::hooks::closeout_record_schema_version()),
    );
    record.insert("task_id".to_string(), json!(task_id));
    record.insert(
        "ended_at".to_string(),
        json!(crate::hooks::current_local_timestamp()),
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
    if let Some(blockers) = arguments.get("blockers").and_then(Value::as_array)
        && !blockers.is_empty() {
            record.insert("blockers".to_string(), json!(blockers));
        }
    if let Some(risks) = arguments.get("risks").and_then(Value::as_array)
        && !risks.is_empty() {
            record.insert("risks".to_string(), json!(risks));
        }
    if let Some(notes) = arguments.get("notes").and_then(Value::as_str)
        && !notes.is_empty() {
            record.insert("notes".to_string(), json!(notes));
        }

    // Ensure parent directory exists
    let record_path = crate::hooks::closeout_record_path_for_task(repo_root, task_id)
        .map_err(|e| format!("invalid task_id: {e}"))?;
    if let Some(parent) = record_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create closeout directory failed: {e}"))?;
    }

    // Write the record
    let content = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("serialize closeout record failed: {e}"))?;
    fs::write(&record_path, &content).map_err(|e| format!("write closeout record failed: {e}"))?;

    // Evaluate the record
    let eval_result =
        crate::hooks::evaluate_closeout_record_file_for_task(repo_root, task_id, &record_path);
    let eval = match eval_result {
        Ok(v) => v,
        Err(e) => json!({"error": e}),
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

    let result = json!({
        "closeout_allowed": closeout_allowed,
        "violations": violations,
    });

    serde_json::to_string_pretty(&result)
        .map_err(|e| format!("serialize closeout result failed: {e}"))
}

pub(super) const WEB_FETCH_MAX_REDIRECTS: usize = 5;

/// Build a reqwest blocking client with proxy, timeout, no-redirect, and DNS pins.
fn build_web_fetch_client(pin_host: &str, addrs: &[std::net::SocketAddr]) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(WEB_FETCH_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("router-rs-framework-mcp/0.1");
    if let Some(proxy_url) = http_util::cached_proxy_url()
        && let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
            builder = builder.proxy(proxy);
        }
    for addr in addrs {
        builder = builder.resolve(pin_host, *addr);
    }
    builder.build().map_err(|err| format!("web_fetch client build failed: {err}"))
}

pub(super) fn tool_web_fetch(arguments: &Value) -> Result<String, String> {
    let url = arguments
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("Missing required argument: url")?;
    // Validate + resolve DNS in one pass to pin results before building client.
    let (parsed_url_str, initial_addr_strs) =
        crate::hooks::validate_and_resolve_web_fetch_url(url)?;
    let parsed_url = reqwest::Url::parse(&parsed_url_str)
        .map_err(|e| format!("web_fetch URL parse error: {e}"))?;
    let initial_addrs: Vec<std::net::SocketAddr> = initial_addr_strs
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();
    let max_bytes = arguments
        .get("max_bytes")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(WEB_FETCH_MAX_BYTES_DEFAULT)
        .clamp(1, WEB_FETCH_MAX_BYTES_DEFAULT);
    let pin_host = parsed_url
        .host_str()
        .ok_or_else(|| format!("web_fetch URL missing host: {url}"))?;
    let mut client = build_web_fetch_client(pin_host, &initial_addrs)?;
    let mut current_url = url.to_string();
    let mut response = None;
    for hop in 0..=WEB_FETCH_MAX_REDIRECTS {
        let resp = client
            .get(&current_url)
            .send()
            .map_err(|err| format!("web_fetch request failed: {err}"))?;
        if resp.status().is_redirection() {
            if hop >= WEB_FETCH_MAX_REDIRECTS {
                return Err(format!(
                    "web_fetch exceeded {WEB_FETCH_MAX_REDIRECTS} redirects"
                ));
            }
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    format!(
                        "web_fetch redirect missing Location header (status {})",
                        resp.status()
                    )
                })?;
            current_url = crate::hooks::resolve_web_fetch_redirect(&current_url, location)?;
            // Pin DNS for redirect target to prevent DNS rebinding TOCTOU.
            let redirect_parsed = reqwest::Url::parse(&current_url)
                .map_err(|err| format!("web_fetch redirect URL parse failed: {err}"))?;
            let rp_host = redirect_parsed
                .host_str()
                .ok_or_else(|| format!("web_fetch redirect URL missing host: {current_url}"))?;
            let rp_port = redirect_parsed.port().unwrap_or(if redirect_parsed.scheme() == "https" { 443 } else { 80 });
            let rp_addrs: Vec<std::net::SocketAddr> =
                crate::hooks::resolve_web_fetch_addresses(rp_host, rp_port)?
                    .iter()
                    .filter_map(|s| s.parse().ok())
                    .collect();
            // Rebuild client with pinned DNS for redirect target — proxy is inherited via build_web_fetch_client.
            client = build_web_fetch_client(rp_host, &rp_addrs)?;
            continue;
        }
        response = Some(resp);
        break;
    }
    let response = response.ok_or_else(|| "web_fetch: no response".to_string())?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = response
        .bytes()
        .map_err(|err| format!("web_fetch read body failed: {err}"))?;
    let truncated = body.len() > max_bytes;
    let slice = &body[..body.len().min(max_bytes)];
    let body_text = String::from_utf8_lossy(slice).into_owned();
    let payload = json!({
        "url": current_url,
        "status": status,
        "content_type": content_type,
        "content_length": body.len(),
        "truncated": truncated,
        "body": body_text,
    });
    serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("web_fetch serialize failed: {err}"))
}

// ---------------------------------------------------------------------------
// Routing evolution: read telemetry log, aggregate, suggest improvements
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct RouteLogEntry {
    ts: Option<String>,
    kind: Option<String>,
    task: Option<String>,
    skill: Option<String>,
    confidence: Option<f32>,
    reroute: Option<bool>,
    #[allow(dead_code)] // Telemetry field, reserved for latency analysis.
    latency_ms: Option<u64>,
    parity_gate: Option<String>,
    #[serde(default)]
    reasons: Vec<String>,
    #[allow(dead_code)] // Telemetry metadata; reserved for token budget analysis.
    matched_tokens: Option<usize>,
}

/// Aggregate routing stats from telemetry journal.
pub(super) fn tool_routing_evolution(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: operation (stats|analyze|extract|calibrate)")?;
    let skill_filter = arguments.get("skill").and_then(Value::as_str);
    let lookback_days = arguments.get("days").and_then(Value::as_u64).unwrap_or(0);

    let journal_path = repo_root.join("artifacts/telemetry/events.jsonl");
    if !journal_path.exists() {
        return Err(format!(
            "Telemetry journal not found at {}",
            journal_path.display()
        ));
    }

    let file = fs::File::open(&journal_path)
        .map_err(|e| format!("open journal: {e}"))?;
    let reader = std::io::BufReader::new(file);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = if lookback_days > 0 {
        now.saturating_sub(lookback_days * 86400)
    } else {
        0
    };

    let mut entries: Vec<RouteLogEntry> = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("read journal line: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: RouteLogEntry =
            serde_json::from_str(&line).map_err(|e| format!("parse journal line: {e}"))?;
        if entry.kind.as_deref() != Some("route_decision") {
            continue;
        }

        if cutoff > 0
            && let Some(ts) = &entry.ts
                && let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(ts)
                    && parsed.timestamp() < cutoff as i64 {
                        continue;
                    }

        if let Some(filter) = skill_filter
            && entry.skill.as_deref() != Some(filter) {
                continue;
            }

        entries.push(entry);
    }

    match operation {
        "stats" => Ok(routing_stats(&entries)),
        "analyze" => Ok(routing_analyze(&entries)),
        "extract" => Ok(routing_extract(&entries)),
        "calibrate" => Ok(routing_calibrate(&entries)),
        _ => Err(format!(
            "Unknown operation: {operation}. Use stats|analyze|extract|calibrate"
        )),
    }
}

fn routing_stats(entries: &[RouteLogEntry]) -> String {
    use std::collections::HashMap;

    let total = entries.len();
    let mut per_skill: HashMap<&str, (u64, f64, u64)> = HashMap::new(); // count, sum_conf, reroute_count
    let mut gate_counts: HashMap<&str, u64> = HashMap::new();
    let mut total_reroute = 0u64;

    for e in entries {
        let skill = e.skill.as_deref().unwrap_or("none");
        let (count, sum, reroute) =
            per_skill.entry(skill).or_insert((0, 0.0, 0));
        *count += 1;
        *sum += e.confidence.unwrap_or(0.0) as f64;
        if e.reroute.unwrap_or(false) {
            *reroute += 1;
            total_reroute += 1;
        }
        let gate = e.parity_gate.as_deref().unwrap_or("unknown");
        *gate_counts.entry(gate).or_insert(0) += 1;
    }

    let mut skills: Vec<serde_json::Value> = per_skill
        .iter()
        .map(|(slug, (count, sum, reroute))| {
            json!({
                "skill": slug,
                "count": count,
                "avg_confidence": format!("{:.2}", sum / *count as f64),
                "reroutes": reroute,
            })
        })
        .collect();
    skills.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));

    let mut gates: Vec<serde_json::Value> = gate_counts
        .iter()
        .map(|(mode, count)| json!({"mode": mode, "count": count}))
        .collect();
    gates.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));

    let total_with_candidates = entries
        .iter()
        .filter(|e| !e.reasons.is_empty())
        .count();

    json!({
        "total_routes": total,
        "timespan_days": "...",  // caller can compute from first/last ts
        "skills_with_logs": per_skill.len(),
        "total_reroute": total_reroute,
        "reroute_rate": format!("{:.1}%", total_reroute as f64 / total.max(1) as f64 * 100.0),
        "entries_with_reasons": total_with_candidates,
        "gate_breakdown": gates,
        "per_skill": skills,
    })
    .to_string()
}

fn routing_analyze(entries: &[RouteLogEntry]) -> String {
    use std::collections::HashMap;

    let total = entries.len();
    if total == 0 {
        return json!({"error": "No routing entries to analyze", "suggestions": ["使用一段时间后再运行 analyze"]}).to_string();
    }

    // Per-skill: avg confidence progression and gate rate
    let mut skill_data: HashMap<&str, Vec<(f32, bool, &str)>> = HashMap::new();
    for e in entries {
        let skill = e.skill.as_deref().unwrap_or("none");
        skill_data
            .entry(skill)
            .or_default()
            .push((e.confidence.unwrap_or(0.0), e.reroute.unwrap_or(false), e.parity_gate.as_deref().unwrap_or("direct")));
    }

    let mut analysis: Vec<serde_json::Value> = skill_data
        .iter()
        .map(|(slug, data)| {
            let n = data.len();
            let avg_conf = data.iter().map(|(c, _, _)| *c as f64).sum::<f64>() / n.max(1) as f64;
            let reroutes = data.iter().filter(|(_, r, _)| *r).count();
            let gate_count = data.iter().filter(|(_, _, g)| *g != "direct").count();
            let gate_rate = gate_count as f64 / n.max(1) as f64;

            json!({
                "skill": slug,
                "count": n,
                "avg_confidence": format!("{:.2}", avg_conf),
                "reroute_rate": format!("{:.1}%", reroutes as f64 / n.max(1) as f64 * 100.0),
                "gate_rate": format!("{:.1}%", gate_rate * 100.0),
            })
        })
        .collect();
    analysis.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));

    // Find skills with high reroute rate (potential confusion)
    let high_reroute: Vec<&serde_json::Value> = analysis
        .iter()
        .filter(|a| {
            a["reroute_rate"].as_str().and_then(|r| {
                r.trim_end_matches('%').parse::<f64>().ok()
            }).unwrap_or(0.0) > 10.0
        })
        .collect();

    json!({
        "total_routes": total,
        "skills_analyzed": analysis.len(),
        "per_skill": analysis,
        "alerts": {
            "high_confusion_skills": high_reroute.iter().map(|a| a["skill"].as_str().unwrap_or("?")).collect::<Vec<_>>(),
            "note": "reroute_rate > 10% suggests embedding confusion or ambiguous utterances"
        },
        "suggestions": [
            if high_reroute.len() > 2 {
                format!("{} skills have >10% reroute rate — consider adding more utterances for these", high_reroute.len())
            } else {
                "Reroute rates look healthy".to_string()
            },
            "Run `calibrate` to get threshold tuning suggestions"
        ]
    })
    .to_string()
}

fn routing_extract(entries: &[RouteLogEntry]) -> String {
    let mut extracted: Vec<serde_json::Value> = entries
        .iter()
        .filter(|e| {
            let conf = e.confidence.unwrap_or(0.0);
            let skill = e.skill.as_deref().unwrap_or("");
            conf >= 0.7
                && !skill.is_empty()
                && skill != "none"
                && !e.reroute.unwrap_or(false)
                && e.task.as_ref().is_some_and(|t| t.len() > 4)
        })
        .map(|e| {
            json!({
                "query": e.task.as_deref().unwrap_or(""),
                "skill": e.skill.as_deref().unwrap_or(""),
                "confidence": e.confidence.unwrap_or(0.0),
                "parity_gate": e.parity_gate.as_deref().unwrap_or("direct"),
            })
        })
        .collect();

    // Deduplicate by (query, skill)
    let mut seen = std::collections::HashSet::new();
    extracted.retain(|e| {
        let key = format!(
            "{}/{}",
            e["query"].as_str().unwrap_or(""),
            e["skill"].as_str().unwrap_or("")
        );
        seen.insert(key)
    });

    // Sort by confidence desc, limit to top 100
    extracted.sort_by(|a, b| {
        b["confidence"]
            .as_f64()
            .partial_cmp(&a["confidence"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    extracted.truncate(100);

    let by_skill: std::collections::BTreeMap<&str, Vec<&serde_json::Value>> =
        extracted.iter().filter_map(|e| {
            e["skill"].as_str().map(|s| (s, e))
        }).fold(std::collections::BTreeMap::new(), |mut map, (skill, entry)| {
            map.entry(skill).or_default().push(entry);
            map
        });

    let mut summary: Vec<serde_json::Value> = by_skill
        .iter()
        .map(|(skill, items)| {
            json!({
                "skill": skill,
                "utterance_count": items.len(),
                "queries": items.iter().map(|e| e["query"].as_str().unwrap_or("")).collect::<Vec<_>>(),
            })
        })
        .collect();
    summary.sort_by(|a, b| {
        b["utterance_count"]
            .as_u64()
            .cmp(&a["utterance_count"].as_u64())
    });

    json!({
        "total_extracted": extracted.len(),
        "by_skill": summary,
        "note": "Confidence >= 0.7, no reroute. Add these to utterences and re-run centroid calibration.",
    })
    .to_string()
}

fn routing_calibrate(entries: &[RouteLogEntry]) -> String {
    use std::collections::HashMap;

    if entries.len() < 10 {
        return json!({
            "suggestions": [],
            "note": "Need at least 10 entries for calibration. Run more queries first."
        }).to_string();
    }

    // Per-skill stats for threshold suggestions
    let mut skill_data: HashMap<&str, Vec<f32>> = HashMap::new();
    for e in entries {
        let skill = e.skill.as_deref().unwrap_or("none");
        skill_data.entry(skill).or_default().push(e.confidence.unwrap_or(0.0));
    }

    let mut suggestions: Vec<serde_json::Value> = Vec::new();
    for (skill, confs) in &skill_data {
        if confs.len() < 5 {
            continue;
        }
        let n = confs.len();
        let mut sorted = confs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted[n / 2];
        let p90_idx = ((n as f64 * 0.9) as usize).min(n - 1);
        let p90 = sorted[p90_idx];

        // If median confidence is high, τ_auto can be raised
        if median > 0.72 {
            suggestions.push(json!({
                "skill": skill,
                "metric": "confidence distribution",
                "median": format!("{:.2}", median),
                "p90": format!("{:.2}", p90),
                "suggestion": format!("τ_auto can be raised (current 0.65). Median confidence for {skill} is {median:.2}"),
                "action": "edit SRC_CONFIG.json thresholds"
            }));
        }
        // If median confidence is low, add more utterances
        if median < 0.45 && n > 20 {
            suggestions.push(json!({
                "skill": skill,
                "metric": "low confidence",
                "median": format!("{:.2}", median),
                "suggestion": format!("Median confidence for {skill} is {median:.2}. Add more seed utterances to distinguish it."),
                "action": "run `routing_evolution extract` for candidate utterances"
            }));
        }
    }

    json!({
        "total_entries": entries.len(),
        "skills_with_enough_data": skill_data.len(),
        "suggestions": suggestions,
        "note": "Thresholds are per-embedding-model. Adjust in thresholds_per_model.<model_name> in scoring config.",
    })
    .to_string()
}
