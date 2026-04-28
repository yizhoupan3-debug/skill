use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const FRAMEWORK_CHANGED_CONTEXT: &str =
    "Framework routing/runtime files changed; run the targeted Rust contract tests before finishing.";
const SETTINGS_CHANGED_CONTEXT: &str =
    "Claude hook/settings files changed; validate JSON and run the Claude hook contract tests before finishing.";
const AUTOMATION_CONTEXT: &str =
    "Automation requests such as 'from now on', 'whenever', or 'before/after' must be implemented through settings hooks, not memory alone.";

const FRAMEWORK_GUARDED_PREFIXES: &[&str] = &[
    "scripts/router-rs/",
    "configs/framework/",
    "skills/SKILL_",
    "skills/SKILL_ROUTING_RUNTIME.json",
    "skills/SKILL_MANIFEST.json",
    "skills/SKILL_TIERS.json",
    "skills/SKILL_SHADOW_MAP.json",
];

const SETTINGS_GUARDED_PATHS: &[&str] = &[".claude/settings.json", ".claude/settings.local.json"];
const GENERATED_ENTRYPOINT_PATHS: &[&str] = &["AGENTS.md", "CLAUDE.md", ".claude/CLAUDE.md"];
const RETIRED_SURFACE_PATHS: &[&str] = &[
    ".codex/hooks.json",
    ".agents",
    "plugins/skill-framework-native/.mcp.json",
];
const HOST_PRIVATE_PREFIXES: &[&str] = &["/Users/joe/.claude/", "~/.claude/"];

pub fn run_claude_hook(command: &str, repo_root: &Path) -> Result<Value, String> {
    let canonical = canonical_claude_hook_command(command)?;
    let payload = read_stdin_payload()?;
    let response = match canonical {
        "pre-tool-use" => run_pre_tool_use(repo_root, &payload),
        "user-prompt-submit" => run_user_prompt_submit(&payload),
        "post-tool-use" => run_post_tool_use(repo_root, &payload),
        "stop" => run_stop(repo_root, &payload),
        _ => unreachable!("canonical hook command should be exhaustive"),
    };
    Ok(response.unwrap_or_else(silent_success))
}

fn canonical_claude_hook_command(command: &str) -> Result<&'static str, String> {
    match command {
        "pre-tool-use" => Ok("pre-tool-use"),
        "user-prompt-submit" => Ok("user-prompt-submit"),
        "post-tool-use" => Ok("post-tool-use"),
        "stop" => Ok("stop"),
        _ => Err(format!("Unsupported Claude hook command: {command}")),
    }
}

fn read_stdin_payload() -> Result<Value, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|err| err.to_string())?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(trimmed).or_else(|_| Ok(json!({ "raw": trimmed })))
}

fn silent_success() -> Value {
    json!({ "suppressOutput": true })
}

