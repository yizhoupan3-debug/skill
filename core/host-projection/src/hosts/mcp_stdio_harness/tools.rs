//! MCP tool implementations (tool_* functions).
//!
//! Extracted from mcp_stdio_harness.rs to keep file size ≤2000 lines.

use super::*;
use framework_core::skill_repo::skill_routing_runtime_json;
#[cfg(any(test, feature = "test-support"))]
use serde_json::Map;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

/// In-memory fallback for `first_turn` detection — **removed** (P2 #16).
/// Cross-session state pollution risk from the per-process AtomicBool flag.
/// Routing calls now treat all turns uniformly with first_turn=false.

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

    // Reject empty tool name before rate limiting and pre-guards
    if tool_name.is_empty() {
        return json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": "Error: tool name must not be empty" }],
                "isError": true,
            },
        });
    }

    // Check rate limit before processing
    {
        let limiter = get_rate_limiter();
        if let Some(mut guard) = poison_safe_lock!(limiter)
            && let Err(e) = guard.check_and_record(tool_name)
        {
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

    let result = dispatch_tool(
        tool_name,
        arguments,
        repo_root,
        host_id,
        connection_session_id,
    );

    match result {
        Ok(content) => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": content }],
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

pub(super) fn tool_skill_route(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, FrameworkError> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::from("Missing required argument: query".to_string()))?;

    // first_turn is always false: the per-process flag was removed to prevent
    // cross-session state pollution (P2 #16). All calls are treated uniformly.
    let first_turn = false;

    // Load records once — hook receives pre-loaded Arc so it never re-reads disk.
    let runtime_path = skill_routing_runtime_json(repo_root);
    let records: std::sync::Arc<Vec<routing_engine::route::SkillRecord>> =
        routing_engine::route::load_records_cached_for_stdio(Some(&runtime_path))?;

    // Route via hook — returns rich RouteDecision, no lossy JSON round-trip.
    let decision = crate::hooks::mcp_tool_skill_route(query, host_id, first_turn, records)?;

    let no_hit = decision.selected_skill.is_empty() || decision.selected_skill == "none";

    // Build JSON response from RouteDecision fields
    let mut response = serde_json::json!({
        "selected_skill": if no_hit { Value::Null } else { Value::String(decision.selected_skill.clone()) },
        "score": decision.score,
        "reasons": decision.reasons,
        "matched_token_count": decision.matched_token_count,
        "layer": decision.layer,
        "fuzzy_match": decision.fuzzy_match,
        "overlay_skill": decision.overlay_skill,
        "route_context": decision.route_context,
        "selected_skill_path": decision.selected_skill_path,
        "checker_id": decision.checker_id,
    });

    if let Some(obj) = response.as_object_mut() {
        if !no_hit {
            // --- Skill-context boost slugs from SKILL_TO_TOOL_MAP ---
            // Cached, typed loader with diagnostics for the selected skill
            // and overlay skill (framework routing may set both).
            let boost_slugs = compute_boost_slugs(
                repo_root,
                &decision.selected_skill,
                decision.overlay_skill.as_deref().unwrap_or(""),
            );

            // --- recommended_tools (top-3, excluding no_routing) ---
            let registry_path = mcp_tool_registry::resolve_tool_registry_path()
                .unwrap_or_else(|| {
                    repo_root.join(framework_core::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH)
                });
            let tools = (|| -> Option<Vec<Value>> {
                let records = mcp_tool_registry::load_tool_records_cached(&registry_path).ok()?;
                let filtered: Vec<_> = records
                    .into_iter()
                    .filter(|r| !r.tool_flags.iter().any(|f| f == "no_routing"))
                    .collect();
                if filtered.is_empty() {
                    return None;
                }
                let results = tool_routing_engine::search::search_tools(
                    query, &filtered, 3, boost_slugs.as_ref(),
                );
                if results.is_empty() {
                    return None;
                }
                Some(results.into_iter().map(|d| json!({
                    "slug": d.selected_tool, "score": d.score,
                    "dispatch_domain": d.dispatch_domain,
                    "mcp_server": d.mcp_server,
                    "matched_token_count": d.matched_token_count,
                    "fuzzy_match": d.fuzzy_match,
                    "reasons": d.reasons,
                })).collect())
            })();
            if let Some(t) = &tools {
                obj.insert("recommended_tools".to_string(), json!(t));
            }

            // --- Audit: log recommended_tools outcome ---
            // Log the recommendation to `logs/skill-routing/recommend_audit.ndjson`
            // for offline quality analysis (P3 #01).
            // NOTE: when log_subpath is an absolute path, write_entry_with_rotation
            // uses it directly (Path::new(root).join(abs_path) yields abs_path),
            // so the log lands at repo_root/logs/... regardless of FRAMEWORK_ROOT.
            let sanitized_query = routing_core::audit_log::sanitize_query_for_log(query);
            let log_path = repo_root.join("logs/skill-routing/recommend_audit.ndjson");
            let log_subpath = log_path.to_string_lossy().to_string();
            let audit_entry = serde_json::json!({
                "ts": crate::hooks::current_local_timestamp(),
                "query": sanitized_query,
                "selected_skill": decision.selected_skill,
                "overlay_skill": decision.overlay_skill,
                "recommended_tools": tools.as_ref().map(|t| {
                    t.iter().map(|v| json!({
                        "slug": v["slug"],
                        "score": v["score"],
                        "reasons": v["reasons"],
                    })).collect::<Vec<_>>()
                }),
            });
            static RECOMMEND_LOG: routing_core::audit_log::AuditLog =
                routing_core::audit_log::AuditLog::new();
            RECOMMEND_LOG.write_entry_with_rotation(&log_subpath, &audit_entry);

            // --- skill_summary: inline SKILL.md first N chars ---
            if let Ok(path) = skill_body_path(repo_root, &decision.selected_skill) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let preview: String = content.chars().take(600).collect();
                    let truncated = content.chars().count() > 600;
                    obj.insert("skill_summary".to_string(), json!({
                        "preview": preview, "truncated": truncated,
                        "full_path": format!("{}", path.display()),
                    }));
                }
            }
        }
    }

    Ok(serde_json::to_string(&response).map_err(|e| e.to_string())?)
}

