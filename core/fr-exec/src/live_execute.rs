//! Live execute HTTP path, prompt builder, and aggregator URL validation.

use serde_json::{Value, json};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;
use std::time::Duration;

use fr_contracts::execution_contract::{
    EXECUTION_AUTHORITY, EXECUTION_MODEL_ID_SOURCE, EXECUTION_RESPONSE_SHAPE_DRY_RUN,
    EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY, EXECUTION_SCHEMA_VERSION,
    build_steady_state_execution_kernel_metadata,
};
use framework_kernel::stdio_payload_types::{
    ExecuteRequestPayload, ExecuteResponsePayload, ExecuteUsagePayload,
};

pub const EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV: &str =
    "ROUTER_RS_EXECUTE_AGGREGATOR_HOST_ALLOWLIST";

fn normalize_allowlisted_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn parse_execute_aggregator_host_allowlist() -> Result<Option<HashSet<String>>, String> {
    let raw_value = match std::env::var(EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!(
                "router-rs live execute allowlist env {EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV} is not valid UTF-8"
            ));
        }
    };

    let hosts = raw_value
        .split(',')
        .map(normalize_allowlisted_host)
        .filter(|entry| !entry.is_empty())
        .collect::<HashSet<_>>();

    if hosts.is_empty() {
        return Err(format!(
            "router-rs live execute allowlist env {EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV} is configured but empty"
        ));
    }

    for entry in &hosts {
        if entry.eq_ignore_ascii_case("localhost") {
            return Err(format!(
                "router-rs live execute allowlist env {EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV} only accepts domain entries (localhost is forbidden)"
            ));
        }
        if entry.parse::<IpAddr>().is_ok() {
            return Err(format!(
                "router-rs live execute allowlist env {EXECUTE_AGGREGATOR_HOST_ALLOWLIST_ENV} only accepts domain entries (IP literals are forbidden): {entry}"
            ));
        }
    }

    Ok(Some(hosts))
}

pub fn execute_request(
    payload: ExecuteRequestPayload,
    research_mode: &str,
) -> Result<ExecuteResponsePayload, String> {
    if payload.dry_run {
        return Ok(build_dry_run_execute_response(&payload));
    }
    let prompt_preview = build_live_execute_prompt(&payload, research_mode);
    if payload.aggregator_base_url.trim().is_empty() {
        return Err("router-rs execute requires a non-empty aggregator_base_url".to_string());
    }
    if payload.aggregator_api_key.trim().is_empty() {
        return Err("router-rs execute requires a non-empty aggregator_api_key".to_string());
    }
    let live_result = perform_live_execute(&payload, &prompt_preview, research_mode)?;
    Ok(build_live_execute_response(
        &payload,
        Some(prompt_preview),
        live_result,
        research_mode,
    ))
}

pub fn build_live_execute_prompt(payload: &ExecuteRequestPayload, research_mode: &str) -> String {
    let native_runtime = payload.selected_skill == "none";
    let mut lines = vec![
        "Help with the user's request directly. The route is already chosen, so stay on it."
            .to_string(),
    ];
    if native_runtime {
        lines.push(
            "Primary focus: native runtime instructions; no skill body was selected.".to_string(),
        );
    } else {
        lines.push(format!("Primary focus: {}", payload.selected_skill));
    }
    if let Some(overlay) = payload
        .overlay_skill
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("Extra guidance: {overlay}"));
    }
    lines.push("How to reply:".to_string());
    lines.push("- Lead with the answer or result.".to_string());
    lines.push(
        "- Use plain Chinese unless the user asks otherwise, and keep the wording natural."
            .to_string(),
    );
    if research_mode == "quick" {
        lines.push("- Keep the default reply short; only use a list when the content is naturally list-shaped.".to_string());
    } else {
        lines.push("- Use a deep-research structure with explicit sections: Key findings, Evidence, Counter-evidence, Confidence, Open risks.".to_string());
    }
    lines.push("- For closeouts, say what was done, what effect was achieved, and what needs to happen next or that the work is finished.".to_string());
    if research_mode == "quick" {
        lines.push("- Do not default to file inventories, evidence dumps, or step-by-step process retellings unless the user asks for them.".to_string());
    } else {
        lines.push("- For each major claim, include at least two independent evidence anchors and one uncertainty note when evidence is incomplete.".to_string());
        lines.push("- If verification_required or evidence_required is true, treat missing evidence as an explicit blocker instead of silently concluding.".to_string());
        lines.push("- Auditable multi-round external research belongs in ledger `RFV_LOOP_STATE.json` via stdio op `framework_quality_gate`; see `core/runtime-core/src/rfv_loop.rs`; hooks never auto-create that ledger.".to_string());
    }
    let prompt_reasons = payload
        .reasons
        .iter()
        .map(|reason| reason.trim())
        .filter(|reason| !reason.is_empty())
        .take(5)
        .collect::<Vec<_>>();
    if !prompt_reasons.is_empty() {
        lines.push("Task cues:".to_string());
        for reason in prompt_reasons {
            lines.push(format!("- {reason}"));
        }
    }
    if native_runtime {
        lines.push(
            "No skill body is required; solve the user's actual task with the native runtime instructions already in context."
                .to_string(),
        );
    } else {
        lines.push("Use the selected skill to solve the user's actual task.".to_string());
    }
    lines.push(format!("Execution mode: {research_mode}."));
    lines.join("\n")
}

