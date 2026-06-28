use serde_json::{Map, Value};

use super::HookEvent;

/// Normalize event name from various host formats (PascalCase, camelCase, kebab-case).
/// Returns Cow<str> to avoid allocation for already-canonical names.
pub fn normalize_event_name(name: &str) -> std::borrow::Cow<'_, str> {
    // Fast path: check if already lowercase with no separators (most common case)
    if name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return std::borrow::Cow::Borrowed(name);
    }
    let lower = name.to_lowercase();
    // Map common variants to canonical names
    match lower.as_str() {
        "sessionstart" | "session-start" | "session.start" => {
            std::borrow::Cow::Borrowed("sessionstart")
        }
        "userpromptsubmit"
        | "user-prompt-submit"
        | "user.prompt.submit"
        | "beforesubmitprompt"
        | "before-submit-prompt" => std::borrow::Cow::Borrowed("userpromptsubmit"),
        "pretooluse" | "pre-tool-use" | "pre.tool.use" | "tool.execute.before" => {
            std::borrow::Cow::Borrowed("pretooluse")
        }
        "posttooluse" | "post-tool-use" | "post.tool.use" | "tool.execute.after" => {
            std::borrow::Cow::Borrowed("posttooluse")
        }
        "stop" | "session.idle" => std::borrow::Cow::Borrowed("stop"),
        "subagentstart" | "subagent-start" | "subagent.start" => {
            std::borrow::Cow::Borrowed("subagentstart")
        }
        "subagentstop" | "subagent-stop" | "subagent.end" => {
            std::borrow::Cow::Borrowed("subagentstop")
        }
        other => std::borrow::Cow::Owned(other.to_string()),
    }
}

/// Extract prompt text from event payload, trying all common field names.
/// This is the superset of all host field names (13 direct keys).
///
/// Field ordering is by observed frequency across closed-set hosts:
///   - `prompt`, `user_prompt`: Claude + OpenCode (highest frequency)
///   - `message`, `content`: multi-host generic payloads
///   - `input`, `text`: OpenCode + generic
///   - `userPrompt`, `userMessage`: Cursor (camelCase)
///   - remaining: rare / host-specific fallbacks
pub fn extract_prompt_text(event: &Value) -> String {
    const KEYS: &[&str] = &[
        "prompt",
        "user_prompt",
        "message",
        "content",
        "input",
        "text",
        "userPrompt",
        "userMessage",
        "query",
        "userContent",
        "command",
        "composerText",
        "editorText",
    ];
    for key in KEYS {
        if let Some(value) = event.get(*key).and_then(Value::as_str)
            && !value.trim().is_empty()
        {
            return value.to_string();
        }
    }
    // Fallback: check prompt-like keys inside nested payload objects
    if let Some(obj) = event.as_object() {
        const NESTED: &[&str] = &["payload", "hookPayload", "data", "body", "hook_input"];
        for nest_key in NESTED {
            if let Some(nested) = obj.get(*nest_key).and_then(Value::as_object) {
                for key in KEYS {
                    if let Some(value) = nested.get(*key).and_then(Value::as_str)
                        && !value.trim().is_empty()
                    {
                        return value.to_string();
                    }
                }
            }
        }
        // Also check tool_input for prompt-like keys (Cursor nested payload convention)
        for ti_key in &["tool_input", "input", "arguments"] {
            if let Some(ti) = obj.get(*ti_key).and_then(Value::as_object) {
                for key in KEYS {
                    if let Some(value) = ti.get(*key).and_then(Value::as_str)
                        && !value.trim().is_empty()
                    {
                        return value.to_string();
                    }
                }
            }
        }
    }
    // Fallback: scan nested messages arrays for last user message
    extract_prompt_from_nested_messages(event)
}

/// Scan nested messages arrays for the last user-role message.
pub fn extract_prompt_from_nested_messages(event: &Value) -> String {
    extract_prompt_from_nested_messages_inner(event, 0)
}

const MAX_NESTING_DEPTH: usize = 4;