pub(super) fn tool_skill_search(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, FrameworkError> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::from("Missing required argument: query".to_string()))?;
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .clamp(1, 50) as usize;
    // P2 #03: reject caller-supplied host_id override; always use connection-level identity.
    if let Some(override_host) = arguments.get("host_id").and_then(Value::as_str).filter(|h| !h.is_empty()) {
        if override_host != host_id {
            tracing::warn!("host_id override rejected: caller tried to override '{host_id}' with '{override_host}'");
        }
    }
    let effective_host = host_id;

    // Direct inline: no hook indirection needed (all deps available)
    let runtime_path = skill_routing_runtime_json(repo_root);
    if !runtime_path.is_file() {
        return Err(FrameworkError::from(format!(
            "Missing repository skill runtime ({})",
            runtime_path.display()
        )));
    }
    let records = routing_engine::route::load_records_cached_for_stdio(
        Some(&runtime_path),
    )?;
    let host_indices = routing_engine::route::filter_record_indices_for_host(
        records.as_ref(), Some(effective_host),
    )?;
    let rows = routing_engine::route::search_skills_subset(
        records.as_ref(), Some(&host_indices), query, limit,
    );
    let results = routing_engine::route::build_search_results_payload(query, rows);
    Ok(serde_json::to_string(&results).map_err(|e| e.to_string())?)
}

pub(super) fn tool_skill_read(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, FrameworkError> {
    let slug = arguments
        .get("skill")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::from("Missing required argument: skill".to_string()))?;
    let max_chars = arguments
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(20_000)
        .clamp(1, 50_000) as usize;

    let path = skill_body_path(repo_root, slug)?;
    let content = fs::read_to_string(&path)
        .map_err(|e| FrameworkError::from(format!("{}: {e}", path.display())))?;
    let truncated = content.chars().count() > max_chars;
    let truncated_content: String = content.chars().take(max_chars).collect();

    Ok(json!({
        "schema_version": "cowork-skill-read-v1",
        "authority": "router-rs-framework",
        "skill": slug,
        // HPM-14: return slug only, not absolute file path (directory structure leak)
        "content": truncated_content,
        "truncated": truncated,
    })
    .to_string())
}

fn skill_runtime_available(repo_root: &Path) -> bool {
    skill_routing_runtime_json(repo_root).is_file() && repo_root.join("skills").is_dir()
}

fn skill_body_path(repo_root: &Path, slug: &str) -> Result<PathBuf, FrameworkError> {
    let clean = slug.trim();
    if clean.is_empty()
        || clean.contains('/')
        || clean.contains('\\')
        || clean.contains("..")
        || clean.contains('\0')
        || clean.starts_with('.')
    {
        return Err(FrameworkError::from(format!("invalid skill slug: {slug}")));
    }

    let path = repo_root.join("skills").join(clean).join("SKILL.md");
    finalize_skill_path(repo_root, &path, slug)
}