fn build_dry_run_execute_response(
    payload: &ExecuteRequestPayload,
) -> ExecuteResponsePayload {
    let trimmed = payload.task.trim();
    let input_tokens = if trimmed.is_empty() { 0 } else { trimmed.chars().count().div_ceil(4) };
    let output_tokens = payload.default_output_tokens.min(96);
    let content = format!(
        "[dry-run] Routed to `{}` on {}. Session `{}` is ready for Rust-owned execution.",
        payload.selected_skill, payload.layer, payload.session_id
    );
    let mut metadata =
        build_steady_state_execution_kernel_metadata(EXECUTION_RESPONSE_SHAPE_DRY_RUN);
    metadata.insert(
        "reason".to_string(),
        Value::String("router-rs returned a deterministic dry-run payload.".to_string()),
    );
    metadata.insert(
        "trace_event_count".to_string(),
        json!(payload.trace_event_count),
    );
    metadata.insert(
        "trace_output_path".to_string(),
        json!(payload.trace_output_path),
    );
    metadata.insert(
        "execution_mode".to_string(),
        Value::String("dry_run".to_string()),
    );
    metadata.insert("route_engine".to_string(), json!(payload.route_engine));
    metadata.insert(
        "diagnostic_route_mode".to_string(),
        json!(payload.diagnostic_route_mode),
    );
    ExecuteResponsePayload {
        execution_schema_version: EXECUTION_SCHEMA_VERSION.to_string(),
        authority: EXECUTION_AUTHORITY.to_string(),
        session_id: payload.session_id.clone(),
        user_id: payload.user_id.clone(),
        skill: payload.selected_skill.clone(),
        overlay: payload.overlay_skill.clone(),
        live_run: false,
        content,
        usage: ExecuteUsagePayload {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
            mode: "estimated".to_string(),
        },
        prompt_preview: None,
        model_id: None,
        metadata: Value::Object(metadata),
    }
}

#[derive(Debug)]
pub struct LiveExecuteResult {
    pub content: String,
    pub model_id: Option<String>,
    pub run_id: Option<String>,
    pub status: Option<String>,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub finish_reason: Option<String>,
    pub continuation_attempted: bool,
    pub continuation_status: Option<String>,
    pub continuation_error: Option<String>,
}

