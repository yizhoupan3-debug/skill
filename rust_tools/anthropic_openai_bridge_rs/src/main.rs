use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use clap::{ArgAction, Parser};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::time;
use tokio_stream::wrappers::ReceiverStream;

type StreamEvent = (&'static str, Value);
const LEGACY_FUNCTION_ID_PREFIX: &str = "call_legacy_";
const LEGACY_FUNCTION_TOOL_INDEX: u64 = u64::MAX;

#[derive(Parser, Debug)]
#[command(name = "anthropic-openai-bridge-rs")]
#[command(about = "Anthropic Messages API bridge to an OpenAI-compatible chat backend")]
struct Cli {
    #[arg(long, env = "AOB_LISTEN", default_value = "127.0.0.1:8320")]
    listen: SocketAddr,
    #[arg(
        long,
        env = "AOB_UPSTREAM_BASE",
        default_value = "http://127.0.0.1:8318/v1"
    )]
    upstream_base: String,
    #[arg(long, env = "AOB_UPSTREAM_KEY", default_value = "sk-dummy")]
    upstream_key: String,
    #[arg(long, env = "AOB_MODEL", default_value = "gpt-5.5")]
    model: String,
    #[arg(long, env = "AOB_PRESERVE_REQUEST_MODEL", default_value_t = false)]
    preserve_request_model: bool,
    #[arg(long, env = "AOB_SYSTEM_ROLE", default_value = "developer")]
    system_role: String,
    #[arg(long, env = "AOB_REASONING_EFFORT")]
    reasoning_effort: Option<String>,
    #[arg(
        long,
        env = "AOB_STREAM_INCLUDE_USAGE",
        default_value_t = true,
        action = ArgAction::Set
    )]
    stream_include_usage: bool,
    #[arg(long, env = "AOB_STREAM_OBFUSCATION", default_value = "false")]
    stream_obfuscation: String,
    #[arg(long, env = "AOB_MAX_TOKENS_FIELD", default_value = "auto")]
    max_tokens_field: String,
    #[arg(long, env = "AOB_STREAM_HEARTBEAT_SECS", default_value_t = 5)]
    stream_heartbeat_secs: u64,
    #[arg(long, env = "AOB_MAX_REQUEST_BYTES", default_value_t = 64 * 1024 * 1024)]
    max_request_bytes: usize,
    #[arg(long, env = "AOB_UPSTREAM_CONNECT_TIMEOUT_SECS", default_value_t = 10)]
    upstream_connect_timeout_secs: u64,
    #[arg(long, env = "AOB_UPSTREAM_REQUEST_TIMEOUT_SECS", default_value_t = 300)]
    upstream_request_timeout_secs: u64,
    #[arg(
        long,
        env = "AOB_UPSTREAM_POOL_MAX_IDLE_PER_HOST",
        default_value_t = 128
    )]
    upstream_pool_max_idle_per_host: usize,
    #[arg(long, env = "AOB_STREAM_CHANNEL_DEPTH", default_value_t = 64)]
    stream_channel_depth: usize,
}

#[derive(Clone)]
struct AppState {
    client: Client,
    upstream_base: String,
    upstream_key: String,
    model: String,
    preserve_request_model: bool,
    system_role: String,
    reasoning_effort: Option<String>,
    stream_include_usage: bool,
    stream_obfuscation: StreamObfuscation,
    max_tokens_field: String,
    stream_heartbeat_secs: u64,
    max_request_bytes: usize,
    stream_channel_depth: usize,
    upstream_request_timeout_secs: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StreamObfuscation {
    Omit,
    Include(bool),
}

#[derive(Debug, Deserialize)]
struct AnthropicRequest {
    model: Option<String>,
    max_tokens: Option<u64>,
    messages: Vec<AnthropicMessage>,
    #[serde(default)]
    system: Option<Value>,
    #[serde(default)]
    tools: Option<Vec<Value>>,
    #[serde(default)]
    tool_choice: Option<Value>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    top_p: Option<f64>,
    #[serde(default)]
    stop_sequences: Option<Vec<String>>,
    #[serde(default)]
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Value,
}

#[derive(Debug, Serialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

#[derive(Debug, Serialize)]
struct AnthropicResponse {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    role: String,
    content: Vec<Value>,
    model: String,
    stop_reason: String,
    stop_sequence: Option<String>,
    usage: AnthropicUsage,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(cli.upstream_connect_timeout_secs))
        .pool_max_idle_per_host(cli.upstream_pool_max_idle_per_host)
        .tcp_nodelay(true)
        .build()
        .map_err(|err| format!("client build failed: {err}"))?;
    let max_request_bytes = cli.max_request_bytes.max(1024);
    let stream_channel_depth = cli.stream_channel_depth.clamp(1, 1024);
    let state = Arc::new(AppState {
        client,
        upstream_base: cli.upstream_base.trim_end_matches('/').to_string(),
        upstream_key: cli.upstream_key,
        model: cli.model,
        preserve_request_model: cli.preserve_request_model,
        system_role: cli.system_role,
        reasoning_effort: cli.reasoning_effort,
        stream_include_usage: cli.stream_include_usage,
        stream_obfuscation: parse_stream_obfuscation(&cli.stream_obfuscation)?,
        max_tokens_field: cli.max_tokens_field,
        stream_heartbeat_secs: cli.stream_heartbeat_secs,
        max_request_bytes,
        stream_channel_depth,
        upstream_request_timeout_secs: cli.upstream_request_timeout_secs,
    });
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(models))
        .route("/v1/messages", post(messages))
        .route("/v1/messages/count_tokens", post(count_tokens))
        .layer(DefaultBodyLimit::max(max_request_bytes))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(cli.listen)
        .await
        .map_err(|err| format!("bind failed: {err}"))?;
    axum::serve(listener, app)
        .await
        .map_err(|err| format!("server failed: {err}"))
}

async fn health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "bridge": "anthropic-openai-bridge-rs",
        "upstream_base": state.upstream_base,
        "model": state.model,
        "preserve_request_model": state.preserve_request_model,
        "system_role": state.system_role,
        "reasoning_effort": state.reasoning_effort,
        "stream_include_usage": state.stream_include_usage,
        "stream_obfuscation": stream_obfuscation_label(&state.stream_obfuscation),
        "max_tokens_field": resolve_max_tokens_field(&state.max_tokens_field, &state.model),
        "max_request_bytes": state.max_request_bytes,
        "stream_channel_depth": state.stream_channel_depth,
        "upstream_request_timeout_secs": state.upstream_request_timeout_secs,
        "sampling_parameters": if supports_sampling_parameters(&state.model) { "forwarded" } else { "dropped_for_reasoning_model" },
        "stream_heartbeat_secs": state.stream_heartbeat_secs,
        "loss_reduction": {
            "system": "top-level Anthropic system maps to a dedicated OpenAI message",
            "tools": "Anthropic tools map to OpenAI function tools",
            "streaming": "OpenAI SSE chunks are translated to Anthropic SSE chunks as they arrive",
            "request": "Claude-host-only fields are stripped instead of serialized into GPT context"
        }
    }))
}

async fn models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({
        "data": [{
            "type": "model",
            "id": state.model,
            "display_name": state.model,
            "created_at": "2026-04-24T00:00:00Z"
        }],
        "has_more": false,
        "first_id": state.model,
        "last_id": state.model
    }))
}

