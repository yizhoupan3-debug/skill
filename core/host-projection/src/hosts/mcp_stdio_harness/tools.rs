//! MCP tool implementations (tool_* functions).
//!
//! Extracted from mcp_stdio_harness.rs to keep file size ≤2000 lines.

use super::*;
use framework_kernel::skill_repo::skill_routing_runtime_json;
use serde_json::{Map, Value, json};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// In-memory fallback for `first_turn` detection.
/// Once `skill_route` succeeds once, this is set to `true`.
static SKILL_ROUTE_EVER_CALLED: AtomicBool = AtomicBool::new(false);

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

    let result = dispatch_tool(tool_name, arguments, repo_root, host_id, connection_session_id);

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
) -> Result<String, String> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .ok_or("Missing required argument: query")?;

    // Dynamically determine first_turn: true only if no routing tools have been called yet.
    // This prevents stale routing behavior on subsequent calls within the same session.
    let first_turn = !SKILL_ROUTE_EVER_CALLED.load(Ordering::Acquire);

    let route_result = crate::hooks::mcp_tool_skill_route(
        query,
        host_id,
        first_turn,
        &repo_root.to_string_lossy(),
    )?;

    // On success, mark that we've completed a route call.
    // This ensures that if the tracker fails later, the in-memory fallback
    // correctly reports first_turn=false.
    SKILL_ROUTE_EVER_CALLED.store(true, Ordering::Release);

    Ok(route_result)
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
    if !runtime_path.is_file() {
        return Err(format!(
            "Missing repository skill runtime ({})",
            runtime_path.display()
        ));
    }
    Ok(crate::hooks::mcp_tool_search_skills(query, limit, effective_host, &repo_root.to_string_lossy())?)
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

fn skill_runtime_available(repo_root: &Path) -> bool {
    skill_routing_runtime_json(repo_root).is_file() && repo_root.join("skills").is_dir()
}

fn skill_body_path(repo_root: &Path, slug: &str) -> Result<PathBuf, String> {
    let clean = slug.trim();
    if clean.is_empty()
        || clean.contains('/')
        || clean.contains('\\')
        || clean.contains("..")
        || clean.contains('\0')
        || clean.starts_with('.')
    {
        return Err(format!("invalid skill slug: {slug}"));
    }

    let path = repo_root.join("skills").join(clean).join("SKILL.md");
    finalize_skill_path(repo_root, &path, slug)
}

/// Validate the resolved skill path stays within the repo root.
/// Returns the path on success.
fn finalize_skill_path(repo_root: &Path, path: &Path, slug: &str) -> Result<PathBuf, String> {
    use core_state_utils::path_guard::{path_is_within_repo_root, reject_unsafe_path};

    reject_unsafe_path(path)?;
    if !path.is_file() {
        return Err(format!("skill body not found: {}", path.display()));
    }
    if !path_is_within_repo_root(repo_root, path) {
        return Err(format!(
            "skill path for {slug} escapes repo root: {}",
            path.display()
        ));
    }
    Ok(path.to_path_buf())
}

pub(super) fn tool_skill_route_status(repo_root: &Path) -> Result<String, String> {
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


    let exit_display = exit_code
        .map(|ec| ec.to_string())
        .unwrap_or_else(|| "null".to_string());
    let honor_note = " (honor-system: not bound to host tool execution — verify independently)";
    Ok(json!({"result": format!(
        "Evidence recorded{honor_note}: {tool_name_display} '{command_display}' -> exit={exit_display}"
    )}).to_string())
}

/// 获取 evidence output 的最大字符数配置。
/// 默认 2000 字符，可通过 `ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS` 环境变量覆盖。
pub(super) fn evidence_output_max_chars() -> usize {
    env_cache_typed!(usize, "ROUTER_RS_EVIDENCE_OUTPUT_MAX_CHARS", 2000)
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


    Ok(json!({"result": format!(
        "Checkpoint written: summary={}, next_actions_count={}",
        summary.chars().count(),
        next_actions.len()
    )}).to_string())
}

pub(super) fn task_lifecycle_profile(task_view: &core_state::task_state::ResolvedTaskView) -> &str {
    task_view
        .goal_state
        .as_ref()
        .and_then(|g| g.get("lifecycle_profile"))
        .and_then(Value::as_str)
        .unwrap_or("task")
}

pub fn tool_closeout_gate(
    arguments: &Value,
    repo_root: &Path,
    host_id: &str,
) -> Result<String, String> {
    Ok(crate::hooks::tool_closeout_gate_evaluate(arguments, repo_root, host_id)?)
}

pub(super) fn tool_closeout_record_write(
    arguments: &Value,
    repo_root: &Path,
    _host_id: &str,
) -> Result<String, String> {
    Ok(crate::hooks::tool_closeout_record_write_dispatch(arguments, repo_root)?)
}

pub(super) fn tool_goal_state_read(arguments: &Value, repo_root: &Path) -> Result<String, String> {
    let task_id = arguments.get("task_id").and_then(Value::as_str);
    let state = core_state::state_manager::read_goal_state(repo_root, task_id)
        .map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&state).map_err(|e| e.to_string())
}

pub(super) fn tool_goal_state_manage(
    arguments: &Value,
    repo_root: &Path,
    connection_session_id: &str,
) -> Result<String, String> {
    let result = crate::hooks::tool_goal_state_manage_dispatch(arguments, repo_root, connection_session_id)?;
    Ok(result)
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
    if initial_addrs.is_empty() && !initial_addr_strs.is_empty() {
        return Err(format!(
            "web_fetch: DNS resolution returned unparseable addresses for '{}': {initial_addr_strs:?}",
            parsed_url.host_str().unwrap_or("?")
        ));
    }
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
            if rp_addrs.is_empty() {
                return Err(format!(
                    "web_fetch: redirect DNS resolution returned unparseable addresses for '{rp_host}': \
                     redirect addresses could not be parsed"
                ));
            }
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

// ── Research Harness MCP Tools ──