fn usage_total(usage: Option<&serde_json::Map<String, Value>>, key: &str) -> u64 {
    usage
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn merge_usage_totals(
    base: Option<&serde_json::Map<String, Value>>,
    extra: Option<&serde_json::Map<String, Value>>,
) -> Option<serde_json::Map<String, Value>> {
    if base.is_none() && extra.is_none() {
        return None;
    }
    let mut merged = base.cloned().unwrap_or_default();
    merged.insert(
        "prompt_tokens".to_string(),
        json!(usage_total(base, "prompt_tokens") + usage_total(extra, "prompt_tokens")),
    );
    merged.insert(
        "completion_tokens".to_string(),
        json!(usage_total(base, "completion_tokens") + usage_total(extra, "completion_tokens")),
    );
    merged.insert(
        "total_tokens".to_string(),
        json!(usage_total(base, "total_tokens") + usage_total(extra, "total_tokens")),
    );
    Some(merged)
}

pub const DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS: usize = 1600;
const DEEP_CONTINUATION_ANCHOR_CHARS: usize = 200;

fn build_compact_anchor(raw: &str, max_chars: usize) -> String {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return "(empty)".to_string();
    }
    let count = compact.chars().count();
    if count <= max_chars {
        compact
    } else {
        format!(
            "{}...",
            compact
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn build_assistant_tail_window(raw: &str, max_chars: usize) -> String {
    let total = raw.chars().count();
    if total <= max_chars {
        return raw.to_string();
    }
    let omitted = total.saturating_sub(max_chars);
    let tail = raw.chars().skip(omitted).collect::<String>();
    format!("[...omitted {omitted} chars...]\n{tail}")
}

pub fn perform_live_execute_with_sender<F>(
    payload: &ExecuteRequestPayload,
    prompt_preview: &str,
    research_mode: &str,
    mut send_request: F,
) -> Result<LiveExecuteResult, String>
where
    F: FnMut(&Value) -> Result<(u16, String), String>,
{
    let mut messages = Vec::new();
    if !prompt_preview.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": prompt_preview,
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": payload.task,
    }));
    let mut max_tokens = payload.default_output_tokens;
    if research_mode == "deep" {
        max_tokens = max_tokens.max(1200);
    }
    let request_body = serde_json::json!({
        "model": payload.model_id,
        "messages": messages,
        "max_tokens": max_tokens,
    });
    use std::time::Duration;
    let mut response_payload = Value::Null;
    let mut last_error = "router-rs live execute request failed".to_string();
    for attempt in 0..=1usize {
        match send_request(&request_body) {
            Ok((status_code, response_body)) => {
                if !(200..300).contains(&status_code) {
                    last_error = format!(
                        "router-rs live execute returned HTTP {}: {}",
                        status_code,
                        truncate_for_error(&response_body)
                    );
                    if attempt == 0
                        && matches!(status_code, 429 | 500..=599)
                    {
                        // 瞬态服务端错误或限速：退避后重试一次
                        std::thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                    // 4xx 客户端错误（401/403/404 等）：直接返回，不重试
                    return Err(last_error);
                }
                response_payload =
                    serde_json::from_str::<Value>(&response_body).map_err(|err| {
                        format!("parse router-rs live execute response failed: {err}")
                    })?;
                break;
            }
            Err(err) => {
                last_error = format!("router-rs live execute request failed: {err}");
                if attempt == 0 {
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                return Err(last_error);
            }
        }
    }
    if response_payload.is_null() {
        return Err(last_error);
    }
    let mut content = extract_chat_completion_content(&response_payload)?;
    let first_usage_ref = response_payload
        .get("usage")
        .and_then(Value::as_object);
    let mut finish_reason = response_payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let mut continuation_attempted = false;
    let mut continuation_status = None;
    let mut continuation_error = None;
    let mut usage_merged: Option<serde_json::Map<String, Value>> = None;
    if research_mode == "deep" && finish_reason.as_deref() == Some("length") {
        continuation_attempted = true;
        let system_anchor = build_compact_anchor(prompt_preview, DEEP_CONTINUATION_ANCHOR_CHARS);
        let task_anchor = build_compact_anchor(&payload.task, DEEP_CONTINUATION_ANCHOR_CHARS);
        let assistant_tail =
            build_assistant_tail_window(&content, DEEP_CONTINUATION_ASSISTANT_TAIL_CHARS);
        let continuation_messages = vec![
            serde_json::json!({
                "role": "system",
                "content": format!(
                    "Deep continuation. Keep the same objective and style. System anchor: {system_anchor}. Task anchor: {task_anchor}."
                )
            }),
            serde_json::json!({"role": "assistant", "content": assistant_tail}),
            serde_json::json!({"role": "user", "content": "Continue exactly from the cutoff. Do not repeat prior text. Prioritize unresolved evidence gaps and open risks."}),
        ];
        let continuation_body = serde_json::json!({
            "model": payload.model_id,
            "messages": continuation_messages,
            "max_tokens": max_tokens,
        });
        match send_request(&continuation_body) {
            Ok((status_code, continuation_text)) => {
                if !(200..300).contains(&status_code) {
                    continuation_status = Some(format!("http_{status_code}"));
                    continuation_error = Some(format!(
                        "router-rs continuation returned HTTP {}: {}",
                        status_code,
                        truncate_for_error(&continuation_text)
                    ));
                } else {
                    match serde_json::from_str::<Value>(&continuation_text) {
                        Ok(continuation_payload) => {
                            match extract_chat_completion_content(&continuation_payload) {
                                Ok(continuation_content) => {
                                    if !continuation_content.trim().is_empty() {
                                        content = format!(
                                            "{}\n\n{}",
                                            content.trim_end(),
                                            continuation_content.trim_start()
                                        );
                                    }
                                    usage_merged = Some(merge_usage_totals(
                                        first_usage_ref,
                                        continuation_payload
                                            .get("usage")
                                            .and_then(Value::as_object),
                                    ).unwrap_or_default());
                                    finish_reason = continuation_payload
                                        .get("choices")
                                        .and_then(Value::as_array)
                                        .and_then(|choices| choices.first())
                                        .and_then(|choice| choice.get("finish_reason"))
                                        .and_then(Value::as_str)
                                        .map(|value| value.to_string())
                                        .or(finish_reason);
                                    continuation_status = Some("success".to_string());
                                }
                                Err(err) => {
                                    continuation_status = Some("content_error".to_string());
                                    continuation_error = Some(format!(
                                        "router-rs continuation content extraction failed: {err}"
                                    ));
                                }
                            }
                        }
                        Err(err) => {
                            continuation_status = Some("parse_error".to_string());
                            continuation_error = Some(format!(
                                "parse router-rs continuation response failed: {err}"
                            ));
                        }
                    }
                }
            }
            Err(err) => {
                continuation_status = Some("request_error".to_string());
                continuation_error = Some(format!(
                    "router-rs live execute continuation request failed: {err}"
                ));
            }
        }
    }
    let active_usage = usage_merged.as_ref().or(first_usage_ref);
    let input_tokens = active_usage
        .and_then(|usage| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(&content) as u64)
        .try_into()
        .unwrap_or(usize::MAX);
    let output_tokens = active_usage
        .and_then(|usage| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(&content) as u64)
        .try_into()
        .unwrap_or(usize::MAX);
    let total_tokens = active_usage
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or((input_tokens as u64).saturating_add(output_tokens as u64))
        .try_into()
        .unwrap_or(usize::MAX);
    Ok(LiveExecuteResult {
        content,
        model_id: response_payload
            .get("model")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        run_id: response_payload
            .get("id")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        status: finish_reason.clone(),
        input_tokens,
        output_tokens,
        total_tokens,
        finish_reason,
        continuation_attempted,
        continuation_status,
        continuation_error,
    })
}

pub fn perform_live_execute(
    payload: &ExecuteRequestPayload,
    prompt_preview: &str,
    research_mode: &str,
) -> Result<LiveExecuteResult, String> {
    validate_live_execute_aggregator_base_url(&payload.aggregator_base_url)?;
    let endpoint = normalize_chat_completions_endpoint(&payload.aggregator_base_url);
    let client = live_execute_http_client()?;
    perform_live_execute_with_sender(payload, prompt_preview, research_mode, |request_body| {
        let response = client
            .post(endpoint.clone())
            .bearer_auth(payload.aggregator_api_key.as_str())
            .json(request_body)
            .send()
            .map_err(|err| format!("router-rs live execute request failed: {err}"))?;
        let status = response.status().as_u16();
        let response_body = response
            .text()
            .map_err(|err| format!("read router-rs live execute response failed: {err}"))?;
        Ok((status, response_body))
    })
}

/// Returns a shared blocking HTTP client for live execute requests.
///
/// NOTE: This uses `reqwest::blocking::Client` which occupies a thread during I/O.
/// If this path is reached from a concurrent stdio loop, the blocking call will tie
/// up a tokio worker thread for up to 30 seconds. A future improvement would be to
/// use an async `reqwest::Client` and `tokio::task::spawn_blocking` to offload the
/// blocking I/O, or to gate this path so it only runs outside the stdio hot path.
pub fn live_execute_http_client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30));
    // Inherit proxy configuration from environment (cached at process level).
    if let Some(proxy_url) = http_util::cached_proxy_url()
        && let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
            builder = builder.proxy(proxy);
        }
    let client = builder
        .build()
        .map_err(|err| format!("build reqwest client failed: {err}"))?;
    let _ = CLIENT.set(client);
    CLIENT
        .get()
        .ok_or_else(|| "build reqwest client failed: client cache was not initialized".to_string())
}

