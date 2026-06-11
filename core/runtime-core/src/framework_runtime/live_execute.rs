//! Live execute HTTP path, prompt builder, and aggregator URL validation.
//! Roadmap v5 P7：自 `cli/runtime_ops.inc` 下沉至 B3。

use serde_json::{json, Value};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;
use std::time::Duration;

use crate::execution_contract::{
    build_steady_state_execution_kernel_metadata, EXECUTION_AUTHORITY, EXECUTION_MODEL_ID_SOURCE,
    EXECUTION_RESPONSE_SHAPE_DRY_RUN, EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY,
    EXECUTION_SCHEMA_VERSION,
};
use crate::stdio_payload_types::{
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

pub fn execute_request(payload: ExecuteRequestPayload) -> Result<ExecuteResponsePayload, String> {
    let prompt_preview = payload
        .prompt_preview
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    if payload.dry_run {
        let dry_run_prompt_preview =
            Some(prompt_preview.unwrap_or_else(|| build_live_execute_prompt(&payload)));
        return Ok(build_dry_run_execute_response(
            &payload,
            dry_run_prompt_preview,
        ));
    }
    let live_prompt_preview = build_live_execute_prompt(&payload);
    if payload.aggregator_base_url.trim().is_empty() {
        return Err("router-rs execute requires a non-empty aggregator_base_url".to_string());
    }
    if payload.aggregator_api_key.trim().is_empty() {
        return Err("router-rs execute requires a non-empty aggregator_api_key".to_string());
    }
    let live_result = perform_live_execute(&payload, &live_prompt_preview)?;
    Ok(build_live_execute_response(
        &payload,
        Some(live_prompt_preview),
        live_result,
    ))
}



pub fn build_live_execute_prompt(payload: &ExecuteRequestPayload) -> String {
    let research_mode = infer_research_mode(payload);
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
    if research_mode == ResearchMode::Quick {
        lines.push("- Keep the default reply short; only use a list when the content is naturally list-shaped.".to_string());
    } else {
        lines.push("- Use a deep-research structure with explicit sections: Key findings, Evidence, Counter-evidence, Confidence, Open risks.".to_string());
    }
    lines.push("- For closeouts, say what was done, what effect was achieved, and what needs to happen next or that the work is finished.".to_string());
    if research_mode == ResearchMode::Quick {
        lines.push("- Do not default to file inventories, evidence dumps, or step-by-step process retellings unless the user asks for them.".to_string());
    } else {
        lines.push("- For each major claim, include at least two independent evidence anchors and one uncertainty note when evidence is incomplete.".to_string());
        lines.push("- If verification_required or evidence_required is true, treat missing evidence as an explicit blocker instead of silently concluding.".to_string());
        lines.push("- Auditable multi-round external research belongs in ledger `RFV_LOOP_STATE.json` via stdio op `framework_rfv_loop`; read `docs/rfv_loop_harness.md` / `docs/references/rfv-loop/external-research-harness.md`; hooks never auto-create that ledger.".to_string());
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
    lines.push(format!("Execution mode: {}.", research_mode.as_str()));
    lines.join("\n")
}

fn build_dry_run_execute_response(
    payload: &ExecuteRequestPayload,
    prompt_preview: Option<String>,
) -> ExecuteResponsePayload {
    let prompt = prompt_preview.clone().unwrap_or_default();
    let input_tokens = estimate_tokens(&format!("{}\n{}", payload.task, prompt));
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
        prompt_preview,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResearchMode {
    Quick,
    Deep,
}

impl ResearchMode {
    fn as_str(self) -> &'static str {
        match self {
            ResearchMode::Quick => "quick",
            ResearchMode::Deep => "deep",
        }
    }
}

/// `external research` alone matches many integration/API strings; require a second research cue.
fn external_research_phrase_signals_deep(lower: &str) -> bool {
    if !lower.contains("external research") {
        return false;
    }
    lower.contains("调研")
        || lower.contains("文献")
        || lower.contains("审计")
        || lower.contains("ledger")
        || lower.contains("rfv")
        || lower.contains("外研")
        || lower.contains("literature")
        || lower.contains("unknowns")
        || lower.contains("contradiction")
        || lower.contains("auditable")
        || lower.contains("research-grade")
        || lower.contains("research grade")
        || lower.contains("科研级")
        || lower.contains("deep dive")
}

/// Narrow host-neutral cues for Execute deep shaping (substring match; ASCII segments may be lowercased).
pub fn payload_text_signals_deep_research(text: &str) -> bool {
    text.contains("深度调研")
        || text.contains("深度研究")
        || text.contains("deep research")
        || text.contains("deep dive")
        || text.contains("literature review")
        || text.contains("literature-review")
        || text.contains("文献调研")
        || external_research_phrase_signals_deep(text)
        || text.contains("research-grade")
        || text.contains("research grade")
        || text.contains("科研级调研")
}

fn normalize_research_mode_token(value: &str) -> Option<ResearchMode> {
    let lowered = value.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return None;
    }
    match lowered.as_str() {
        "quick" | "fast" | "lite" | "shallow" | "autopilot-quick" => Some(ResearchMode::Quick),
        "deep" | "deep_research" | "deep-research" | "autopilot-deep" => Some(ResearchMode::Deep),
        _ => None,
    }
}

fn infer_research_mode(payload: &ExecuteRequestPayload) -> ResearchMode {
    if let Some(mode) = payload
        .research_mode
        .as_deref()
        .and_then(normalize_research_mode_token)
    {
        return mode;
    }
    if let Some(mode) = payload
        .execution_protocol
        .as_deref()
        .and_then(normalize_research_mode_token)
    {
        return mode;
    }
    let task = payload.task.trim().to_ascii_lowercase();
    if payload_text_signals_deep_research(&task) {
        return ResearchMode::Deep;
    }
    if task.contains("快查")
        || task.contains("快速调研")
    {
        return ResearchMode::Quick;
    }
    for reason in &payload.reasons {
        if let Some(mode) = normalize_research_mode_token(reason) {
            return mode;
        }
        let lowered = reason.to_ascii_lowercase();
        if payload_text_signals_deep_research(&lowered) {
            return ResearchMode::Deep;
        }
    }
    if payload.selected_skill == "implementx" {
        return ResearchMode::Quick;
    }
    ResearchMode::Quick
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
    let research_mode = infer_research_mode(payload);
    let mut max_tokens = payload.default_output_tokens;
    if research_mode == ResearchMode::Deep {
        max_tokens = max_tokens.max(1200);
    }
    let request_body = serde_json::json!({
        "model": payload.model_id,
        "messages": messages,
        "max_tokens": max_tokens,
    });
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
                    if attempt == 0 {
                        continue;
                    }
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
    let first_usage = response_payload
        .get("usage")
        .and_then(Value::as_object)
        .cloned();
    let mut usage = first_usage.clone();
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
    if research_mode == ResearchMode::Deep && finish_reason.as_deref() == Some("length") {
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
                                    usage = merge_usage_totals(
                                        first_usage.as_ref(),
                                        continuation_payload
                                            .get("usage")
                                            .and_then(Value::as_object),
                                    );
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
    let input_tokens = usage
        .as_ref()
        .and_then(|usage| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(&content) as u64) as usize;
    let output_tokens = usage
        .as_ref()
        .and_then(|usage| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(&content) as u64) as usize;
    let total_tokens = usage
        .as_ref()
        .and_then(|usage| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or((input_tokens + output_tokens) as u64) as usize;
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
) -> Result<LiveExecuteResult, String> {
    validate_live_execute_aggregator_base_url(&payload.aggregator_base_url)?;
    let endpoint = normalize_chat_completions_endpoint(&payload.aggregator_base_url);
    let client = live_execute_http_client()?;
    perform_live_execute_with_sender(payload, prompt_preview, |request_body| {
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
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
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
) -> ExecuteResponsePayload {
    let mut metadata =
        build_steady_state_execution_kernel_metadata(EXECUTION_RESPONSE_SHAPE_LIVE_PRIMARY);
    metadata.insert("run_id".to_string(), json!(live_result.run_id));
    metadata.insert("status".to_string(), json!(live_result.status));
    metadata.insert(
        "research_mode".to_string(),
        Value::String(infer_research_mode(payload).as_str().to_string()),
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