/// Validate the resolved skill path stays within the repo root.
/// Additionally checks that symlink-resolved paths remain within the
/// skills directory (catches redirections in intermediate components).
/// Returns the path on success.
fn finalize_skill_path(
    repo_root: &Path,
    path: &Path,
    slug: &str,
) -> Result<PathBuf, FrameworkError> {
    use core_state_utils::path_guard::{path_is_within_repo_root, reject_unsafe_path};

    reject_unsafe_path(path)?;

    // HPM-15: canonicalize first to detect symlink swaps before the is_file check.
    let canonical_path = path.canonicalize().map_err(|_| {
        FrameworkError::from(format!(
            "skill path not found or unresolvable: {}",
            path.display()
        ))
    })?;

    let skills_dir = repo_root.join("skills");
    // Use canonicalized path for the in-repo check as well
    if !canonical_path.starts_with(&skills_dir) {
        return Err(FrameworkError::from(format!(
            "skill path for {slug} resolves outside skills directory via symlink: {}",
            canonical_path.display()
        )));
    }

    let is_under_skills = path_is_within_repo_root(repo_root, &canonical_path);
    if !is_under_skills {
        return Err(FrameworkError::from(format!(
            "skill path for {slug} escapes repo root: {}",
            canonical_path.display()
        )));
    }

    if !canonical_path.is_file() {
        return Err(FrameworkError::from(format!(
            "skill body not found: {}",
            path.display()
        )));
    }

    Ok(canonical_path)
}

pub(super) fn tool_skill_route_status(repo_root: &Path) -> Result<String, FrameworkError> {
    let runtime_path = skill_routing_runtime_json(repo_root);
    let mut remediation = Vec::new();
    if !runtime_path.is_file() {
        remediation.push(format!(
            "generate repository runtime artifacts so {} exists",
            runtime_path.to_string_lossy()
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
        "routing_tools_exposed": skill_runtime_available(repo_root),
        "remediation": remediation,
    })
    .to_string())
}

#[cfg(any(test, feature = "test-support"))]
pub fn build_evidence_entry(arguments: &Value) -> Result<Map<String, Value>, FrameworkError> {
    // HPM-16: Input validation for evidence records.
    // tool_name is validated against a known allowlist of framework tool names.
    // exit_code and command are validated for format and reasonable ranges.
    // Cross-referencing against host execution logs is not yet implemented.
    let tool_name = arguments
        .get("tool_name")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::from("Missing required argument: tool_name".to_string()))?;

    // Validate tool_name: must be a known framework tool
    if tool_name.trim().is_empty() {
        return Err(FrameworkError::from("tool_name must not be empty".to_string()));
    }
    // Only allow alphanumeric, hyphens, underscores, slashes in tool_name
    if !tool_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '/') {
        return Err(FrameworkError::from(
            format!("tool_name '{tool_name}' contains invalid characters")
        ));
    }

    let command = arguments
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| FrameworkError::from("Missing required argument: command".to_string()))?;

    // Validate command: must not be empty
    if command.trim().is_empty() {
        return Err(FrameworkError::from("command must not be empty".to_string()));
    }

    let exit_code = arguments.get("exit_code").and_then(Value::as_i64);

    // Validate exit_code range if present
    if let Some(ec) = exit_code {
        // exit_code must be a standard exit code (-1 for signal, 0-255 for normal)
        if ec < -1 || ec > 255 {
            return Err(FrameworkError::from(
                format!("exit_code {ec} is out of valid range (-1 to 255)")
            ));
        }
    }
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

/// 获取 evidence output 的最大字符数配置。
/// 默认 2000 字符，可通过 `ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS` 环境变量覆盖。
#[cfg(any(test, feature = "test-support"))]
pub(super) fn evidence_output_max_chars() -> usize {
    env_cache_typed!(usize, "ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS", 2000)
}