pub fn build_live_execute_response(
    payload: &ExecuteRequestPayload,
    prompt_preview: Option<String>,
    live_result: LiveExecuteResult,
    research_mode: &str,
) -> ExecuteResponsePayload {
    let mut metadata =
        build_steady_state_execution_kernel_metadata(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY);
    metadata.insert("run_id".to_string(), json!(live_result.run_id));
    metadata.insert("status".to_string(), json!(live_result.status));
    metadata.insert(
        "research_mode".to_string(),
        Value::String(research_mode.to_string()),
    );
    metadata.insert(
        "finish_reason".to_string(),
        json!(live_result.finish_reason),
    );
    if live_result.continuation_attempted
        || live_result.continuation_status.is_some()
        || live_result.continuation_error.is_some()
    {
        metadata.insert(
            "continuation_attempted".to_string(),
            json!(live_result.continuation_attempted),
        );
        metadata.insert(
            "continuation_status".to_string(),
            json!(live_result.continuation_status),
        );
        metadata.insert(
            "continuation_error".to_string(),
            json!(live_result.continuation_error),
        );
    }
    metadata.insert(
        "trace_event_count".to_string(),
        json!(payload.trace_event_count),
    );
    metadata.insert(
        "trace_output_path".to_string(),
        json!(payload.trace_output_path),
    );
    metadata.insert(
        "execution_mode".to_string(),
        Value::String("live".to_string()),
    );
    metadata.insert("route_engine".to_string(), json!(payload.route_engine));
    metadata.insert(
        "diagnostic_route_mode".to_string(),
        json!(payload.diagnostic_route_mode),
    );
    metadata.insert(
        "execution_kernel_model_id_source".to_string(),
        Value::String(EXECUTION_MODEL_ID_SOURCE.to_string()),
    );
    ExecuteResponsePayload {
        execution_schema_version: EXECUTION_SCHEMA_VERSION.to_string(),
        authority: EXECUTION_AUTHORITY.to_string(),
        session_id: payload.session_id.clone(),
        user_id: payload.user_id.clone(),
        skill: payload.selected_skill.clone(),
        overlay: payload.overlay_skill.clone(),
        live_run: true,
        content: live_result.content,
        usage: ExecuteUsagePayload {
            input_tokens: live_result.input_tokens,
            output_tokens: live_result.output_tokens,
            total_tokens: live_result.total_tokens,
            mode: "live".to_string(),
        },
        prompt_preview,
        model_id: live_result.model_id.clone(),
        metadata: Value::Object(metadata),
    }
}