async fn count_tokens(
    State(state): State<Arc<AppState>>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    let estimated = match serde_json::from_value::<AnthropicRequest>(request.clone()) {
        Ok(request) => build_openai_request(&state, &request, false)
            .map(|mapped| estimate_openai_request_tokens(&mapped))
            .unwrap_or_else(|_| estimate_tokens(&request_to_lossy_text(&request))),
        Err(_) => estimate_tokens(&request.to_string()),
    };
    Json(json!({"input_tokens": estimated.max(1)}))
}

async fn messages(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<AnthropicRequest>,
) -> Response {
    match handle_messages(state, headers, request).await {
        Ok(response) => response,
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &err),
    }
}

async fn handle_messages(
    state: Arc<AppState>,
    headers: HeaderMap,
    request: AnthropicRequest,
) -> Result<Response, String> {
    let openai_body = build_openai_request(&state, &request, request.stream)?;
    let upstream = format!("{}/chat/completions", state.upstream_base);
    let mut upstream_request = state
        .client
        .post(upstream)
        .bearer_auth(resolve_upstream_key(&state, &headers))
        .json(&openai_body);
    if !request.stream && state.upstream_request_timeout_secs > 0 {
        upstream_request =
            upstream_request.timeout(Duration::from_secs(state.upstream_request_timeout_secs));
    }
    let response = upstream_request
        .send()
        .await
        .map_err(|err| format!("upstream request failed: {err}"))?;
    let status = response.status();
    if request.stream {
        if !status.is_success() {
            let text = response
                .text()
                .await
                .map_err(|err| format!("read upstream response failed: {err}"))?;
            return Ok(json_error(status, &text));
        }
        let upstream_model = openai_body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(&state.model)
            .to_string();
        let estimated_input_tokens = estimate_openai_request_tokens(&openai_body);
        return Ok(openai_stream_to_anthropic_response(
            response,
            upstream_model,
            estimated_input_tokens,
            state.stream_heartbeat_secs,
            state.stream_channel_depth,
        ));
    }
    let text = response
        .text()
        .await
        .map_err(|err| format!("read upstream response failed: {err}"))?;
    if !status.is_success() {
        return Ok(json_error(status, &text));
    }
    let openai_response: Value = serde_json::from_str(&text)
        .map_err(|err| format!("parse upstream response failed: {err}: {text}"))?;
    let upstream_model = openai_body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(&state.model)
        .to_string();
    let anthropic = openai_to_anthropic(&upstream_model, &openai_response);
    if request.stream {
        Ok(sse_response(&anthropic))
    } else {
        Ok(Json(anthropic).into_response())
    }
}

fn resolve_upstream_key(state: &AppState, headers: &HeaderMap) -> String {
    if state.upstream_key != "pass-through" {
        return state.upstream_key.clone();
    }
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string)
        .or_else(|| {
            headers
                .get("x-api-key")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "sk-dummy".to_string())
}

fn build_openai_request(
    state: &AppState,
    request: &AnthropicRequest,
    stream: bool,
) -> Result<Value, String> {
    let mut body = Map::new();
    let model = resolve_upstream_model(state, request);
    body.insert("model".to_string(), Value::String(model.clone()));
    body.insert(
        "messages".to_string(),
        Value::Array(build_openai_messages(state, request)?),
    );
    if let Some(max_tokens) = request.max_tokens {
        body.insert(
            resolve_max_tokens_field(&state.max_tokens_field, &model),
            Value::Number(max_tokens.into()),
        );
    }
    if supports_reasoning_effort(&model) {
        if let Some(effort) = &state.reasoning_effort {
            body.insert(
                "reasoning_effort".to_string(),
                Value::String(effort.to_string()),
            );
        }
    }
    if supports_sampling_parameters(&model) {
        if let Some(value) = request.temperature {
            insert_number(&mut body, "temperature", value);
        }
        if let Some(value) = request.top_p {
            insert_number(&mut body, "top_p", value);
        }
    }
    if let Some(stops) = &request.stop_sequences {
        body.insert("stop".to_string(), json!(stops));
    }
    if let Some(tools) = request.tools.as_ref().filter(|tools| !tools.is_empty()) {
        body.insert(
            "tools".to_string(),
            Value::Array(tools.iter().map(anthropic_tool_to_openai).collect()),
        );
        if let Some(choice) = &request.tool_choice {
            if let Some(mapped) = anthropic_tool_choice_to_openai(choice) {
                body.insert("tool_choice".to_string(), mapped);
            }
        }
        if anthropic_disable_parallel_tool_use(&request.tool_choice) {
            body.insert("parallel_tool_calls".to_string(), Value::Bool(false));
        }
    }
    body.insert("stream".to_string(), Value::Bool(stream));
    if stream && state.stream_include_usage {
        body.insert(
            "stream_options".to_string(),
            build_stream_options(&state.stream_obfuscation),
        );
    }
    Ok(Value::Object(body))
}

fn resolve_upstream_model(state: &AppState, request: &AnthropicRequest) -> String {
    if state.preserve_request_model {
        return request.model.clone().unwrap_or_else(|| state.model.clone());
    }
    state.model.clone()
}

fn resolve_max_tokens_field(configured: &str, model: &str) -> String {
    match configured.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => {
            if uses_max_completion_tokens(model) {
                "max_completion_tokens".to_string()
            } else {
                "max_tokens".to_string()
            }
        }
        value => value.to_string(),
    }
}

fn normalized_model_family(model: &str) -> String {
    model
        .trim()
        .trim_start_matches("openai/")
        .trim_start_matches("openai:")
        .trim_start_matches("models/")
        .to_ascii_lowercase()
}

fn uses_max_completion_tokens(model: &str) -> bool {
    let model = normalized_model_family(model);
    model.starts_with("gpt-5")
        || model.starts_with("o1")
        || model.starts_with("o3")
        || model.starts_with("o4")
}

fn supports_reasoning_effort(model: &str) -> bool {
    uses_max_completion_tokens(model)
}

fn supports_sampling_parameters(model: &str) -> bool {
    !uses_max_completion_tokens(model)
}

fn build_openai_messages(
    state: &AppState,
    request: &AnthropicRequest,
) -> Result<Vec<Value>, String> {
    let mut messages = Vec::new();
    if let Some(system) = &request.system {
        let system_text = content_value_to_text(system);
        if !system_text.trim().is_empty() {
            messages.push(json!({"role": state.system_role, "content": system_text}));
        }
    }
    for message in &request.messages {
        let role = message.role.as_str();
        match role {
            "user" => append_user_message(&mut messages, &message.content),
            "assistant" => append_assistant_message(&mut messages, &message.content),
            _ => return Err(format!("unsupported Anthropic role: {role}")),
        }
    }
    Ok(messages)
}