fn deny_pre_tool_use(reason: String) -> Option<Value> {
    Some(json!({
        "suppressOutput": true,
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

fn add_context(event: &str, context: &str) -> Option<Value> {
    Some(json!({
        "suppressOutput": true,
        "hookSpecificOutput": {
            "hookEventName": event,
            "additionalContext": context,
        },
    }))
}

fn block_stop(reason: &str) -> Option<Value> {
    Some(json!({
        "decision": "block",
        "reason": reason,
        "suppressOutput": true,
    }))
}

fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    if let Some(reason) = dangerous_bash_reason(payload) {
        return deny_pre_tool_use(reason);
    }
    for path in payload_relative_paths(repo_root, payload) {
        if is_retired_surface(&path) {
            return deny_pre_tool_use(format!(
                "Blocked restoring retired generated surface {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_generated_entrypoint(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of generated host entrypoint {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_host_private_path(&path) {
            return deny_pre_tool_use(format!(
                "Blocked direct mutation of host-private Claude state {path}; project policy must live in repo settings or Rust runtime code."
            ));
        }
    }
    if let Some(path) = bash_write_target(payload) {
        if is_retired_surface(&path) {
            return deny_pre_tool_use(format!(
                "Blocked shell mutation of retired generated surface {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_generated_entrypoint(&path) {
            return deny_pre_tool_use(format!(
                "Blocked shell mutation of generated host entrypoint {path}; use the Rust host-entrypoint sync path instead."
            ));
        }
        if is_host_private_path(&path) {
            return deny_pre_tool_use(format!(
                "Blocked shell mutation of host-private Claude state {path}; keep shared policy in project settings."
            ));
        }
    }
    None
}

fn run_user_prompt_submit(payload: &Value) -> Option<Value> {
    let prompt = payload
        .get("prompt")
        .or_else(|| payload.get("user_prompt"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if prompt_mentions_automation(prompt) {
        return add_context("UserPromptSubmit", AUTOMATION_CONTEXT);
    }
    None
}

fn run_post_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    let paths = payload_relative_paths(repo_root, payload);
    let touched_settings = paths.iter().any(|path| is_settings_path(path));
    let touched_framework = paths.iter().any(|path| is_framework_guarded_path(path));
    if touched_settings || touched_framework {
        persist_touch_state(repo_root, touched_settings, touched_framework);
    }
    if touched_settings {
        return add_context("PostToolUse", SETTINGS_CHANGED_CONTEXT);
    }
    if touched_framework {
        return add_context("PostToolUse", FRAMEWORK_CHANGED_CONTEXT);
    }
    None
}

fn run_stop(repo_root: &Path, payload: &Value) -> Option<Value> {
    let state = read_touch_state(repo_root);
    if state.settings && !payload_mentions_validation(payload) {
        return block_stop("Validate Claude hook/settings JSON before ending this turn.");
    }
    if state.framework && !payload_mentions_tests(payload) {
        return block_stop("Run targeted Rust contract tests for framework routing/runtime changes before ending this turn.");
    }
    clear_touch_state(repo_root);
    None
}

#[derive(Default)]
struct TouchState {
    settings: bool,
    framework: bool,
}

fn touch_state_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".claude/hook_state.json")
}

fn persist_touch_state(repo_root: &Path, settings: bool, framework: bool) {
    let current = read_touch_state(repo_root);
    let payload = json!({
        "settings": current.settings || settings,
        "framework": current.framework || framework,
    });
    let path = touch_state_path(repo_root);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{}\n", payload));
}

fn read_touch_state(repo_root: &Path) -> TouchState {
    let path = touch_state_path(repo_root);
    let Ok(raw) = fs::read_to_string(path) else {
        return TouchState::default();
    };
    let Ok(payload) = serde_json::from_str::<Value>(&raw) else {
        return TouchState::default();
    };
    TouchState {
        settings: payload
            .get("settings")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        framework: payload
            .get("framework")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn clear_touch_state(repo_root: &Path) {
    let _ = fs::remove_file(touch_state_path(repo_root));
}

fn prompt_mentions_automation(prompt: &str) -> bool {
    let lowered = prompt.to_ascii_lowercase();
    [
        "from now on",
        "whenever",
        "every time",
        "each time",
        "before ",
        "after ",
        "每次",
        "以后",
        "从现在起",
        " whenever ",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn dangerous_bash_reason(payload: &Value) -> Option<String> {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return None;
    }
    let command = bash_command(payload)?;
    let lowered = command.to_ascii_lowercase();
    let dangerous = [
        (
            r"(^|\s)rm\s+-[^\n;&|]*r[^\n;&|]*f\s+/(\s|$)",
            "rm -rf against filesystem root",
        ),
        (r"\bgit\s+reset\s+--hard\b", "git reset --hard"),
        (
            r"\bgit\s+clean\s+-[^\n;&|]*[fd][^\n;&|]*[fd]",
            "git clean -fd",
        ),
        (
            r"\bgit\s+push\b[^\n;&|]*(--force|-f\b|--force-with-lease)",
            "git force push",
        ),
        (r"\bgit\s+branch\s+-d\b", "git branch deletion"),
        (r"\bmkfs\.", "filesystem formatting command"),
        (r":\(\)\s*\{\s*:\|:&\s*\};:", "fork bomb"),
    ];
    for (pattern, label) in dangerous {
        if Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(&lowered))
            .unwrap_or(false)
        {
            return Some(format!("Blocked dangerous shell command: {label}."));
        }
    }
    None
}

fn payload_relative_paths(repo_root: &Path, payload: &Value) -> Vec<String> {
    let mut paths = HashSet::new();
    collect_payload_paths(payload, &mut paths);
    paths
        .into_iter()
        .map(|path| relative_candidate_path(&path, repo_root))
        .collect()
}

fn collect_payload_paths(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if is_path_key(key) {
                    collect_path_value(child, paths);
                }
                collect_payload_paths(child, paths);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_payload_paths(item, paths);
            }
        }
        _ => {}
    }
}

fn collect_path_value(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::String(text) => {
            let normalized = text.replace('\\', "/");
            if !normalized.is_empty() {
                paths.insert(normalized);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_path_value(item, paths);
            }
        }
        _ => {}
    }
}

fn is_path_key(key: &str) -> bool {
    matches!(
        key,
        "file_path"
            | "changed_path"
            | "path"
            | "config_path"
            | "target_path"
            | "changed_files"
            | "file_paths"
            | "paths"
    )
}

fn relative_candidate_path(path: &str, repo_root: &Path) -> String {
    if is_host_private_path(path) {
        return path.replace('\\', "/");
    }
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        if let Ok(rel) = candidate
            .canonicalize()
            .unwrap_or(candidate.clone())
            .strip_prefix(
                repo_root
                    .canonicalize()
                    .unwrap_or_else(|_| repo_root.to_path_buf()),
            )
        {
            return rel.to_string_lossy().replace('\\', "/");
        }
    }
    path.replace('\\', "/")
}

fn bash_write_target(payload: &Value) -> Option<String> {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return None;
    }
    let command = bash_command(payload)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        for hint in RETIRED_SURFACE_PATHS
            .iter()
            .chain(GENERATED_ENTRYPOINT_PATHS.iter())
            .chain(HOST_PRIVATE_PREFIXES.iter())
        {
            if !segment.contains(hint) {
                continue;
            }
            if looks_mutating || bash_segment_redirects_to_hint(&segment, hint) {
                return Some((*hint).to_string());
            }
        }
    }
    None
}

fn bash_command(payload: &Value) -> Option<&str> {
    payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)
}