pub fn normalize_chat_completions_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

pub fn validate_live_execute_aggregator_base_url(base_url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(base_url).map_err(|err| {
        format!("router-rs live execute requires valid aggregator_base_url: {err}")
    })?;

    if parsed.scheme() != "https" {
        return Err(
            "router-rs live execute requires https aggregator_base_url (http is not allowed)"
                .to_string(),
        );
    }

    let host = parsed.host_str().ok_or_else(|| {
        "router-rs live execute requires aggregator_base_url with a host".to_string()
    })?;

    if host.eq_ignore_ascii_case("localhost") || host.eq_ignore_ascii_case("localhost.") {
        return Err("router-rs live execute blocks localhost aggregator_base_url".to_string());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_forbidden_live_execute_ip(&ip) {
            return Err(format!(
                "router-rs live execute blocks unsafe aggregator_base_url host IP: {host}"
            ));
        }
        return Err(
            "router-rs live execute requires domain-based aggregator_base_url (IP literals are not allowed)"
                .to_string(),
        );
    }

    if let Some(allowlisted_hosts) = parse_execute_aggregator_host_allowlist()? {
        let normalized_host = normalize_allowlisted_host(host);
        if !allowlisted_hosts.contains(&normalized_host) {
            return Err(format!(
                "router-rs live execute blocks aggregator_base_url host not in allowlist: {host}"
            ));
        }
    }

    Ok(())
}

fn is_forbidden_live_execute_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_forbidden_live_execute_ipv4(*ipv4),
        IpAddr::V6(ipv6) => is_forbidden_live_execute_ipv6(*ipv6),
    }
}

fn is_forbidden_live_execute_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.octets()[0] == 127
        || ip.is_unspecified()
}

fn is_forbidden_live_execute_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback() || ip.is_unspecified() || ip.is_unicast_link_local() || ip.is_unique_local()
}