fn append_user_message(messages: &mut Vec<Value>, content: &Value) {
    if let Some(text) = user_content_as_plain_text(content) {
        if !text.trim().is_empty() {
            messages.push(json!({"role": "user", "content": text}));
        }
        return;
    }
    let blocks = content_blocks(content);
    let mut user_parts = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("tool_result") => {
                if !user_parts.is_empty() {
                    messages
                        .push(json!({"role": "user", "content": std::mem::take(&mut user_parts)}));
                }
                let text = content_value_to_text(block.get("content").unwrap_or(&Value::Null));
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": block.get("tool_use_id").and_then(Value::as_str).unwrap_or("toolu_unknown"),
                    "content": text,
                }));
            }
            Some("image") => {
                if let Some(source) = block.get("source") {
                    if let Some(url) = anthropic_image_source_to_url(source) {
                        user_parts.push(json!({"type": "image_url", "image_url": {"url": url}}));
                    }
                }
            }
            Some("thinking") | Some("redacted_thinking") => {}
            _ => {
                let text = content_value_to_text(&block);
                if !text.trim().is_empty() {
                    user_parts.push(json!({"type": "text", "text": text}));
                }
            }
        }
    }
    if !user_parts.is_empty() {
        messages.push(json!({"role": "user", "content": user_parts}));
    }
}

fn user_content_as_plain_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => Some(text.clone()),
        Value::Array(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                match block.get("type").and_then(Value::as_str) {
                    Some("tool_result") | Some("image") => return None,
                    Some("thinking") | Some("redacted_thinking") => {}
                    _ => {
                        let text = content_value_to_text(block);
                        if !text.trim().is_empty() {
                            parts.push(text);
                        }
                    }
                }
            }
            Some(parts.join("\n"))
        }
        Value::Object(map) => match map.get("type").and_then(Value::as_str) {
            Some("tool_result") | Some("image") => None,
            Some("thinking") | Some("redacted_thinking") => Some(String::new()),
            _ => Some(content_value_to_text(content)),
        },
        _ => Some(content_value_to_text(content)),
    }
}

fn append_assistant_message(messages: &mut Vec<Value>, content: &Value) {
    let blocks = content_blocks(content);
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("tool_use") => {
                tool_calls.push(json!({
                    "id": block.get("id").and_then(Value::as_str).unwrap_or("toolu_unknown"),
                    "type": "function",
                    "function": {
                        "name": block.get("name").and_then(Value::as_str).unwrap_or("unknown_tool"),
                        "arguments": block.get("input").cloned().unwrap_or_else(|| json!({})).to_string(),
                    }
                }));
            }
            Some("thinking") | Some("redacted_thinking") => {}
            _ => {
                let text = content_value_to_text(&block);
                if !text.trim().is_empty() {
                    text_parts.push(text);
                }
            }
        }
    }
    let mut message = Map::new();
    message.insert("role".to_string(), Value::String("assistant".to_string()));
    if text_parts.is_empty() && !tool_calls.is_empty() {
        message.insert("content".to_string(), Value::Null);
    } else {
        message.insert("content".to_string(), Value::String(text_parts.join("\n")));
    }
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    if message.get("content") != Some(&Value::String(String::new()))
        || message.contains_key("tool_calls")
    {
        messages.push(Value::Object(message));
    }
}

fn content_blocks(content: &Value) -> Vec<Value> {
    match content {
        Value::String(text) => vec![json!({"type": "text", "text": text})],
        Value::Array(items) => items.clone(),
        Value::Object(_) => vec![content.clone()],
        _ => Vec::new(),
    }
}

fn content_value_to_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(content_value_to_text)
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => match map.get("type").and_then(Value::as_str) {
            Some("text") => map
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            Some("thinking") | Some("redacted_thinking") => String::new(),
            Some("tool_result") => {
                content_value_to_text(map.get("content").unwrap_or(&Value::Null))
            }
            _ => value.to_string(),
        },
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn request_to_lossy_text(request: &AnthropicRequest) -> String {
    let mut parts = Vec::new();
    if let Some(system) = &request.system {
        parts.push(content_value_to_text(system));
    }
    for message in &request.messages {
        parts.push(content_value_to_text(&message.content));
    }
    parts.join("\n")
}

fn estimate_openai_request_tokens(request: &Value) -> u64 {
    let Some(messages) = request.get("messages").and_then(Value::as_array) else {
        return estimate_tokens(&request.to_string());
    };
    let mut chars = 0_u64;
    for message in messages {
        chars += message
            .get("role")
            .and_then(Value::as_str)
            .map(|role| role.chars().count() as u64)
            .unwrap_or(0);
        chars += estimate_content_chars(message.get("content").unwrap_or(&Value::Null));
        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                chars += estimate_content_chars(tool_call);
            }
        }
    }
    if let Some(tools) = request.get("tools").and_then(Value::as_array) {
        for tool in tools {
            chars += estimate_content_chars(tool) / 3;
        }
    }
    estimate_tokens_by_chars(chars)
}

fn estimate_content_chars(value: &Value) -> u64 {
    match value {
        Value::String(text) => text.chars().count() as u64,
        Value::Array(items) => items.iter().map(estimate_content_chars).sum(),
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                return text.chars().count() as u64;
            }
            if let Some(image) = map.get("image_url") {
                return estimate_content_chars(image).min(4096);
            }
            value.to_string().chars().count() as u64
        }
        Value::Null => 0,
        _ => value.to_string().chars().count() as u64,
    }
}

