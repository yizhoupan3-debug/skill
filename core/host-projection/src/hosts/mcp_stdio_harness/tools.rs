//! MCP tool implementations (tool_* functions).
//!
//! Extracted from mcp_stdio_harness.rs to keep file size ≤2000 lines.

use super::*;
use framework_core::skill_repo::skill_routing_runtime_json;
#[cfg(any(test, feature = "test-support"))]
use serde_json::Map;
use serde_json::{Value, json};
use std::path::Path;

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

    let route_result = crate::hooks::mcp_tool_skill_route(
        query,
        host_id,
        first_turn,
        &repo_root.to_string_lossy(),
    )?;

    // Enhance: append recommended_tools from tool routing
    let mut result_value: Value = serde_json::from_str(&route_result).unwrap_or(Value::Null);
    if let Some(obj) = result_value.as_object_mut() {
        let registry_path = mcp_tool_registry::resolve_tool_registry_path()
            .unwrap_or_else(|| {
                repo_root.join(framework_core::constants::MCP_TOOL_REGISTRY_RELATIVE_PATH)
            });

        let tools = (|| -> Option<Vec<Value>> {
            let records = mcp_tool_registry::load_tool_records_cached(&registry_path).ok()?;
            // Exclude no_routing tools (meta-tools) from recommendations
            let filtered: Vec<_> = records
                .into_iter()
                .filter(|r| !r.tool_flags.iter().any(|f| f == "no_routing"))
                .collect();
            if filtered.is_empty() {
                return None;
            }
            let results = tool_routing_engine::search::search_tools(query, &filtered, 3);
            if results.is_empty() {
                return None;
            }
            Some(
                results
                    .into_iter()
                    .map(|d| {
                        json!({
                            "slug": d.selected_tool,
                            "score": d.score,
                            "dispatch_domain": d.dispatch_domain,
                            "fuzzy_match": d.fuzzy_match,
                            "reasons": d.reasons,
                        })
                    })
                    .collect(),
            )
        })();

        if let Some(tools) = tools {
            obj.insert("recommended_tools".to_string(), json!(tools));
        }
    }

    Ok(result_value.to_string())
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

    let runtime_path = skill_routing_runtime_json(repo_root);
    if !runtime_path.is_file() {
        return Err(FrameworkError::from(format!(
            "Missing repository skill runtime ({})",
            runtime_path.display()
        )));
    }
    Ok(crate::hooks::mcp_tool_search_skills(
        query,
        limit,
        effective_host,
        &repo_root.to_string_lossy(),
    )?)
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
    Ok(serde_json::to_string_pretty(&state)
        .map_err(|e| FrameworkError::from(e.to_string()))?)
}

// ── Research Harness MCP Tools ──