fn truncate_for_error(raw: &str) -> String {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    // Single-pass truncation via char_indices() to avoid two full traversals.
    const MAX_CHARS: usize = 240;
    const TRUNCATE_AT: usize = 237;
    if compact.is_ascii() {
        // Fast path for ASCII: byte length == char count.
        if compact.len() <= MAX_CHARS {
            compact
        } else {
            compact[..TRUNCATE_AT].to_string() + "..."
        }
    } else if compact.chars().count() <= MAX_CHARS {
        compact
    } else if let Some((byte_idx, _)) = compact.char_indices().nth(TRUNCATE_AT) {
        compact[..byte_idx].to_string() + "..."
    } else {
        compact
    }
}

pub fn extract_chat_completion_content(payload: &Value) -> Result<String, String> {
    let message_content = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .ok_or_else(|| {
            "router-rs live execute response missing choices[0].message.content".to_string()
        })?;

    if let Some(content) = message_content.as_str() {
        return Ok(content.to_string());
    }

    if let Some(parts) = message_content.as_array() {
        let joined = parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
                    .or_else(|| {
                        part.get("content")
                            .and_then(Value::as_str)
                            .map(|value| value.to_string())
                    })
            })
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Ok(joined);
        }
    }

    Err("router-rs live execute response content had an unsupported shape".to_string())
}