fn anthropic_image_source_to_url(source: &Value) -> Option<String> {
    if source.get("type").and_then(Value::as_str) == Some("url") {
        return source
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    if source.get("type").and_then(Value::as_str) == Some("base64") {
        let media_type = source.get("media_type").and_then(Value::as_str)?;
        let data = source.get("data").and_then(Value::as_str)?;
        return Some(format!("data:{media_type};base64,{data}"));
    }
    None
}

fn anthropic_tool_to_openai(tool: &Value) -> Value {
    let mut function = Map::new();
    function.insert(
        "name".to_string(),
        tool.get("name")
            .cloned()
            .unwrap_or(Value::String("unknown_tool".to_string())),
    );
    if let Some(description) = tool.get("description").filter(|value| match value {
        Value::String(text) => !text.trim().is_empty(),
        Value::Null => false,
        _ => true,
    }) {
        function.insert("description".to_string(), description.clone());
    }
    function.insert(
        "parameters".to_string(),
        tool.get("input_schema")
            .cloned()
            .unwrap_or_else(|| json!({"type": "object"})),
    );
    json!({"type": "function", "function": Value::Object(function)})
}

fn anthropic_tool_choice_to_openai(choice: &Value) -> Option<Value> {
    match choice.get("type").and_then(Value::as_str) {
        Some("auto") => Some(Value::String("auto".to_string())),
        Some("none") => Some(Value::String("none".to_string())),
        Some("any") => Some(Value::String("required".to_string())),
        Some("tool") => choice
            .get("name")
            .and_then(Value::as_str)
            .map(|name| json!({"type": "function", "function": {"name": name}})),
        _ => None,
    }
}

fn anthropic_disable_parallel_tool_use(choice: &Option<Value>) -> bool {
    choice
        .as_ref()
        .and_then(|choice| choice.get("disable_parallel_tool_use"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn parse_stream_obfuscation(raw: &str) -> Result<StreamObfuscation, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "omit" | "none" | "off" => Ok(StreamObfuscation::Omit),
        "true" | "1" | "yes" | "on" => Ok(StreamObfuscation::Include(true)),
        "false" | "0" | "no" => Ok(StreamObfuscation::Include(false)),
        value => Err(format!(
            "unsupported AOB_STREAM_OBFUSCATION value: {value}; use omit, true, or false"
        )),
    }
}

fn build_stream_options(obfuscation: &StreamObfuscation) -> Value {
    let mut options = Map::new();
    options.insert("include_usage".to_string(), Value::Bool(true));
    if let StreamObfuscation::Include(value) = obfuscation {
        options.insert("include_obfuscation".to_string(), Value::Bool(*value));
    }
    Value::Object(options)
}

fn stream_obfuscation_label(obfuscation: &StreamObfuscation) -> &'static str {
    match obfuscation {
        StreamObfuscation::Omit => "omit",
        StreamObfuscation::Include(true) => "true",
        StreamObfuscation::Include(false) => "false",
    }
}

fn openai_to_anthropic(default_model: &str, openai: &Value) -> AnthropicResponse {
    let choice = openai
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| json!({}));
    let message = choice.get("message").cloned().unwrap_or_else(|| json!({}));
    let mut content = Vec::new();
    if let Some(text) = message
        .get("content")
        .or_else(|| message.get("refusal"))
        .and_then(Value::as_str)
    {
        if !text.is_empty() {
            content.push(json!({"type": "text", "text": text}));
        }
    }
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tool_call in tool_calls {
            let function = tool_call.get("function").unwrap_or(&Value::Null);
            content.push(json!({
                "type": "tool_use",
                "id": tool_call.get("id").and_then(Value::as_str).unwrap_or("toolu_unknown"),
                "name": function.get("name").and_then(Value::as_str).unwrap_or("unknown_tool"),
                "input": parse_tool_arguments(function.get("arguments")).unwrap_or_else(|| json!({})),
            }));
        }
    }
    if let Some(function) = message.get("function_call") {
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown_tool");
        content.push(json!({
            "type": "tool_use",
            "id": legacy_function_call_id(name),
            "name": name,
            "input": parse_tool_arguments(function.get("arguments")).unwrap_or_else(|| json!({})),
        }));
    }
    if content.is_empty() {
        content.push(json!({"type": "text", "text": ""}));
    }
    let usage = openai.get("usage").unwrap_or(&Value::Null);
    let output_tokens = usage_token(usage, &["completion_tokens", "output_tokens"])
        .unwrap_or_else(|| estimate_tokens(&content_value_to_text(&Value::Array(content.clone()))));
    AnthropicResponse {
        id: openai
            .get("id")
            .and_then(Value::as_str)
            .map(|id| format!("msg_{id}"))
            .unwrap_or_else(|| format!("msg_{}", unix_seconds())),
        kind: "message".to_string(),
        role: "assistant".to_string(),
        content,
        model: openai
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(default_model)
            .to_string(),
        stop_reason: map_finish_reason(choice.get("finish_reason").and_then(Value::as_str)),
        stop_sequence: None,
        usage: AnthropicUsage {
            input_tokens: usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .or_else(|| usage_token(usage, &["input_tokens"]))
                .unwrap_or(0),
            output_tokens,
        },
    }
}

fn usage_token(usage: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| usage.get(*key).and_then(Value::as_u64))
}

fn parse_tool_arguments(value: Option<&Value>) -> Option<Value> {
    match value? {
        Value::String(text) => parse_tool_arguments_text(text),
        Value::Object(_) | Value::Array(_) => value.cloned(),
        _ => None,
    }
}

fn parse_tool_arguments_text(text: &str) -> Option<Value> {
    if text.trim().is_empty() {
        return Some(json!({}));
    }
    match serde_json::from_str(text) {
        Ok(value) => Some(value),
        Err(_) => Some(json!({"_raw": text})),
    }
}

fn legacy_function_call_id(name: &str) -> String {
    let mut sanitized = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            sanitized.push(character);
        }
    }
    if sanitized.is_empty() {
        sanitized.push_str("unknown_tool");
    }
    format!("{LEGACY_FUNCTION_ID_PREFIX}{sanitized}")
}

fn map_finish_reason(reason: Option<&str>) -> String {
    match reason {
        Some("length") => "max_tokens",
        Some("tool_calls") | Some("function_call") => "tool_use",
        Some("content_filter") => "refusal",
        Some("stop") | None => "end_turn",
        Some(_) => "end_turn",
    }
    .to_string()
}

fn sse_response(message: &AnthropicResponse) -> Response {
    let mut stream = String::new();
    push_sse(
        &mut stream,
        "message_start",
        &json!({
            "type": "message_start",
            "message": {
                "id": message.id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": message.model,
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {"input_tokens": message.usage.input_tokens, "output_tokens": 1}
            }
        }),
    );
    for (index, block) in message.content.iter().enumerate() {
        let block_start = if block.get("type").and_then(Value::as_str) == Some("tool_use") {
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "tool_use",
                    "id": block.get("id").cloned().unwrap_or_else(|| Value::String("toolu_unknown".to_string())),
                    "name": block.get("name").cloned().unwrap_or_else(|| Value::String("unknown_tool".to_string())),
                    "input": {}
                }
            })
        } else {
            json!({"type": "content_block_start", "index": index, "content_block": {"type": "text", "text": ""}})
        };
        push_sse(&mut stream, "content_block_start", &block_start);
        if block.get("type").and_then(Value::as_str) == Some("tool_use") {
            push_sse(
                &mut stream,
                "content_block_delta",
                &json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": block.get("input").cloned().unwrap_or_else(|| json!({})).to_string()
                    }
                }),
            );
        } else {
            push_sse(
                &mut stream,
                "content_block_delta",
                &json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {"type": "text_delta", "text": block.get("text").and_then(Value::as_str).unwrap_or("")}
                }),
            );
        }
        push_sse(
            &mut stream,
            "content_block_stop",
            &json!({"type": "content_block_stop", "index": index}),
        );
    }
    push_sse(
        &mut stream,
        "message_delta",
        &json!({
            "type": "message_delta",
            "delta": {"stop_reason": message.stop_reason, "stop_sequence": message.stop_sequence},
            "usage": {"output_tokens": message.usage.output_tokens}
        }),
    );
    push_sse(
        &mut stream,
        "message_stop",
        &json!({"type": "message_stop"}),
    );
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(stream))
        .expect("valid SSE response")
}

