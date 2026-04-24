use axum::body::Body;
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
        .tcp_nodelay(true)
        .build()
        .map_err(|err| format!("client build failed: {err}"))?;
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
    });
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(models))
        .route("/v1/messages", post(messages))
        .route("/v1/messages/count_tokens", post(count_tokens))
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
    let response = state
        .client
        .post(upstream)
        .bearer_auth(resolve_upstream_key(&state, &headers))
        .json(&openai_body)
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
        return Ok(openai_stream_to_anthropic_response(
            response,
            state.model.clone(),
            state.stream_heartbeat_secs,
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
    let anthropic = openai_to_anthropic(&state.model, &openai_response);
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
    if let Some(effort) = &state.reasoning_effort {
        body.insert(
            "reasoning_effort".to_string(),
            Value::String(effort.to_string()),
        );
    }
    if let Some(value) = request.temperature {
        insert_number(&mut body, "temperature", value);
    }
    if let Some(value) = request.top_p {
        insert_number(&mut body, "top_p", value);
    }
    if let Some(stops) = &request.stop_sequences {
        body.insert("stop".to_string(), json!(stops));
    }
    if let Some(tools) = &request.tools {
        body.insert(
            "tools".to_string(),
            Value::Array(tools.iter().map(anthropic_tool_to_openai).collect()),
        );
    }
    if let Some(choice) = &request.tool_choice {
        if let Some(mapped) = anthropic_tool_choice_to_openai(choice) {
            body.insert("tool_choice".to_string(), mapped);
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
            if model.starts_with("gpt-5")
                || model.starts_with("o1")
                || model.starts_with("o3")
                || model.starts_with("o4")
            {
                "max_completion_tokens".to_string()
            } else {
                "max_tokens".to_string()
            }
        }
        value => value.to_string(),
    }
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
    let blocks = content_blocks(content);
    let mut user_parts = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("tool_result") => {
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
    message.insert("content".to_string(), Value::String(text_parts.join("\n")));
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    messages.push(Value::Object(message));
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
    json!({
        "type": "function",
        "function": {
            "name": tool.get("name").cloned().unwrap_or(Value::String("unknown_tool".to_string())),
            "description": tool.get("description").cloned().unwrap_or(Value::String(String::new())),
            "parameters": tool.get("input_schema").cloned().unwrap_or_else(|| json!({"type": "object"})),
        }
    })
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
    if let Some(text) = message.get("content").and_then(Value::as_str) {
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
                "input": parse_json_string(function.get("arguments")).unwrap_or_else(|| json!({})),
            }));
        }
    }
    if content.is_empty() {
        content.push(json!({"type": "text", "text": ""}));
    }
    let usage = openai.get("usage").unwrap_or(&Value::Null);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
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
                .unwrap_or(0),
            output_tokens,
        },
    }
}

fn parse_json_string(value: Option<&Value>) -> Option<Value> {
    let text = value.and_then(Value::as_str)?;
    serde_json::from_str(text).ok()
}