fn estimate_tokens(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    trimmed.chars().count().div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── build_live_execute_prompt ──

    #[test]
    fn prompt_native_runtime_no_skill_body() {
        let p = ExecuteRequestPayload {
            schema_version: "1".into(), task: "hello".into(), session_id: "s1".into(),
            user_id: "u1".into(), selected_skill: "none".into(), overlay_skill: None,
            layer: "L3".into(), route_engine: None, diagnostic_route_mode: None,
            reasons: vec![], prompt_preview: None, dry_run: false,
            trace_event_count: 0, trace_output_path: None, default_output_tokens: 512,
            research_mode: None, execution_protocol: None,
            verification_required: None, evidence_required: None,
            model_id: "gpt-4".into(), aggregator_base_url: "".into(), aggregator_api_key: "".into(),
        };
        let prompt = build_live_execute_prompt(&p, "quick");
        assert!(prompt.contains("no skill body"), "native runtime hint");
        assert!(!prompt.contains("Primary focus: none"), "no 'none' label leak");
    }

    #[test]
    fn prompt_selected_skill_and_overlay() {
        let p = ExecuteRequestPayload {
            selected_skill: "pdf".into(), overlay_skill: Some("ocr".into()),
            ..base_payload()
        };
        let prompt = build_live_execute_prompt(&p, "deep");
        assert!(prompt.contains("Primary focus: pdf"));
        assert!(prompt.contains("Extra guidance: ocr"));
        assert!(prompt.contains("deep-research"), "deep mode structure");
    }

    #[test]
    fn prompt_quick_mode_says_short_reply() {
        let prompt = build_live_execute_prompt(&base_payload(), "quick");
        assert!(prompt.contains("short"), "quick mode hint");
        assert!(!prompt.contains("deep-research"), "no deep structure");
    }

    #[test]
    fn prompt_includes_reasons_up_to_five() {
        let p = ExecuteRequestPayload {
            reasons: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into(), "f".into()],
            ..base_payload()
        };
        let prompt = build_live_execute_prompt(&p, "quick");
        assert!(prompt.contains("Task cues:"));
        // Only the first 5 reasons should appear, "f" should not
        assert!(!prompt.contains("- f"), "reason 'f' should be truncated (6th)");
    }

    #[test]
    fn prompt_omits_cues_when_no_reasons() {
        let prompt = build_live_execute_prompt(&base_payload(), "quick");
        assert!(!prompt.contains("Task cues:"));
    }

    // ── normalize_chat_completions_endpoint ──

    #[test]
    fn endpoint_already_has_chat_completions() {
        assert_eq!(
            normalize_chat_completions_endpoint("https://api.example.com/chat/completions"),
            "https://api.example.com/chat/completions"
        );
    }

    #[test]
    fn endpoint_appends_chat_completions() {
        assert_eq!(
            normalize_chat_completions_endpoint("https://api.example.com"),
            "https://api.example.com/chat/completions"
        );
    }

    #[test]
    fn endpoint_strips_trailing_slash() {
        assert_eq!(
            normalize_chat_completions_endpoint("https://api.example.com/"),
            "https://api.example.com/chat/completions"
        );
    }

    // ── validate_live_execute_aggregator_base_url ──

    #[test]
    fn validate_url_accepts_valid_https_domain() {
        let result = validate_live_execute_aggregator_base_url("https://api.example.com");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_url_rejects_http() {
        let result = validate_live_execute_aggregator_base_url("http://api.example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("https"));
    }

    #[test]
    fn validate_url_rejects_localhost() {
        let result = validate_live_execute_aggregator_base_url("https://localhost:8080");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("localhost"));
    }

    #[test]
    fn validate_url_rejects_ip_literal() {
        let result = validate_live_execute_aggregator_base_url("https://1.2.3.4");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("IP"));
    }

    #[test]
    fn validate_url_rejects_private_ip() {
        let result = validate_live_execute_aggregator_base_url("https://10.0.0.1");
        assert!(result.is_err());
    }

    #[test]
    fn validate_url_rejects_invalid_url() {
        let result = validate_live_execute_aggregator_base_url("not-a-url");
        assert!(result.is_err());
    }

    // ── extract_chat_completion_content ──

    #[test]
    fn extract_standard_content_string() {
        let payload = json!({
            "choices": [{"message": {"content": "Hello world"}}]
        });
        assert_eq!(extract_chat_completion_content(&payload).unwrap(), "Hello world");
    }

    #[test]
    fn extract_content_array() {
        let payload = json!({
            "choices": [{"message": {"content": [{"text": "Hello "}, {"text": "world"}]}}]
        });
        assert_eq!(extract_chat_completion_content(&payload).unwrap(), "Hello world");
    }

    #[test]
    fn extract_missing_choice_returns_err() {
        let payload = json!({});
        assert!(extract_chat_completion_content(&payload).is_err());
    }

    #[test]
    fn extract_empty_choices_returns_err() {
        let payload = json!({"choices": []});
        assert!(extract_chat_completion_content(&payload).is_err());
    }

    #[test]
    fn extract_unsupported_shape_returns_err() {
        let payload = json!({"choices": [{"message": {"content": 42}}]});
        assert!(extract_chat_completion_content(&payload).is_err());
    }

    // ── execute_request (dry_run path) ──

    #[test]
    fn execute_request_dry_run_returns_estimated_tokens() {
        let p = ExecuteRequestPayload {
            dry_run: true, task: "hello world".into(), default_output_tokens: 64,
            ..base_payload()
        };
        let result = execute_request(p, "quick");
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert!(!resp.live_run);
        assert!(resp.content.contains("[dry-run]"));
        assert!(resp.content.contains("pdf"), "default skill rendered");
        assert_eq!(resp.usage.mode, "estimated");
        assert!(resp.usage.input_tokens > 0);
    }

    #[test]
    fn execute_request_dry_run_empty_task_has_zero_input_tokens() {
        let p = ExecuteRequestPayload {
            dry_run: true, task: "".into(), default_output_tokens: 64,
            ..base_payload()
        };
        let resp = execute_request(p, "quick").unwrap();
        assert_eq!(resp.usage.input_tokens, 0);
    }

    // ── build_live_execute_response ──

    #[test]
    fn build_response_without_continuation() {
        let payload = base_payload();
        let result = LiveExecuteResult {
            content: "done".into(), model_id: Some("gpt-4".into()),
            run_id: Some("r1".into()), status: Some("stop".into()),
            input_tokens: 10, output_tokens: 20, total_tokens: 30,
            finish_reason: Some("stop".into()),
            continuation_attempted: false, continuation_status: None, continuation_error: None,
        };
        let resp = build_live_execute_response(&payload, None, result, "quick");
        assert!(resp.live_run);
        assert_eq!(resp.content, "done");
        assert_eq!(resp.model_id.as_deref(), Some("gpt-4"));
        // Continuation fields should not appear in metadata
        let md = resp.metadata.as_object().unwrap();
        assert!(md.get("continuation_attempted").is_none(), "skip when absent");
    }

    #[test]
    fn build_response_with_continuation() {
        let payload = base_payload();
        let result = LiveExecuteResult {
            content: "deep result".into(), model_id: None,
            run_id: Some("r2".into()), status: Some("length".into()),
            input_tokens: 100, output_tokens: 200, total_tokens: 300,
            finish_reason: Some("length".into()),
            continuation_attempted: true, continuation_status: Some("success".into()),
            continuation_error: None,
        };
        let resp = build_live_execute_response(&payload, Some("preview".into()), result, "deep");
        assert!(resp.live_run);
        assert_eq!(resp.prompt_preview.as_deref(), Some("preview"));
        let md = resp.metadata.as_object().unwrap();
        assert_eq!(md.get("continuation_status").and_then(Value::as_str), Some("success"));
    }

    // ── perform_live_execute_with_sender (mock sender) ──

    #[test]
    fn live_execute_with_sender_success() {
        let p = ExecuteRequestPayload {
            task: "test task".into(), model_id: "gpt-4".into(),
            default_output_tokens: 512, ..base_payload()
        };
        let result = perform_live_execute_with_sender(
            &p, "prompt", "quick",
            |_| Ok((200, r#"{"choices":[{"message":{"content":"OK"}}],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}"#.into())),
        );
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.content, "OK");
        assert_eq!(r.input_tokens, 5);
        assert_eq!(r.output_tokens, 3);
    }

    #[test]
    fn live_execute_retries_on_500_then_succeeds() {
        let p = ExecuteRequestPayload {
            task: "retry test".into(), model_id: "gpt-4".into(),
            default_output_tokens: 512, ..base_payload()
        };
        let mut call_count = 0usize;
        let result = perform_live_execute_with_sender(
            &p, "prompt", "quick",
            |_| {
                call_count += 1;
                if call_count == 1 {
                    Ok((500, "server error".into()))
                } else {
                    Ok((200, r#"{"choices":[{"message":{"content":"recovered"}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#.into()))
                }
            },
        );
        assert!(result.is_ok());
        assert_eq!(call_count, 2, "should retry once");
    }

    #[test]
    fn live_execute_400_does_not_retry() {
        let mut call_count = 0usize;
        let p = ExecuteRequestPayload {
            task: "no retry".into(), model_id: "gpt-4".into(),
            default_output_tokens: 512, ..base_payload()
        };
        let result = perform_live_execute_with_sender(
            &p, "prompt", "quick",
            |_| {
                call_count += 1;
                Ok((401, "unauthorized".into()))
            },
        );
        assert!(result.is_err());
        assert_eq!(call_count, 1, "should NOT retry on 4xx");
    }

    #[test]
    fn live_execute_deep_continuation_on_length() {
        let p = ExecuteRequestPayload {
            task: "deep".into(), model_id: "gpt-4".into(),
            default_output_tokens: 1024, ..base_payload()
        };
        let result = perform_live_execute_with_sender(
            &p, "prompt", "deep",
            |body| {
                let is_continuation = body.get("messages").and_then(Value::as_array)
                    .map(|m| m.len() > 2).unwrap_or(false);
                if is_continuation {
                    Ok((200, r#"{"choices":[{"message":{"content":" continuation"}}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}}"#.into()))
                } else {
                    Ok((200, r#"{"choices":[{"message":{"content":"first half"},"finish_reason":"length"}]}"#.into()))
                }
            },
        );
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.continuation_attempted);
        assert_eq!(r.continuation_status.as_deref(), Some("success"));
        // Content should include both halves
        assert!(r.content.contains("first half"), "original content preserved");
        assert!(r.content.contains("continuation"), "continuation appended");
    }

    #[test]
    fn live_execute_deep_no_continuation_for_stop() {
        let p = ExecuteRequestPayload {
            task: "short".into(), model_id: "gpt-4".into(),
            default_output_tokens: 256, ..base_payload()
        };
        let result = perform_live_execute_with_sender(
            &p, "prompt", "deep",
            |_| Ok((200, r#"{"choices":[{"message":{"content":"done"}}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#.into())),
        );
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(!r.continuation_attempted, "no continuation when finish_reason != length");
    }

    // ── Shared helper ──

    fn base_payload() -> ExecuteRequestPayload {
        ExecuteRequestPayload {
            schema_version: "1".into(), task: "test".into(), session_id: "s1".into(),
            user_id: "u1".into(), selected_skill: "pdf".into(), overlay_skill: None,
            layer: "L3".into(), route_engine: None, diagnostic_route_mode: None,
            reasons: vec![], prompt_preview: None, dry_run: false,
            trace_event_count: 0, trace_output_path: None, default_output_tokens: 512,
            research_mode: None, execution_protocol: None,
            verification_required: None, evidence_required: None,
            model_id: "gpt-4".into(), aggregator_base_url: "".into(),
            aggregator_api_key: "".into(),
        }
    }
}