fn openai_stream_to_anthropic_response(
    response: reqwest::Response,
    default_model: String,
    estimated_input_tokens: u64,
    heartbeat_secs: u64,
    channel_depth: usize,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(channel_depth);
    tokio::spawn(async move {
        bridge_openai_stream(
            response,
            default_model,
            estimated_input_tokens,
            heartbeat_secs,
            tx,
        )
        .await;
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive")
        .header("x-accel-buffering", "no")
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .expect("valid streaming SSE response")
}

async fn bridge_openai_stream(
    response: reqwest::Response,
    default_model: String,
    estimated_input_tokens: u64,
    heartbeat_secs: u64,
    tx: mpsc::Sender<Result<Bytes, Infallible>>,
) {
    let mut state = StreamBridgeState::new(default_model, estimated_input_tokens);
    if !send_sse(&tx, "message_start", &state.message_start_event()).await {
        return;
    }

    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let heartbeat = Duration::from_secs(heartbeat_secs);
    loop {
        let chunk = if heartbeat_secs == 0 {
            stream.next().await
        } else {
            match time::timeout(heartbeat, stream.next()).await {
                Ok(chunk) => chunk,
                Err(_) => {
                    if !send_comment(&tx, "keep-alive").await {
                        return;
                    }
                    continue;
                }
            }
        };
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(err) => {
                let _ = send_sse(
                    &tx,
                    "error",
                    &json!({
                        "type": "error",
                        "error": {"type": "api_error", "message": format!("upstream stream failed: {err}")}
                    }),
                )
                .await;
                return;
            }
        };
        buffer.extend_from_slice(&chunk);
        while let Some((frame_len, drain_len)) = find_sse_frame(&buffer) {
            let frame = buffer[..frame_len].to_vec();
            buffer.drain(..drain_len);
            let frame = String::from_utf8_lossy(&frame);
            if !handle_sse_frame_data(&tx, &mut state, &frame).await {
                return;
            }
        }
    }
    if !buffer.is_empty() {
        let frame = String::from_utf8_lossy(&buffer);
        if !handle_sse_frame_data(&tx, &mut state, &frame).await {
            return;
        }
    }
    finish_stream(&tx, &mut state).await;
}

async fn handle_sse_frame_data(
    tx: &mpsc::Sender<Result<Bytes, Infallible>>,
    state: &mut StreamBridgeState,
    frame: &str,
) -> bool {
    for data in sse_frame_data(frame) {
        if data == "[DONE]" {
            finish_stream(tx, state).await;
            return false;
        }
        let chunk: Value = match serde_json::from_str(&data) {
            Ok(chunk) => chunk,
            Err(_) => continue,
        };
        let events = openai_stream_chunk_to_anthropic_events(state, &chunk);
        if !events.is_empty() && !send_sse_events(tx, events).await {
            return false;
        }
    }
    true
}

async fn finish_stream(
    tx: &mpsc::Sender<Result<Bytes, Infallible>>,
    state: &mut StreamBridgeState,
) {
    let mut events = Vec::new();
    state.finish(&mut events);
    if events.is_empty() {
        return;
    }
    let _ = send_sse_events(tx, events).await;
}

async fn send_sse(tx: &mpsc::Sender<Result<Bytes, Infallible>>, event: &str, data: &Value) -> bool {
    tx.send(Ok(Bytes::from(sse_event(event, data))))
        .await
        .is_ok()
}

async fn send_sse_events(
    tx: &mpsc::Sender<Result<Bytes, Infallible>>,
    events: Vec<StreamEvent>,
) -> bool {
    tx.send(Ok(sse_events(events))).await.is_ok()
}

async fn send_comment(tx: &mpsc::Sender<Result<Bytes, Infallible>>, comment: &str) -> bool {
    tx.send(Ok(Bytes::from(format!(": {comment}\n\n"))))
        .await
        .is_ok()
}

fn sse_event(event: &str, data: &Value) -> String {
    let mut output = String::new();
    push_sse(&mut output, event, data);
    output
}

fn sse_events(events: Vec<StreamEvent>) -> Bytes {
    let mut output = String::new();
    for (event, data) in events {
        push_sse(&mut output, event, &data);
    }
    Bytes::from(output)
}

fn find_sse_frame(buffer: &[u8]) -> Option<(usize, usize)> {
    for index in 0..buffer.len().saturating_sub(1) {
        if &buffer[index..index + 2] == b"\n\n" {
            return Some((index, index + 2));
        }
        if index + 4 <= buffer.len() && &buffer[index..index + 4] == b"\r\n\r\n" {
            return Some((index, index + 4));
        }
    }
    None
}

fn sse_frame_data(frame: &str) -> Vec<String> {
    let mut data_lines = Vec::new();
    for line in frame.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start().to_string());
        }
    }
    if data_lines.is_empty() {
        Vec::new()
    } else {
        vec![data_lines.join("\n")]
    }
}

#[derive(Debug)]
struct StreamBridgeState {
    id: String,
    model: String,
    text_index: Option<usize>,
    next_content_index: usize,
    tool_blocks: BTreeMap<u64, ToolStreamBlock>,
    stop_reason: String,
    input_tokens: u64,
    output_tokens: u64,
    output_chars: u64,
    stream_events: u64,
    finished: bool,
}

#[derive(Debug)]
struct ToolStreamBlock {
    content_index: Option<usize>,
    id: Option<String>,
    name: Option<String>,
    emitted_arguments: bool,
}

impl StreamBridgeState {
    fn new(model: String, input_tokens: u64) -> Self {
        Self {
            id: format!("msg_stream_{}", unix_seconds()),
            model,
            text_index: None,
            next_content_index: 0,
            tool_blocks: BTreeMap::new(),
            stop_reason: "end_turn".to_string(),
            input_tokens,
            output_tokens: 0,
            output_chars: 0,
            stream_events: 0,
            finished: false,
        }
    }

    fn message_start_event(&self) -> Value {
        json!({
            "type": "message_start",
            "message": {
                "id": self.id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": self.model,
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {"input_tokens": self.input_tokens, "output_tokens": 1}
            }
        })
    }

    fn ensure_text_block(&mut self, events: &mut Vec<StreamEvent>) -> usize {
        if let Some(index) = self.text_index {
            return index;
        }
        let index = self.next_content_index;
        self.next_content_index += 1;
        self.text_index = Some(index);
        events.push((
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {"type": "text", "text": ""}
            }),
        ));
        index
    }

    fn ensure_tool_block(
        &mut self,
        tool_index: u64,
        id: Option<&str>,
        name: Option<&str>,
        events: &mut Vec<StreamEvent>,
    ) -> usize {
        self.remember_tool_block(tool_index, id, name);
        if let Some(index) = self
            .tool_blocks
            .get(&tool_index)
            .and_then(|block| block.content_index)
        {
            return index;
        }
        let index = self.next_content_index;
        self.next_content_index += 1;
        let block = self
            .tool_blocks
            .entry(tool_index)
            .or_insert(ToolStreamBlock {
                content_index: None,
                id: None,
                name: None,
                emitted_arguments: false,
            });
        let name = block
            .name
            .clone()
            .unwrap_or_else(|| "unknown_tool".to_string());
        let id = block
            .id
            .clone()
            .unwrap_or_else(|| legacy_function_call_id(&name));
        block.content_index = Some(index);
        events.push((
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}
            }),
        ));
        index
    }

    fn remember_tool_block(&mut self, tool_index: u64, id: Option<&str>, name: Option<&str>) {
        let block = self
            .tool_blocks
            .entry(tool_index)
            .or_insert(ToolStreamBlock {
                content_index: None,
                id: None,
                name: None,
                emitted_arguments: false,
            });
        if let Some(id) = id {
            block.id = Some(id.to_string());
        }
        if let Some(name) = name {
            block.name = Some(name.to_string());
        }
    }

    fn finish(&mut self, events: &mut Vec<StreamEvent>) {
        if self.finished {
            return;
        }
        self.finished = true;
        for tool_index in self
            .tool_blocks
            .iter()
            .filter_map(|(tool_index, block)| {
                (block.content_index.is_none() && block.emitted_arguments).then_some(*tool_index)
            })
            .collect::<Vec<_>>()
        {
            self.ensure_tool_block(tool_index, None, None, events);
        }
        if let Some(index) = self.text_index {
            events.push((
                "content_block_stop",
                json!({"type": "content_block_stop", "index": index}),
            ));
        }
        for block in self.tool_blocks.values() {
            if let Some(content_index) = block.content_index {
                events.push((
                    "content_block_stop",
                    json!({"type": "content_block_stop", "index": content_index}),
                ));
            }
        }
        if self.output_tokens == 0 {
            self.output_tokens = estimate_tokens_by_chars(self.output_chars);
        }
        events.push((
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": {"stop_reason": self.stop_reason, "stop_sequence": null},
                "usage": {"output_tokens": self.output_tokens}
            }),
        ));
        events.push(("message_stop", json!({"type": "message_stop"})));
    }
}