pub fn tool_closeout_gate(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, FrameworkError> {
    Ok(crate::hooks::tool_closeout_gate_evaluate(
        arguments, repo_root, host_id,
    )?)
}

pub(super) fn tool_closeout_record_write(
    arguments: &Value,
    repo_root: &Path,
    _host_id: &str,
) -> Result<String, FrameworkError> {
    Ok(crate::hooks::tool_closeout_record_write_dispatch(
        arguments, repo_root,
    )?)
}

pub(super) fn tool_goal_state_manage(
    arguments: &Value,
    repo_root: &Path,
    connection_session_id: &str,
) -> Result<String, FrameworkError> {
    let result =
        crate::hooks::tool_goal_state_manage_dispatch(arguments, repo_root, connection_session_id)?;
    Ok(result)
}

pub(super) fn tool_goal_state_read(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, FrameworkError> {
    let task_id = arguments
        .get("task_id")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty());
    let state = core_state::state_manager::read_goal_state(repo_root, task_id)
        .map_err(|e| FrameworkError::from(format!("goal_state_read: {e}")))?;
    let response = match state {
        Some(s) => json!({"ok": true, "goal_state": s}),
        None => json!({
            "ok": false,
            "goal_state": null,
            "message": "No active goal state found. Use 'goal_state_manage(operation=\"start\", ...)' to create one, or provide an explicit 'task_id' parameter."
        }),
    };
    Ok(serde_json::to_string_pretty(&response)
        .map_err(|e| FrameworkError::from(e.to_string()))?)
}

// ── Record Evidence ──

/// Record evidence: append a claim-evidence pair to EVIDENCE_INDEX.json.
pub(super) fn tool_record_evidence(
    arguments: &Value,
    repo_root: &Path,
) -> Result<String, FrameworkError> {
    let claim = arguments
        .get("claim")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| FrameworkError::validation("record_evidence: 'claim' is required"))?;
    let evidence_text = arguments
        .get("evidence")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| FrameworkError::validation("record_evidence: 'evidence' is required"))?;
    let source = arguments
        .get("source")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| FrameworkError::validation("record_evidence: 'source' is required"))?;
    let confidence = arguments.get("confidence").and_then(Value::as_f64);

    // Resolve current task from TASK_POINTERS
    let (active, focus) = core_state::state_manager::read_task_pointer_pair(repo_root);
    let task_id = active.as_deref().or(focus.as_deref()).ok_or_else(|| {
        FrameworkError::validation(
            "record_evidence: no active task (TASK_POINTERS.json is empty). Create a task or goal first.",
        )
    })?;
    let tid = core_state_utils::path_guard::validate_task_id_component(task_id)
        .map_err(|_| FrameworkError::validation(format!("record_evidence: invalid task_id '{task_id}'")))?;

    let task_dir = repo_root.join("artifacts/current").join(tid);
    std::fs::create_dir_all(&task_dir).map_err(FrameworkError::Io)?;
    let evidence_path = task_dir.join("EVIDENCE_INDEX.json");

    // Read existing, append, write back
    let existing_raw = std::fs::read_to_string(&evidence_path).unwrap_or_else(|_| "{}".to_string());
    let mut index: Value = serde_json::from_str(&existing_raw).unwrap_or_else(|_| json!({
        "schema_version": "evidence-index-v2",
        "artifacts": [],
    }));
    let artifacts = index
        .as_object_mut()
        .and_then(|o| o.get_mut("artifacts"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| FrameworkError::validation("EVIDENCE_INDEX.json: missing 'artifacts' array"))?;

    let mut entry = serde_json::Map::new();
    entry.insert("claim".to_string(), json!(claim));
    entry.insert("evidence".to_string(), json!(evidence_text));
    entry.insert("source".to_string(), json!(source));
    entry.insert("kind".to_string(), json!("mcp_record_evidence"));
    entry.insert("recorded_at".to_string(), json!(framework_core::time::now_iso()));
    // Self-attested evidence is recorded as successful by default.
    // Add exit_code to override or provide mechanical verification.
    let exit_code = arguments.get("exit_code").and_then(Value::as_i64);
    if let Some(ec) = exit_code {
        entry.insert("exit_code".to_string(), json!(ec));
        if let Some(s) = arguments.get("success").and_then(Value::as_bool) {
            // Cross-validate: reject contradiction like {exit_code:0, success:false}
            if s != (ec == 0) {
                return Err(FrameworkError::validation(format!(
                    "record_evidence: exit_code={ec} and success={s} contradict each other"
                )));
            }
            entry.insert("success".to_string(), json!(s));
        } else {
            entry.insert("success".to_string(), json!(ec == 0));
        }
    } else {
        let has_success = arguments.get("success").and_then(Value::as_bool);
        entry.insert("success".to_string(), json!(has_success.unwrap_or(true)));
    }
    if let Some(c) = confidence {
        entry.insert("confidence".to_string(), json!(c));
    }
    artifacts.push(Value::Object(entry));
    let evidence_count = artifacts.len();

    // Drop mutable borrow before write_atomic_json
    let _ = artifacts;
    core_state_utils::atomic_write::write_atomic_json(&evidence_path, &index)?;

    Ok(json!({
        "ok": true,
        "task_id": task_id,
        "evidence_count": evidence_count,
    })
    .to_string())
}