fn map_finish_reason(reason: Option<&str>) -> String {
    match reason {
        Some("length") => "max_tokens",
        Some("tool_calls") => "tool_use",
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
    heartbeat_secs: u64,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(256);
    tokio::spawn(async move {
        bridge_openai_stream(response, default_model, heartbeat_secs, tx).await;
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
    heartbeat_secs: u64,
    tx: mpsc::Sender<Result<Bytes, Infallible>>,
) {
    let mut state = StreamBridgeState::new(default_model);
    if !send_sse(
        &tx,
        "message_start",
        &json!({
            "type": "message_start",
            "message": {
                "id": state.id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": state.model,
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {"input_tokens": 0, "output_tokens": 1}
            }
        }),
    )
    .await
    {
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
            for data in sse_frame_data(&frame) {
                if data == "[DONE]" {
                    finish_stream(&tx, &mut state).await;
                    return;
                }
                let chunk: Value = match serde_json::from_str(&data) {
                    Ok(chunk) => chunk,
                    Err(_) => continue,
                };
                for (event, payload) in openai_stream_chunk_to_anthropic_events(&mut state, &chunk)
                {
                    if !send_sse(&tx, &event, &payload).await {
                        return;
                    }
                }
            }
        }
    }
    finish_stream(&tx, &mut state).await;
}

async fn finish_stream(
    tx: &mpsc::Sender<Result<Bytes, Infallible>>,
    state: &mut StreamBridgeState,
) {
    let mut events = Vec::new();
    state.finish(&mut events);
    for (event, payload) in events {
        if !send_sse(tx, &event, &payload).await {
            return;
        }
    }
}

async fn send_sse(tx: &mpsc::Sender<Result<Bytes, Infallible>>, event: &str, data: &Value) -> bool {
    tx.send(Ok(Bytes::from(sse_event(event, data))))
        .await
        .is_ok()
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

fn find_sse_frame(buffer: &[u8]) -> Option<(usize, usize)> {
    for index in 0..buffer.len().saturating_sub(1) {
        if &buffer[index..index + 2] == b"\n\n" {
            return Some((index, index + 2));
        }
        if index + 4 <= buffer.len() && &buffer[index..index + 4] == b"\r\n\r\n" {
            return Some((index, index + 4));
        }
    }
    if buffer.ends_with(b"\r\n") || buffer.ends_with(b"\n") {
        return Some((buffer.len(), buffer.len()));
    }
    None
}

fn sse_frame_data(frame: &str) -> Vec<String> {
    let mut items = Vec::new();
    for line in frame.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(data) = line.strip_prefix("data:") {
            items.push(data.trim_start().to_string());
        }
    }
    items
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
}

#[derive(Debug)]
struct ToolStreamBlock {
    content_index: usize,
}

impl StreamBridgeState {
    fn new(model: String) -> Self {
        Self {
            id: format!("msg_stream_{}", unix_seconds()),
            model,
            text_index: None,
            next_content_index: 0,
            tool_blocks: BTreeMap::new(),
            stop_reason: "end_turn".to_string(),
            input_tokens: 0,
            output_tokens: 0,
            output_chars: 0,
        }
    }

    fn ensure_text_block(&mut self, events: &mut Vec<(String, Value)>) -> usize {
        if let Some(index) = self.text_index {
            return index;
        }
        let index = self.next_content_index;
        self.next_content_index += 1;
        self.text_index = Some(index);
        events.push((
            "content_block_start".to_string(),
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
        events: &mut Vec<(String, Value)>,
    ) -> usize {
        if let Some(block) = self.tool_blocks.get(&tool_index) {
            return block.content_index;
        }
        let index = self.next_content_index;
        self.next_content_index += 1;
        let id = id.unwrap_or("toolu_stream").to_string();
        let name = name.unwrap_or("unknown_tool").to_string();
        self.tool_blocks.insert(
            tool_index,
            ToolStreamBlock {
                content_index: index,
            },
        );
        events.push((
            "content_block_start".to_string(),
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}
            }),
        ));
        index
    }

    fn finish(&mut self, events: &mut Vec<(String, Value)>) {
        if let Some(index) = self.text_index {
            events.push((
                "content_block_stop".to_string(),
                json!({"type": "content_block_stop", "index": index}),
            ));
        }
        for block in self.tool_blocks.values() {
            events.push((
                "content_block_stop".to_string(),
                json!({"type": "content_block_stop", "index": block.content_index}),
            ));
        }
        if self.output_tokens == 0 {
            self.output_tokens = estimate_tokens_by_chars(self.output_chars);
        }
        events.push((
            "message_delta".to_string(),
            json!({
                "type": "message_delta",
                "delta": {"stop_reason": self.stop_reason, "stop_sequence": null},
                "usage": {"output_tokens": self.output_tokens}
            }),
        ));
        events.push(("message_stop".to_string(), json!({"type": "message_stop"})));
    }
}

fn openai_stream_chunk_to_anthropic_events(
    state: &mut StreamBridgeState,
    chunk: &Value,
) -> Vec<(String, Value)> {
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
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            if !text.is_empty() {
                let index = state.ensure_text_block(&mut events);
                state.output_chars += text.chars().count() as u64;
                events.push((
                    "content_block_delta".to_string(),
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
                let index = state.ensure_tool_block(
                    tool_index,
                    tool_call.get("id").and_then(Value::as_str),
                    function.get("name").and_then(Value::as_str),
                    &mut events,
                );
                if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                    if !arguments.is_empty() {
                        state.output_chars += arguments.chars().count() as u64;
                        events.push((
                            "content_block_delta".to_string(),
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
        assert_eq!(mapped["messages"][2]["content"], "");
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
    fn can_preserve_request_model_and_force_reasoning_knobs() {
        let mut state = state();
        state.preserve_request_model = true;
        state.reasoning_effort = Some("low".to_string());
        state.max_tokens_field = "max_completion_tokens".to_string();
        state.stream_obfuscation = StreamObfuscation::Include(false);
        let request = AnthropicRequest {
            model: Some("claude-sonnet-4-5".to_string()),
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
        assert_eq!(mapped["model"], "claude-sonnet-4-5");
        assert_eq!(mapped["reasoning_effort"], "low");
        assert_eq!(mapped["max_completion_tokens"], 128);
        assert_eq!(mapped["stream_options"]["include_obfuscation"], false);
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
    fn maps_openai_stream_chunks_to_anthropic_deltas() {
        let mut state = StreamBridgeState::new("gpt-5.5".to_string());
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
}