fn split_bash_segments(command: &str) -> Vec<String> {
    Regex::new(r"\s*(?:&&|\|\||;|\|)\s*")
        .ok()
        .map(|regex| {
            regex
                .split(command)
                .filter_map(|segment| {
                    let trimmed = segment.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![command.trim().to_string()])
}

fn bash_command_looks_mutating(command: &str) -> bool {
    [
        r"^\s*(mv|cp|install|touch|rm|unlink|truncate|mkdir)\b",
        r"^\s*ln\b[^\n]*\s-[^\n]*[fs][^\n]*\b",
        r"^\s*git\s+(checkout\s+--|restore\b)",
        r"\bsed\s+-i\b",
        r"\bperl\s+-pi\b",
        r"\bpython3?\s+-c\b",
        r"\bnode\s+-e\b",
        r"\bruby\s+-e\b",
        r"\btee\b",
        r"\bdd\b",
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(command))
            .unwrap_or(false)
    })
}

fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    let escaped = regex::escape(hint);
    [
        format!(r#"(>>?|>\|)\s*['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
        format!(r#"\btee\b(?:\s+-a)?\s+['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
        format!(r#"\bdd\b[^\n;&|]*\bof=['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(segment))
            .unwrap_or(false)
    })
}

fn is_retired_surface(path: &str) -> bool {
    RETIRED_SURFACE_PATHS
        .iter()
        .any(|retired| path == *retired || path.starts_with(&format!("{retired}/")))
}

fn is_generated_entrypoint(path: &str) -> bool {
    GENERATED_ENTRYPOINT_PATHS
        .iter()
        .any(|generated| path == *generated)
}

fn is_host_private_path(path: &str) -> bool {
    HOST_PRIVATE_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

fn is_settings_path(path: &str) -> bool {
    SETTINGS_GUARDED_PATHS
        .iter()
        .any(|guarded| path == *guarded)
}

fn is_framework_guarded_path(path: &str) -> bool {
    FRAMEWORK_GUARDED_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(prefix))
}

fn payload_mentions_validation(payload: &Value) -> bool {
    payload_text(payload).contains("validated_settings_json")
}

fn payload_mentions_tests(payload: &Value) -> bool {
    let text = payload_text(payload);
    text.contains("cargo test") || text.contains("targeted_rust_contract_tests")
}

fn payload_text(payload: &Value) -> String {
    serde_json::to_string(payload)
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_dangerous_bash() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git reset --hard HEAD" }
        });
        let output = run_pre_tool_use(Path::new("/repo"), &payload).unwrap();
        assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "deny");
    }

    #[test]
    fn silent_for_safe_read_only_bash() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git status --short" }
        });
        assert!(run_pre_tool_use(Path::new("/repo"), &payload).is_none());
    }
}