fn openai_stream_chunk_to_anthropic_events(
    state: &mut StreamBridgeState,
    chunk: &Value,
) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    if let Some(model) = chunk.get("model").and_then(Value::as_str) {
        state.model = model.to_string();
    }
    if let Some(usage) = chunk.get("usage") {
        if let Some(tokens) = usage.get("prompt_tokens").and_then(Value::as_u64) {
            state.input_tokens = tokens;
        }
        if let Some(tokens) = usage.get("completion_tokens").and_then(Value::as_u64) {
            state.output_tokens = tokens;
        }
    }
    let Some(choices) = chunk.get("choices").and_then(Value::as_array) else {
        return events;
    };
    for choice in choices {
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            state.stop_reason = map_finish_reason(Some(reason));
        }
        let delta = choice.get("delta").unwrap_or(&Value::Null);
        if let Some(text) = delta
            .get("content")
            .or_else(|| delta.get("reasoning_content"))
            .or_else(|| delta.get("reasoning"))
            .and_then(Value::as_str)
        {
            if !text.is_empty() {
                let index = state.ensure_text_block(&mut events);
                state.output_chars += text.chars().count() as u64;
                state.stream_events += 1;
                events.push((
                    "content_block_delta",
                    json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {"type": "text_delta", "text": text}
                    }),
                ));
            }
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                let tool_index = tool_call.get("index").and_then(Value::as_u64).unwrap_or(0);
                let function = tool_call.get("function").unwrap_or(&Value::Null);
                let tool_id = tool_call.get("id").and_then(Value::as_str);
                let tool_name = function.get("name").and_then(Value::as_str);
                if function.get("arguments").and_then(Value::as_str).is_none() {
                    state.remember_tool_block(tool_index, tool_id, tool_name);
                    continue;
                }
                let index = state.ensure_tool_block(tool_index, tool_id, tool_name, &mut events);
                if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                    if !arguments.is_empty() {
                        if let Some(block) = state.tool_blocks.get_mut(&tool_index) {
                            block.emitted_arguments = true;
                        }
                        state.output_chars += arguments.chars().count() as u64;
                        state.stream_events += 1;
                        events.push((
                            "content_block_delta",
                            json!({
                                "type": "content_block_delta",
                                "index": index,
                                "delta": {"type": "input_json_delta", "partial_json": arguments}
                            }),
                        ));
                    }
                }
            }
        }
        if let Some(function) = delta.get("function_call") {
            let tool_name = function.get("name").and_then(Value::as_str);
            if function.get("arguments").and_then(Value::as_str).is_none() {
                state.remember_tool_block(LEGACY_FUNCTION_TOOL_INDEX, None, tool_name);
                continue;
            }
            let index =
                state.ensure_tool_block(LEGACY_FUNCTION_TOOL_INDEX, None, tool_name, &mut events);
            if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                if !arguments.is_empty() {
                    if let Some(block) = state.tool_blocks.get_mut(&LEGACY_FUNCTION_TOOL_INDEX) {
                        block.emitted_arguments = true;
                    }
                    state.output_chars += arguments.chars().count() as u64;
                    state.stream_events += 1;
                    events.push((
                        "content_block_delta",
                        json!({
                            "type": "content_block_delta",
                            "index": index,
                            "delta": {"type": "input_json_delta", "partial_json": arguments}
                        }),
                    ));
                }
            }
        }
    }
    events
}

fn push_sse(output: &mut String, event: &str, data: &Value) {
    output.push_str("event: ");
    output.push_str(event);
    output.push('\n');
    output.push_str("data: ");
    output.push_str(&data.to_string());
    output.push_str("\n\n");
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({
            "type": "error",
            "error": {
                "type": "api_error",
                "message": message
            }
        })),
    )
        .into_response()
}

fn insert_number(map: &mut Map<String, Value>, key: &str, value: f64) {
    if let Some(number) = serde_json::Number::from_f64(value) {
        map.insert(key.to_string(), Value::Number(number));
    }
}

fn estimate_tokens(text: &str) -> u64 {
    ((text.chars().count() as u64) / 4).max(1)
}