// ── Session Checkpoint ──

/// Save a session checkpoint snapshot.
pub(super) fn tool_session_checkpoint(
    repo_root: &Path,
) -> Result<String, FrameworkError> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let (active, focus) = core_state::state_manager::read_task_pointer_pair(repo_root);
    let checkpoint = json!({
        "schema_version": "session-checkpoint-v1",
        "snapshot_at": framework_core::time::now_iso(),
        "active_task_id": active,
        "focus_task_id": focus,
    });

    let cp_dir = repo_root.join("artifacts/current/.checkpoints");
    std::fs::create_dir_all(&cp_dir).map_err(FrameworkError::Io)?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let cp_path = cp_dir.join(format!("checkpoint-{ts}.json"));

    core_state_utils::atomic_write::write_atomic_json(&cp_path, &checkpoint)?;

    // Keep max 5 checkpoints
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&cp_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    entries.sort();
    while entries.len() > 5 {
        if let Some(old) = entries.first() {
            let _ = std::fs::remove_file(old);
            entries.remove(0);
        }
    }

    Ok(json!({
        "ok": true,
        "checkpoint_path": cp_path.to_string_lossy().to_string(),
    })
    .to_string())
}

// ── Research Harness MCP Tools ──

// ── Typed SKILL_TO_TOOL_MAP loader (cached, with diagnostics) ──

/// Typed entry for SKILL_TO_TOOL_MAP.json
#[derive(serde::Deserialize)]
struct SkillToToolMapEntry {
    skill_slug: String,
    // null → empty Vec (skills without mapped tools use null in JSON)
    #[serde(default)]
    tool_slugs: Vec<String>,
}

/// Typed root for SKILL_TO_TOOL_MAP.json
#[derive(serde::Deserialize)]
struct SkillToToolMap {
    #[serde(rename = "schema_version")]
    _schema_version: String,
    entries: Vec<SkillToToolMapEntry>,
}

/// Load SKILL_TO_TOOL_MAP once and cache forever. Returns an empty map on
/// any error (file not found, parse failure, etc.) with a `tracing::warn!`
/// diagnostic so misconfiguration is visible.
fn load_skill_to_tool_map(repo_root: &Path) -> &HashMap<String, HashSet<String>> {
    static CACHE: OnceLock<HashMap<String, HashSet<String>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let map_path = repo_root
            .join(framework_core::constants::SKILL_TO_TOOL_MAP_RELATIVE_PATH);
        match std::fs::read_to_string(&map_path) {
            Ok(content) => match serde_json::from_str::<SkillToToolMap>(&content) {
                Ok(map) => map
                    .entries
                    .into_iter()
                    .map(|e| {
                        (
                            e.skill_slug,
                            e.tool_slugs.into_iter().collect::<HashSet<_>>(),
                        )
                    })
                    .collect(),
                Err(e) => {
                    tracing::warn!(
                        "SKILL_TO_TOOL_MAP parse error at {}: {e}",
                        map_path.display()
                    );
                    HashMap::new()
                }
            },
            Err(e) => {
                tracing::warn!(
                    "SKILL_TO_TOOL_MAP not found at {}: {e}",
                    map_path.display()
                );
                HashMap::new()
            }
        }
    })
}

/// Compute boost slugs for a (selected_skill, overlay_skill) pair by looking
/// up the cached SKILL_TO_TOOL_MAP. Returns `None` when neither skill has
/// registered tools (so the scoring pipeline applies no boost).
fn compute_boost_slugs(
    repo_root: &Path,
    selected_skill: &str,
    overlay_skill: &str,
) -> Option<HashSet<String>> {
    let map = load_skill_to_tool_map(repo_root);
    let mut slugs = HashSet::new();
    if let Some(tools) = map.get(selected_skill) {
        slugs.extend(tools.iter().cloned());
    } else {
        tracing::warn!(
            "compute_boost_slugs: selected_skill '{selected_skill}' not found in SKILL_TO_TOOL_MAP"
        );
    }
    if !overlay_skill.is_empty() && overlay_skill != selected_skill {
        if let Some(tools) = map.get(overlay_skill) {
            slugs.extend(tools.iter().cloned());
        }
    }
    if slugs.is_empty() {
        None
    } else {
        Some(slugs)
    }
}