fn extract_prompt_from_nested_messages_inner(event: &Value, depth: usize) -> String {
    if depth > MAX_NESTING_DEPTH {
        return String::new();
    }
    const MESSAGE_KEYS: &[&str] = &[
        "messages",
        "conversationMessages",
        "chatMessages",
        "history",
    ];
    const NESTED_KEYS: &[&str] = &["hookPayload", "data", "body", "payload", "event"];

    if let Some(obj) = event.as_object() {
        // Try message arrays
        for key in MESSAGE_KEYS {
            if let Some(Value::Array(arr)) = obj.get(*key) {
                for item in arr.iter().rev() {
                    if let Some(msg) = item.as_object()
                        && is_user_message_role(msg)
                        && let Some(text) = message_body_text(msg)
                    {
                        return text;
                    }
                }
            }
        }
        // Recurse into nested containers (with depth limit)
        for key in NESTED_KEYS {
            if let Some(nested) = obj.get(*key) {
                let s = extract_prompt_from_nested_messages_inner(nested, depth + 1);
                if !s.trim().is_empty() {
                    return s;
                }
            }
        }
    }
    String::new()
}

/// Check if a message object has a user/human role.
pub fn is_user_message_role(obj: &serde_json::Map<String, Value>) -> bool {
    let role = obj
        .get("role")
        .or_else(|| obj.get("type"))
        .or_else(|| obj.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(role.as_str(), "user" | "human")
}

/// Extract body text from a message object.
pub fn message_body_text(msg: &serde_json::Map<String, Value>) -> Option<String> {
    for key in &["content", "text", "body", "message", "value"] {
        if let Some(val) = msg.get(*key) {
            if let Some(s) = val.as_str()
                && !s.trim().is_empty()
            {
                return Some(s.to_string());
            }
            // Handle content as array of parts (Claude/OpenAI format)
            if let Some(arr) = val.as_array() {
                let text: String = arr
                    .iter()
                    .filter_map(|part| {
                        part.get("text")
                            .or_else(|| part.get("content"))
                            .and_then(Value::as_str)
                    })
                    .collect::<Vec<_>>()
                    .join("");
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

/// Extract tool name from event payload.
pub fn extract_tool_name(event: &Value) -> String {
    let from_obj = |obj: &Map<String, Value>| {
        obj.get("tool_name")
            .or(obj.get("tool"))
            .or(obj.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    if let Some(name) = event.as_object().and_then(from_obj) {
        return name;
    }
    // Fallback: check inside nested payload objects
    if let Some(obj) = event.as_object() {
        for nest_key in &["payload", "hookPayload", "data", "body"] {
            if let Some(nested) = obj.get(*nest_key).and_then(Value::as_object)
                && let Some(name) = from_obj(nested)
            {
                return name;
            }
        }
    }
    String::new()
}

/// Extract tool input from event payload.
pub fn extract_tool_input(event: &Value) -> Value {
    let from_obj = |obj: &Map<String, Value>| {
        obj.get("tool_input")
            .or(obj.get("input"))
            .or(obj.get("arguments"))
            .cloned()
            .filter(Value::is_object)
    };
    if let Some(input) = event.as_object().and_then(from_obj) {
        return input;
    }
    // Fallback: check inside nested payload objects
    if let Some(obj) = event.as_object() {
        for nest_key in &["payload", "hookPayload", "data", "body"] {
            if let Some(nested) = obj.get(*nest_key).and_then(Value::as_object)
                && let Some(input) = from_obj(nested)
            {
                return input;
            }
        }
    }
    serde_json::json!({})
}

/// Extract completion text from Stop event payload.
pub fn extract_completion_text(event: &HookEvent) -> String {
    event
        .payload
        .get("completion_text")
        .or(event.payload.get("response"))
        .or(event.payload.get("result"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Shared stop signal text: combines prompt + assistant response for gate detection.
/// Used by all 4 hosts (Claude, Codex, OpenCode, Cursor).
pub fn stop_signal_text_from_payload(payload: &Value) -> String {
    let prompt = extract_prompt_text(payload);
    let response = extract_response_text(payload);
    if prompt.trim().is_empty() {
        response
    } else if response.trim().is_empty() {
        prompt
    } else {
        format!("{prompt}\n{response}")
    }
}

/// Extract assistant response text from payload, covering all host response key variants.
/// Keys checked: response, agent_response, agentResponse, assistant_response,
/// last_assistant_message, content, text, output (first non-empty wins).
pub fn extract_response_text(payload: &Value) -> String {
    const RESPONSE_KEYS: &[&str] = &[
        "response",
        "agent_response",
        "agentResponse",
        "assistant_response",
        "last_assistant_message",
        "content",
        "text",
        "output",
    ];
    for key in RESPONSE_KEYS {
        if let Some(value) = payload.get(*key).and_then(Value::as_str)
            && !value.trim().is_empty()
        {
            return value.to_string();
        }
    }
    String::new()
}