fn estimate_tokens_by_chars(chars: u64) -> u64 {
    (chars / 4).max(1)
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> AppState {
        AppState {
            client: Client::new(),
            upstream_base: "http://127.0.0.1:8318/v1".to_string(),
            upstream_key: "sk-dummy".to_string(),
            model: "gpt-5.5".to_string(),
            preserve_request_model: false,
            system_role: "developer".to_string(),
            reasoning_effort: None,
            stream_include_usage: true,
            stream_obfuscation: StreamObfuscation::Include(false),
            max_tokens_field: "auto".to_string(),
            stream_heartbeat_secs: 5,
            max_request_bytes: 64 * 1024 * 1024,
            stream_channel_depth: 64,
            upstream_request_timeout_secs: 300,
        }
    }

    #[test]
    fn maps_system_tools_and_tool_results_without_text_wrapping() {
        let request = AnthropicRequest {
            model: Some("claude-sonnet-4-5".to_string()),
            max_tokens: Some(128),
            system: Some(Value::String("You are direct.".to_string())),
            tools: Some(vec![
                json!({"name":"read_file","description":"Read a file","input_schema":{"type":"object","properties":{"path":{"type":"string"}}}}),
            ]),
            tool_choice: Some(json!({"type": "auto"})),
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: false,
            messages: vec![
                AnthropicMessage {
                    role: "user".to_string(),
                    content: Value::String("read it".to_string()),
                },
                AnthropicMessage {
                    role: "assistant".to_string(),
                    content: json!([
                        {"type":"thinking","thinking":"Claude-only hidden chain"},
                        {"type":"tool_use","id":"toolu_1","name":"read_file","input":{"path":"README.md"}}
                    ]),
                },
                AnthropicMessage {
                    role: "user".to_string(),
                    content: json!([
                        {"type":"tool_result","tool_use_id":"toolu_1","content":"ok"}
                    ]),
                },
            ],
        };
        let mapped = build_openai_request(&state(), &request, false).expect("mapped");
        assert_eq!(
            mapped["messages"][0],
            json!({"role":"developer","content":"You are direct."})
        );
        assert_eq!(mapped["model"], "gpt-5.5");
        assert_eq!(mapped["max_completion_tokens"], 128);
        assert_eq!(
            mapped["messages"][1],
            json!({"role":"user","content":"read it"})
        );
        assert_eq!(mapped["messages"][2]["content"], Value::Null);
        assert_eq!(
            mapped["messages"][2]["tool_calls"][0]["function"]["name"],
            "read_file"
        );
        assert_eq!(
            mapped["messages"][3],
            json!({"role":"tool","tool_call_id":"toolu_1","content":"ok"})
        );
        assert_eq!(
            mapped["tools"][0]["function"]["parameters"]["properties"]["path"]["type"],
            "string"
        );
        assert_eq!(mapped["stream"], false);
    }

    #[test]
    fn plain_user_text_maps_to_string_to_reduce_payload_tokens() {
        let request = AnthropicRequest {
            model: None,
            max_tokens: None,
            system: None,
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: json!([
                    {"type":"thinking","thinking":"not forwarded"},
                    {"type":"text","text":"hello"},
                    {"type":"text","text":"world"}
                ]),
            }],
        };
        let mapped = build_openai_request(&state(), &request, false).expect("mapped");
        assert_eq!(
            mapped["messages"][0],
            json!({"role": "user", "content": "hello\nworld"})
        );
    }

    #[test]
    fn tool_schema_omits_empty_description() {
        let tool = anthropic_tool_to_openai(&json!({
            "name": "read_file",
            "description": "",
            "input_schema": {"type": "object"}
        }));
        assert!(tool["function"].get("description").is_none());
        assert_eq!(tool["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn maps_disable_parallel_tool_use_to_openai_parallel_flag() {
        let request = AnthropicRequest {
            model: None,
            max_tokens: None,
            system: None,
            tools: Some(vec![
                json!({"name":"read_file","input_schema":{"type":"object"}}),
            ]),
            tool_choice: Some(json!({"type": "auto", "disable_parallel_tool_use": true})),
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: Value::String("read it".to_string()),
            }],
        };
        let mapped = build_openai_request(&state(), &request, false).expect("mapped");
        assert_eq!(mapped["tool_choice"], "auto");
        assert_eq!(mapped["parallel_tool_calls"], false);
    }

    #[test]
    fn flushes_user_text_before_tool_result_to_preserve_order() {
        let request = AnthropicRequest {
            model: None,
            max_tokens: None,
            system: None,
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: json!([
                    {"type":"text","text":"before"},
                    {"type":"tool_result","tool_use_id":"toolu_1","content":"ok"},
                    {"type":"text","text":"after"}
                ]),
            }],
        };
        let mapped = build_openai_request(&state(), &request, false).expect("mapped");
        assert_eq!(mapped["messages"][0]["role"], "user");
        assert_eq!(mapped["messages"][0]["content"][0]["text"], "before");
        assert_eq!(mapped["messages"][1]["role"], "tool");
        assert_eq!(mapped["messages"][2]["content"][0]["text"], "after");
    }

    #[test]
    fn ignores_tool_choice_without_tools() {
        let request = AnthropicRequest {
            model: None,
            max_tokens: None,
            system: None,
            tools: None,
            tool_choice: Some(json!({"type": "auto", "disable_parallel_tool_use": true})),
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: Value::String("ping".to_string()),
            }],
        };
        let mapped = build_openai_request(&state(), &request, false).expect("mapped");
        assert!(mapped.get("tool_choice").is_none());
        assert!(mapped.get("parallel_tool_calls").is_none());
    }

    #[test]
    fn non_stream_response_uses_upstream_model_fallback() {
        let response = openai_to_anthropic(
            "openai/gpt-5.5",
            &json!({
                "id": "chatcmpl_test",
                "choices": [{"message": {"content": "pong"}, "finish_reason": "stop"}]
            }),
        );
        assert_eq!(response.model, "openai/gpt-5.5");
    }

    #[test]
    fn drops_sampling_parameters_for_reasoning_models() {
        let request = AnthropicRequest {
            model: None,
            max_tokens: None,
            system: None,
            tools: None,
            tool_choice: None,
            temperature: Some(0.2),
            top_p: Some(0.9),
            stop_sequences: None,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: Value::String("ping".to_string()),
            }],
        };
        let mapped = build_openai_request(&state(), &request, false).expect("mapped");
        assert!(mapped.get("temperature").is_none());
        assert!(mapped.get("top_p").is_none());

        let mut legacy_state = state();
        legacy_state.model = "gpt-4o".to_string();
        let mapped = build_openai_request(&legacy_state, &request, false).expect("mapped");
        assert_eq!(mapped["temperature"], 0.2);
        assert_eq!(mapped["top_p"], 0.9);
    }

    #[test]
    fn streaming_request_uses_upstream_stream_and_usage() {
        let request = AnthropicRequest {
            model: None,
            max_tokens: Some(128),
            system: None,
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: true,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: Value::String("ping".to_string()),
            }],
        };
        let mapped = build_openai_request(&state(), &request, true).expect("mapped");
        assert_eq!(mapped["stream"], true);
        assert_eq!(mapped["stream_options"]["include_usage"], true);
        assert_eq!(mapped["stream_options"]["include_obfuscation"], false);
    }

    #[test]
    fn state_defaults_keep_pressure_knobs_bounded() {
        let state = state();
        assert_eq!(state.max_request_bytes, 64 * 1024 * 1024);
        assert_eq!(state.stream_channel_depth, 64);
    }

    #[test]
    fn can_preserve_request_model_and_force_reasoning_knobs() {
        let mut state = state();
        state.preserve_request_model = true;
        state.reasoning_effort = Some("low".to_string());
        state.max_tokens_field = "max_completion_tokens".to_string();
        state.stream_obfuscation = StreamObfuscation::Include(false);
        let request = AnthropicRequest {
            model: Some("openai/gpt-5.5".to_string()),
            max_tokens: Some(128),
            system: None,
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: true,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: Value::String("ping".to_string()),
            }],
        };
        let mapped = build_openai_request(&state, &request, true).expect("mapped");
        assert_eq!(mapped["model"], "openai/gpt-5.5");
        assert_eq!(mapped["reasoning_effort"], "low");
        assert_eq!(mapped["max_completion_tokens"], 128);
        assert_eq!(mapped["stream_options"]["include_obfuscation"], false);
    }

    #[test]
    fn preserved_claude_alias_does_not_get_gpt_reasoning_knobs() {
        let mut state = state();
        state.preserve_request_model = true;
        state.reasoning_effort = Some("low".to_string());
        let request = AnthropicRequest {
            model: Some("claude-sonnet-4-5".to_string()),
            max_tokens: Some(128),
            system: None,
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: Value::String("ping".to_string()),
            }],
        };
        let mapped = build_openai_request(&state, &request, false).expect("mapped");
        assert_eq!(mapped["model"], "claude-sonnet-4-5");
        assert!(mapped.get("reasoning_effort").is_none());
    }

    #[test]
    fn auto_max_tokens_uses_legacy_field_for_non_reasoning_models() {
        let mut state = state();
        state.model = "gpt-4o".to_string();
        let request = AnthropicRequest {
            model: None,
            max_tokens: Some(128),
            system: None,
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: Value::String("ping".to_string()),
            }],
        };
        let mapped = build_openai_request(&state, &request, false).expect("mapped");
        assert_eq!(mapped["max_tokens"], 128);
        assert!(mapped.get("max_completion_tokens").is_none());
    }

    #[test]
    fn count_tokens_estimates_translated_request_not_raw_claude_json() {
        let request = AnthropicRequest {
            model: None,
            max_tokens: None,
            system: Some(Value::String("System".to_string())),
            tools: None,
            tool_choice: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            stream: false,
            messages: vec![AnthropicMessage {
                role: "assistant".to_string(),
                content: json!([
                    {"type":"thinking","thinking":"this should not count"},
                    {"type":"text","text":"visible"}
                ]),
            }],
        };
        let mapped = build_openai_request(&state(), &request, false).expect("mapped");
        let translated = estimate_openai_request_tokens(&mapped);
        let raw = estimate_tokens(&serde_json::to_string(&mapped).expect("json"));
        assert!(translated < raw);
    }

    #[test]
    fn maps_openai_tool_calls_back_to_anthropic_blocks() {
        let response = openai_to_anthropic(
            "gpt-5.5",
            &json!({
                "id": "chatcmpl_test",
                "model": "gpt-5.5",
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "checking",
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {"name": "read_file", "arguments": "{\"path\":\"README.md\"}"}
                        }]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": {"prompt_tokens": 10, "completion_tokens": 3}
            }),
        );
        assert_eq!(response.stop_reason, "tool_use");
        assert_eq!(
            response.content[0],
            json!({"type":"text","text":"checking"})
        );
        assert_eq!(response.content[1]["type"], "tool_use");
        assert_eq!(response.content[1]["input"]["path"], "README.md");
    }

    #[test]
    fn maps_object_tool_arguments_refusal_and_response_usage_aliases() {
        let response = openai_to_anthropic(
            "gpt-5.5",
            &json!({
                "id": "chatcmpl_test",
                "choices": [{
                    "message": {
                        "refusal": "cannot comply",
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {"name": "read_file", "arguments": {"path": "README.md"}}
                        }]
                    },
                    "finish_reason": "content_filter"
                }],
                "usage": {"input_tokens": 11, "output_tokens": 4}
            }),
        );
        assert_eq!(response.stop_reason, "refusal");
        assert_eq!(response.usage.input_tokens, 11);
        assert_eq!(response.usage.output_tokens, 4);
        assert_eq!(response.content[0]["text"], "cannot comply");
        assert_eq!(response.content[1]["input"]["path"], "README.md");
    }

    #[test]
    fn preserves_malformed_tool_arguments_in_raw_field() {
        let response = openai_to_anthropic(
            "gpt-5.5",
            &json!({
                "choices": [{
                    "message": {
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {"name": "write_file", "arguments": "{\"path\""}
                        }]
                    },
                    "finish_reason": "tool_calls"
                }]
            }),
        );
        assert_eq!(response.stop_reason, "tool_use");
        assert_eq!(response.content[0]["input"]["_raw"], "{\"path\"");
    }

    #[test]
    fn stream_response_uses_anthropic_event_sequence() {
        let message = AnthropicResponse {
            id: "msg_test".to_string(),
            kind: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![json!({"type":"text","text":"pong"})],
            model: "gpt-5.5".to_string(),
            stop_reason: "end_turn".to_string(),
            stop_sequence: None,
            usage: AnthropicUsage {
                input_tokens: 1,
                output_tokens: 1,
            },
        };
        let response = sse_response(&message);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/event-stream")
        );
    }

    #[test]
    fn sse_parser_waits_for_complete_frames_and_joins_multiline_data() {
        assert_eq!(find_sse_frame(b"data: {\"partial\": true}\n"), None);
        assert_eq!(find_sse_frame(b"data: {\"ok\": true}\n\n"), Some((18, 20)));
        assert_eq!(
            sse_frame_data("event: message\ndata: {\"a\":\ndata: 1}\n"),
            vec!["{\"a\":\n1}".to_string()]
        );
    }

    #[test]
    fn maps_openai_stream_chunks_to_anthropic_deltas() {
        let mut state = StreamBridgeState::new("gpt-5.5".to_string(), 7);
        assert_eq!(
            state.message_start_event()["message"]["usage"]["input_tokens"],
            7
        );
        let text_events = openai_stream_chunk_to_anthropic_events(
            &mut state,
            &json!({
                "model": "gpt-5.5",
                "choices": [{"delta": {"content": "pong"}, "finish_reason": null}]
            }),
        );
        assert_eq!(text_events[0].0, "content_block_start");
        assert_eq!(text_events[1].1["delta"]["type"], "text_delta");
        assert_eq!(text_events[1].1["delta"]["text"], "pong");

        let tool_events = openai_stream_chunk_to_anthropic_events(
            &mut state,
            &json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "function": {"name": "read_file", "arguments": "{\"path\""}
                        }]
                    },
                    "finish_reason": null
                }]
            }),
        );
        assert_eq!(tool_events[0].0, "content_block_start");
        assert_eq!(tool_events[1].1["delta"]["type"], "input_json_delta");
        assert_eq!(tool_events[1].1["delta"]["partial_json"], "{\"path\"");
    }

    #[test]
    fn maps_legacy_function_call_and_reasoning_stream_shapes() {
        let legacy = openai_to_anthropic(
            "gpt-4o",
            &json!({
                "id": "chatcmpl_legacy",
                "choices": [{
                    "message": {
                        "function_call": {"name": "read_file", "arguments": "{\"path\":\"README.md\"}"}
                    },
                    "finish_reason": "function_call"
                }]
            }),
        );
        assert_eq!(legacy.stop_reason, "tool_use");
        assert_eq!(legacy.content[0]["id"], "call_legacy_read_file");
        assert_eq!(legacy.content[0]["input"]["path"], "README.md");

        let mut state = StreamBridgeState::new("gpt-4o".to_string(), 1);
        let events = openai_stream_chunk_to_anthropic_events(
            &mut state,
            &json!({
                "choices": [{
                    "delta": {
                        "reasoning_content": "visible reasoning ",
                        "function_call": {"name": "read_file", "arguments": "{\"path\""}
                    },
                    "finish_reason": "function_call"
                }]
            }),
        );
        assert_eq!(events[1].1["delta"]["text"], "visible reasoning ");
        assert_eq!(events[2].1["content_block"]["id"], "call_legacy_read_file");
        assert_eq!(events[3].1["delta"]["partial_json"], "{\"path\"");
    }

    #[test]
    fn delays_tool_stream_block_until_arguments_arrive() {
        let mut state = StreamBridgeState::new("gpt-5.5".to_string(), 1);
        let name_only = openai_stream_chunk_to_anthropic_events(
            &mut state,
            &json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "function": {"name": "read_file"}
                        }]
                    }
                }]
            }),
        );
        assert!(name_only.is_empty());

        let argument_events = openai_stream_chunk_to_anthropic_events(
            &mut state,
            &json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "function": {"arguments": "{\"path\""}
                        }]
                    }
                }]
            }),
        );
        assert_eq!(argument_events[0].1["content_block"]["id"], "call_1");
        assert_eq!(argument_events[0].1["content_block"]["name"], "read_file");
        assert_eq!(argument_events[1].1["delta"]["partial_json"], "{\"path\"");
    }

    #[test]
    fn finish_does_not_emit_empty_tool_block_from_name_only_delta() {
        let mut state = StreamBridgeState::new("gpt-5.5".to_string(), 1);
        let events = openai_stream_chunk_to_anthropic_events(
            &mut state,
            &json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "function": {"name": "read_file"}
                        }]
                    },
                    "finish_reason": "stop"
                }]
            }),
        );
        assert!(events.is_empty());
        let mut finish_events = Vec::new();
        state.finish(&mut finish_events);
        assert!(finish_events
            .iter()
            .all(|event| event.0 != "content_block_start"));
        assert_eq!(
            finish_events.last().map(|event| event.0),
            Some("message_stop")
        );
    }

    #[test]
    fn finish_stream_is_idempotent_after_done_frames() {
        let mut state = StreamBridgeState::new("gpt-5.5".to_string(), 1);
        let mut first = Vec::new();
        state.finish(&mut first);
        let mut second = Vec::new();
        state.finish(&mut second);
        assert_eq!(first.last().map(|event| event.0), Some("message_stop"));
        assert!(second.is_empty());
    }
}
