use crate::cursor_hooks::{
    has_delegation_override, has_override, has_review_override, is_parallel_delegation_prompt,
    is_review_prompt, normalize_subagent_type, normalize_tool_name, saw_reject_reason,
};
use crate::framework_runtime::{
    build_automatic_continuity_checkpoint_payload, build_framework_contract_summary_envelope,
    build_framework_refresh_payload, try_append_codex_post_tool_evidence,
    write_framework_session_artifacts,
};
use chrono::Utc;
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Read};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CODEX_HOOK_AUTHORITY: &str = "rust-codex-audit";
const HOST_ENTRYPOINT_SYNC_MANIFEST_PATH: &str = ".codex/host_entrypoints_sync_manifest.json";
const HOST_ENTRYPOINT_SYNC_HINT: &str = "router-rs codex sync --repo-root \"$PWD\"";
const CODEX_AGENT_POLICY_PATH: &str = "AGENTS.md";
const CODEX_HOOKS_PATH: &str = ".codex/hooks.json";
const CODEX_HOOKS_README_PATH: &str = ".codex/README.md";
const HOST_ENTRYPOINT_JSON_RELATIVE_PATHS: [&str; 1] = [CODEX_HOOKS_PATH];
const PROTECTED_GENERATED_PATHS: [&str; 4] = [
    CODEX_AGENT_POLICY_PATH,
    CODEX_HOOKS_PATH,
    CODEX_HOOKS_README_PATH,
    HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
];
const PROTECTED_GENERATED_PREFIXES: [&str; 0] = [];
const CODEX_REVIEW_SUBAGENT_TOOL_NAMES: [&str; 6] = [
    "task",
    "functions.task",
    "functions.subagent",
    "functions.spawn_agent",
    "subagent",
    "spawn_agent",
];
const CODEX_REVIEW_SUBAGENT_TYPES: &[&str] = &[
    "default",
    "explore",
    "explorer",
    "general-purpose",
    "generalpurpose",
    "shell",
    "worker",
    "browser-use",
    "browseruse",
    "ci-investigator",
    "ciinvestigator",
    "best-of-n-runner",
    "bestofnrunner",
    "cursor-guide",
    "cursorguide",
];
const INSTALL_EVENTS: [&str; 5] = [
    "SessionStart",
    "PreToolUse",
    "UserPromptSubmit",
    "PostToolUse",
    "Stop",
];
pub const ROUTER_RS_HOOK_PROJECTION_VERSION: &str = "v1.0.0";
const INSTALL_STATUS_USER_PROMPT: &str = "Checking review/subagent gate";
const INSTALL_STATUS_SESSION_START: &str = "Loading Codex workspace context";
const INSTALL_STATUS_PRE_TOOL: &str = "Checking generated-surface guard";
const INSTALL_STATUS_POST_TOOL: &str = "Updating review/subagent gate state";
const INSTALL_STATUS_STOP: &str = "Enforcing review/subagent gate";
const CODEX_ADDITIONAL_CONTEXT_MAX_CHARS: usize = 640;
static ATOMIC_WRITE_NONCE: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
thread_local! {
    static FORCE_ATOMIC_WRITE_FAIL: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    Apply,
    Check,
}

#[derive(Debug, Clone)]
struct HooksMergeStat {
    status: &'static str,
    preserved_existing_entries: usize,
    added_entries: usize,
    removed_legacy_entries: usize,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
struct CodexReviewGateState {
    #[serde(default)]
    seq: i64,
    #[serde(default)]
    subagent_required: bool,
    #[serde(default)]
    review_required: bool,
    #[serde(default)]
    delegation_required: bool,
    #[serde(default)]
    review_override: bool,
    #[serde(default)]
    delegation_override: bool,
    #[serde(default)]
    reject_reason_seen: bool,
    #[serde(default)]
    review_subagent_seen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    review_subagent_tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
}

fn codex_state_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".codex").join("hook-state")
}

struct CodexStateLock {
    path: PathBuf,
}

impl Drop for CodexStateLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn codex_session_key(event: &Value) -> String {
    let raw = event
        .get("session_id")
        .or(event.get("conversation_id"))
        .or(event.get("thread_id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let now_ns = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let nonce = ATOMIC_WRITE_NONCE.fetch_add(1, Ordering::SeqCst);
            format!("invocation:{}:{now_ns}:{nonce}", std::process::id())
        });
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    let full_hex = digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    full_hex.chars().take(32).collect()
}

fn codex_state_path(repo_root: &Path, event: &Value) -> PathBuf {
    codex_state_dir(repo_root).join(format!("review-subagent-{}.json", codex_session_key(event)))
}

fn parse_lock_metadata(text: &str) -> (Option<u32>, Option<u64>) {
    let mut pid = None;
    let mut ts = None;
    for part in text.split_whitespace() {
        if let Some(value) = part.strip_prefix("pid=") {
            pid = value.parse::<u32>().ok();
        } else if let Some(value) = part.strip_prefix("ts=") {
            ts = value.parse::<u64>().ok();
        }
    }
    (pid, ts)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // Avoid spawning `kill` (PATH / sandbox failures must not look like "process dead").
    unsafe {
        let rc = libc::kill(pid as libc::pid_t, 0);
        if rc == 0 {
            return true;
        }
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::ESRCH) => false,
            Some(libc::EPERM) => true,
            _ => true,
        }
    }
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

fn lock_is_stale(path: &Path) -> bool {
    let text = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return true,
    };
    let (pid, ts) = parse_lock_metadata(&text);
    if pid.is_none() && ts.is_none() {
        if text.trim().is_empty() {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            if let Ok(meta) = fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    let modified_ms = modified
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    if now_ms.saturating_sub(modified_ms) <= 1_000 {
                        return false;
                    }
                }
            }
        }
        return true;
    }
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if let Some(process_id) = pid {
        if process_is_alive(process_id) {
            return false;
        }
    }
    ts.is_none_or(|t| now_ms.saturating_sub(t) > 30_000)
}

fn acquire_codex_state_lock(state_path: &Path) -> Result<CodexStateLock, String> {
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    let started = SystemTime::now();
    loop {
        let open = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path);
        match open {
            Ok(mut file) => {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let stamp = format!("pid={} ts={now_ms}\n", std::process::id());
                use std::io::Write as _;
                file.write_all(stamp.as_bytes())
                    .map_err(|err| format!("state_lock_write_failed: {err}"))?;
                file.sync_all()
                    .map_err(|err| format!("state_lock_sync_failed: {err}"))?;
                return Ok(CodexStateLock { path: lock_path });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                if started.elapsed().unwrap_or_else(|_| Duration::from_secs(0))
                    > Duration::from_secs(20)
                {
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(err) => return Err(format!("state_lock_acquire_failed: {err}")),
        }
    }
    Err("state_lock_timeout".to_string())
}

fn codex_load_state_from_path(path: &Path) -> Result<Option<CodexReviewGateState>, String> {
    let text = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("state_read_failed".to_string()),
    };
    let mut value: Value =
        serde_json::from_str(&text).map_err(|_| "state_json_invalid".to_string())?;
    if let Some(obj) = value.as_object_mut() {
        let schema_v1 = obj
            .get("schema_version")
            .and_then(Value::as_i64)
            .is_some_and(|v| v == 1);
        if schema_v1 {
            if obj
                .get("override")
                .and_then(Value::as_bool)
                .is_some_and(|v| v)
            {
                obj.entry("review_override".to_string())
                    .or_insert(json!(true));
                obj.entry("delegation_override".to_string())
                    .or_insert(json!(true));
            }
            if obj
                .get("delegation_required")
                .and_then(Value::as_bool)
                .is_some_and(|v| v)
                && !obj
                    .get("review_subagent_seen")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                obj.entry("seq".to_string()).or_insert(json!(1));
            }
        }
    }
    serde_json::from_value::<CodexReviewGateState>(value)
        .map(Some)
        .map_err(|_| "state_json_invalid".to_string())
}

#[cfg(test)]
fn codex_load_state(
    repo_root: &Path,
    event: &Value,
) -> Result<Option<CodexReviewGateState>, String> {
    codex_load_state_from_path(&codex_state_path(repo_root, event))
}

fn codex_save_state_to_path(state_path: &Path, state: &CodexReviewGateState) -> bool {
    let directory = state_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let target = state_path.to_path_buf();
    let mut payload = match serde_json::to_string_pretty(state) {
        Ok(value) => value,
        Err(_) => return false,
    };
    payload.push('\n');
    if fs::create_dir_all(&directory).is_err() {
        return false;
    }
    let file_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state.json");
    let mut tmp = None;
    let mut tmp_file = None;
    for _ in 0..64 {
        let nonce = ATOMIC_WRITE_NONCE.fetch_add(1, Ordering::Relaxed);
        let candidate = directory.join(format!(".tmp-{}-{file_name}-{nonce}", std::process::id()));
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(file) => {
                tmp = Some(candidate);
                tmp_file = Some(file);
                break;
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => return false,
        }
    }
    let Some(tmp) = tmp else {
        return false;
    };
    let Some(mut tmp_file) = tmp_file else {
        return false;
    };
    use std::io::Write as _;
    if tmp_file.write_all(payload.as_bytes()).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    if tmp_file.sync_all().is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    if fs::rename(&tmp, &target).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    #[cfg(unix)]
    if let Some(parent) = target.parent() {
        if let Ok(dir) = OpenOptions::new().read(true).open(parent) {
            let _ = dir.sync_all();
        }
    }
    true
}

fn with_codex_state_lock<T, F>(repo_root: &Path, event: &Value, f: F) -> Result<T, String>
where
    F: FnOnce(Option<CodexReviewGateState>) -> Result<(Option<CodexReviewGateState>, T), String>,
{
    let state_path = codex_state_path(repo_root, event);
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("state_dir_create_failed: {err}"))?;
    }
    let _guard = acquire_codex_state_lock(&state_path)?;
    let loaded = codex_load_state_from_path(&state_path)?;
    let (next_state, output) = f(loaded)?;
    if let Some(state) = next_state {
        if !codex_save_state_to_path(&state_path, &state) {
            return Err("state_write_failed".to_string());
        }
    }
    Ok(output)
}

fn codex_prompt_text(event: &Value) -> String {
    for key in ["prompt", "user_prompt", "message", "input"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    String::new()
}

fn codex_tool_name(event: &Value) -> String {
    event
        .get("tool_name")
        .or(event.get("tool"))
        .or(event.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn codex_tool_input(event: &Value) -> Value {
    event
        .get("tool_input")
        .or(event.get("input"))
        .or(event.get("arguments"))
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn saw_subagent_codex(tool_name: &str, tool_input: &Value) -> bool {
    let name = normalize_tool_name(Some(tool_name));
    if !CODEX_REVIEW_SUBAGENT_TOOL_NAMES.contains(&name.as_str()) {
        return false;
    }
    codex_subagent_type_evidence(tool_input)
}

fn codex_subagent_type_evidence(tool_input: &Value) -> bool {
    let typed_fields = [
        tool_input.get("subagent_type").and_then(Value::as_str),
        tool_input.get("agent_type").and_then(Value::as_str),
        tool_input.get("agentType").and_then(Value::as_str),
        tool_input.get("type").and_then(Value::as_str),
    ];
    typed_fields.iter().any(|field| {
        let normalized = normalize_subagent_type(*field);
        CODEX_REVIEW_SUBAGENT_TYPES.contains(&normalized.as_str())
    })
}

fn codex_compact_contexts(parts: Vec<String>) -> Option<String> {
    let mut dedup = HashSet::new();
    let mut unique = Vec::new();
    for part in parts {
        let normalized = part.trim();
        if normalized.is_empty() {
            continue;
        }
        let key = normalized.to_ascii_lowercase();
        if dedup.insert(key) {
            unique.push(normalized.to_string());
        }
    }
    if unique.is_empty() {
        return None;
    }
    let combined = unique.join("\n");
    if combined.len() <= CODEX_ADDITIONAL_CONTEXT_MAX_CHARS {
        return Some(combined);
    }
    let mut truncated: String = combined
        .chars()
        .take(CODEX_ADDITIONAL_CONTEXT_MAX_CHARS.saturating_sub(3))
        .collect();
    truncated.push_str("...");
    Some(truncated)
}

fn handle_codex_userpromptsubmit(repo_root: &Path, event: &Value) -> Option<Value> {
    let text = codex_prompt_text(event);
    // Per-turn reset is intentional: Stop resets state for clean turn boundary,
    // UserPromptSubmit re-derives required-flags from prompt text. We carry
    // over only `seq` (monotonic conversation counter for telemetry) so it
    // doesn't roll back to 0 every turn, while still wiping per-turn evidence
    // (`review_subagent_seen`, `review_subagent_tool`, etc.) that must be
    // re-established within the new turn.
    let mut state = CodexReviewGateState {
        seq: 0,
        subagent_required: true,
        ..CodexReviewGateState::default()
    };

    if is_review_prompt(&text) {
        state.review_required = true;
        state.prompt = Some(text.chars().take(500).collect());
    }
    if is_parallel_delegation_prompt(&text) {
        state.delegation_required = true;
        state.prompt = Some(text.chars().take(500).collect());
    }
    if has_override(&text) || has_review_override(&text) || has_delegation_override(&text) {
        state.review_override = true;
        state.delegation_override = true;
    }
    if saw_reject_reason(&text) {
        state.reject_reason_seen = true;
    }

    let write_result = with_codex_state_lock(repo_root, event, |loaded| {
        let mut next = state.clone();
        if let Some(prev) = loaded {
            next.seq = prev.seq.saturating_add(1);
        } else {
            next.seq = 1;
        }
        Ok((Some(next), ()))
    });
    if write_result.is_err() {
        return Some(json!({
            "decision": "block",
            "reason": "Review gate state could not be persisted under .codex/hook-state. Fail-closed to avoid silent policy bypass.",
        }));
    }

    let mut contexts: Vec<String> = Vec::new();
    if state.subagent_required && !state.review_override {
        contexts.push(
            "Subagent gate active: start a bounded subagent lane first, or explicitly record one reject reason (small_task/shared_context_heavy/write_scope_overlap/next_step_blocked/verification_missing/token_overhead_dominates).".to_string()
        );
    }
    if state.review_required && !state.review_override {
        contexts.push(
            "Review gate active: launch independent reviewer lanes (e.g., security/architecture/regression) before concluding analysis, or record one reject reason.".to_string()
        );
    }
    if state.delegation_required && !state.delegation_override {
        contexts.push(
            "Parallel request detected: split work into bounded lanes before integration, or record one reject reason.".to_string()
        );
    }
    if !state.review_override && !state.delegation_override && !state.reject_reason_seen {
        if let Some(warning) = codex_projection_drift_warning(repo_root) {
            contexts.push(warning);
        }
    }

    let additional_context = codex_compact_contexts(contexts);
    if additional_context.is_none() {
        None
    } else {
        Some(json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": additional_context,
            }
        }))
    }
}

fn handle_codex_posttooluse(repo_root: &Path, event: &Value) -> Option<Value> {
    if let Err(err) = try_append_codex_post_tool_evidence(repo_root, event) {
        eprintln!("[router-rs] post-tool evidence append failed (non-fatal): {err}");
    }
    let tool_name = codex_tool_name(event);
    let tool_input = codex_tool_input(event);
    if !saw_subagent_codex(&tool_name, &tool_input) {
        return None;
    }
    match with_codex_state_lock(repo_root, event, |loaded| {
        let mut state = match loaded {
            Some(value) => value,
            None => return Err("state_missing".to_string()),
        };
        state.review_subagent_seen = true;
        state.review_subagent_tool = tool_input
            .get("subagent_type")
            .or(tool_input.get("agent_type"))
            .or(tool_input.get("agentType"))
            .or(tool_input.get("type"))
            .and_then(Value::as_str)
            .map(|s| normalize_subagent_type(Some(s)))
            .map(|kind| format!("{tool_name}#{kind}"))
            .or(Some(tool_name.clone()));
        Ok((Some(state), ()))
    }) {
        Ok(()) => None,
        Err(err) if err == "state_missing" => Some(json!({
            "decision": "block",
            "reason": "Review gate state is missing under .codex/hook-state during PostToolUse subagent evidence update. Fail-closed to avoid inconsistent gating.",
        })),
        Err(err) => Some(json!({
            "decision": "block",
            "reason": format!(
                "Review gate state is unreadable during PostToolUse subagent evidence update under .codex/hook-state ({}). Fail-closed to avoid silent policy bypass.",
                err
            ),
        })),
    }
}

fn handle_codex_stop(repo_root: &Path, event: &Value) -> Option<Value> {
    if event
        .get("stop_hook_active")
        .or(event.get("stopHookActive"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    let text = codex_prompt_text(event);
    let inferred_overridden = has_override(&text)
        || has_review_override(&text)
        || has_delegation_override(&text)
        || saw_reject_reason(&text);
    let inferred_required = !text.trim().is_empty() && !inferred_overridden;
    match with_codex_state_lock(repo_root, event, |loaded| {
        let state = match loaded {
            Some(value) => value,
            None => return Err("state_missing".to_string()),
        };
        if state.subagent_required
            && !state.review_override
            && !state.reject_reason_seen
            && !state.review_subagent_seen
        {
            return Ok((
                None,
                Some(json!({
                    "decision": "block",
                    "reason": "Default subagent policy is active, but no subagent was observed. Spawn a bounded subagent lane now, or explicitly record a reject reason (small_task/shared_context_heavy/write_scope_overlap/next_step_blocked/verification_missing/token_overhead_dominates).",
                })),
            ));
        }
        if state.review_required
            && !state.review_override
            && !state.reject_reason_seen
            && !state.review_subagent_seen
        {
            return Ok((
                None,
                Some(json!({
                    "decision": "block",
                    "reason": "Broad/deep review was requested, but no independent subagent review was observed. Spawn suitable reviewer sidecars now, or explicitly record why spawning is rejected.",
                })),
            ));
        }
        if state.delegation_required
            && !state.delegation_override
            && !state.reject_reason_seen
            && !state.review_subagent_seen
        {
            return Ok((
                None,
                Some(json!({
                    "decision": "block",
                    "reason": "Independent parallel lanes were requested, but no bounded subagent sidecar was observed. Spawn suitable sidecars before finalizing, or rerun with an explicit no-subagent override.",
                })),
            ));
        }
        let reset = CodexReviewGateState {
            seq: 0,
            ..CodexReviewGateState::default()
        };
        Ok((Some(reset), None))
    }) {
        Ok(output) => {
            if output.is_none() {
                try_write_continuity_checkpoint_on_codex_stop(repo_root, event);
            }
            output
        }
        Err(err) if err == "state_missing" => {
            let mut reason = "Review gate state is missing under .codex/hook-state. Fail-closed to avoid bypass when enforcement state is unavailable.".to_string();
            if !inferred_required {
                reason.push_str(" Stop payload had no review context; blocking conservatively.");
            }
            Some(json!({
                "decision": "block",
                "reason": reason,
            }))
        }
        Err(io_error) => Some(json!({
            "decision": "block",
            "reason": format!(
                "Review gate state is unreadable or unavailable under .codex/hook-state ({}). Fail-closed to avoid silent policy bypass.",
                io_error
            ),
        })),
    }
}

fn continuity_stop_checkpoint_env_enabled() -> bool {
    match env::var("ROUTER_RS_CONTINUITY_STOP_CHECKPOINT") {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            !(token == "0" || token == "false" || token == "off" || token == "no")
        }
        Err(_) => true,
    }
}

/// Codex Stop 守门通过后写入 `artifacts/current/*` 与指针文件；失败不阻断 Stop（仅 stderr）。
fn try_write_continuity_checkpoint_on_codex_stop(repo_root: &Path, event: &Value) {
    if !continuity_stop_checkpoint_env_enabled() {
        return;
    }
    let text = codex_prompt_text(event);
    let task_line = text.lines().next().unwrap_or("").trim().to_string();
    let summary_body = if text.trim().is_empty() {
        "Stop hook automatic checkpoint. Stop payload had no user prompt text; refine SESSION_SUMMARY manually or rely on prior turns.".to_string()
    } else {
        text
    };
    let payload =
        build_automatic_continuity_checkpoint_payload(repo_root, &task_line, &summary_body);
    if let Err(err) = write_framework_session_artifacts(payload) {
        eprintln!("[router-rs] continuity checkpoint write failed (non-fatal): {err}");
    }
}

fn handle_codex_session_start(repo_root: &Path, payload: &Value) -> Option<Value> {
    let source = payload
        .get("source")
        .or(payload.get("matcher"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut contexts = vec![
        format!(
            "Codex workspace context: policy root={}, routing truth=skills/SKILL_ROUTING_RUNTIME.json, active state=.supervisor_state.json.",
            repo_root.display()
        ),
    ];
    // Keep refresh compact: SessionStart additionalContext is capped (~640 chars) by codex_compact_contexts.
    if let Ok(refresh) = build_framework_refresh_payload(repo_root, 4, false) {
        if let Some(prompt) = refresh.get("prompt").and_then(Value::as_str) {
            if !prompt.trim().is_empty() {
                contexts.push(format!("Continuity digest:\n{}", prompt.trim()));
            }
        }
    }
    contexts.push(
        "Hook policy: inject only short live state; keep long-lived rules in AGENTS.md and generated/runtime truth in skills/."
            .to_string(),
    );
    if !source.trim().is_empty() {
        contexts.push(format!("SessionStart source: {source}."));
    }
    let additional_context = codex_compact_contexts(contexts)?;
    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context,
        }
    }))
}

fn run_codex_review_subagent_gate(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    if !payload.is_object() {
        return Ok(Some(review_gate_input_error(
            "Review gate input schema invalid: expected a JSON object payload.",
        )));
    }
    let event_name = payload
        .get("hook_event_name")
        .or(payload.get("event"))
        .and_then(Value::as_str)
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    Ok(match event_name.as_str() {
        "sessionstart" => handle_codex_session_start(repo_root, payload),
        "userpromptsubmit" => handle_codex_userpromptsubmit(repo_root, payload),
        "posttooluse" => handle_codex_posttooluse(repo_root, payload),
        "stop" => handle_codex_stop(repo_root, payload),
        "" => Some(review_gate_input_error(
            "Review gate input schema invalid: missing hook_event_name/event.",
        )),
        other => Some(review_gate_input_error(&format!(
            "Review gate input schema invalid: unsupported hook_event_name/event `{other}`.",
        ))),
    })
}
pub fn build_codex_hook_manifest() -> Value {
    let mut hooks = serde_json::Map::new();
    for event in INSTALL_EVENTS {
        let timeout = match event {
            "SessionStart" => 3,
            "PostToolUse" => 5,
            _ => 8,
        };
        let mut hook = json!({
            "type": "command",
            "command": build_project_hook_command(event),
            "timeout": timeout,
            "statusMessage": hook_event_status_message(event),
        });
        if event == "Stop" {
            hook["loop_limit"] = json!(3);
        }
        let mut entry = json!({
            "hooks": [hook],
        });
        if event == "SessionStart" {
            entry["matcher"] = json!("startup|resume|clear");
        }
        hooks.insert(event.to_string(), json!([entry]));
    }
    json!({
        "version": 1,
        "_comment": "Managed by router-rs. Regenerate with `router-rs codex sync --repo-root \"$PWD\"`.",
        "hooks": hooks,
    })
}

struct HostEntrypointSyncSection {
    text_files: Vec<String>,
    json_files: Vec<String>,
}

fn host_entrypoint_partial_sync_section(
    desired_files: &BTreeMap<String, Vec<u8>>,
) -> HostEntrypointSyncSection {
    HostEntrypointSyncSection {
        text_files: desired_host_entrypoint_text_files(desired_files),
        json_files: HOST_ENTRYPOINT_JSON_RELATIVE_PATHS
            .iter()
            .map(|path| (*path).to_string())
            .collect(),
    }
}

#[derive(Default)]
struct SingleSyncReport {
    written: Vec<String>,
    would_write: Vec<String>,
    unchanged: Vec<String>,
    created_dirs: Vec<String>,
}

pub(crate) fn sync_host_entrypoints(repo_root: &Path, apply: bool) -> Result<Value, String> {
    let root = normalize_repo_root(repo_root)?;
    let desired_files = build_host_entrypoint_files(&root)?;
    let partial_section = host_entrypoint_partial_sync_section(&desired_files);
    let (matched_worktrees, skipped_worktrees) = discover_matching_worktrees(&root);
    let mut report = json!({
        "written": [],
        "would_write": [],
        "unchanged": [],
        "created_dirs": [],
        "synced_worktrees": [],
        "skipped_worktrees": skipped_worktrees,
    });
    let full_text_files = desired_host_entrypoint_text_files(&desired_files);
    let mut full_json_files = HOST_ENTRYPOINT_JSON_RELATIVE_PATHS.to_vec();
    full_json_files.push(HOST_ENTRYPOINT_SYNC_MANIFEST_PATH);
    let full_section = HostEntrypointSyncSection {
        text_files: full_text_files
            .into_iter()
            .map(|path| path.to_string())
            .collect(),
        json_files: full_json_files
            .into_iter()
            .map(|path| path.to_string())
            .collect(),
    };
    let mut targets = vec![root.clone()];
    targets.extend(matched_worktrees);

    for target_root in targets {
        let section = if target_root == root {
            &full_section
        } else {
            &partial_section
        };
        let single = match sync_host_entrypoints_single_root(
            &desired_files,
            &target_root,
            &root,
            apply,
            section,
        ) {
            Ok(single) => single,
            Err(err) if target_root != root => {
                extend_report_array(
                    &mut report,
                    "skipped_worktrees",
                    vec![format!("{} ({err})", target_root.to_string_lossy())],
                )?;
                continue;
            }
            Err(err) => return Err(err),
        };
        extend_report_array(&mut report, "written", single.written)?;
        extend_report_array(&mut report, "would_write", single.would_write)?;
        extend_report_array(&mut report, "unchanged", single.unchanged)?;
        extend_report_array(&mut report, "created_dirs", single.created_dirs)?;
        if target_root != root {
            extend_report_array(
                &mut report,
                "synced_worktrees",
                vec![target_root.to_string_lossy().into_owned()],
            )?;
        }
    }

    sort_report_array(&mut report, "written")?;
    sort_report_array(&mut report, "would_write")?;
    sort_report_array(&mut report, "unchanged")?;
    sort_report_array(&mut report, "created_dirs")?;
    sort_report_array(&mut report, "synced_worktrees")?;
    sort_report_array(&mut report, "skipped_worktrees")?;
    Ok(report)
}

fn build_host_entrypoint_files(_repo_root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    files.insert(
        CODEX_AGENT_POLICY_PATH.to_string(),
        build_codex_agent_policy().into_bytes(),
    );
    files.insert(
        CODEX_HOOKS_PATH.to_string(),
        serialize_pretty_json_bytes(&build_codex_hook_manifest())?,
    );
    files.insert(
        CODEX_HOOKS_README_PATH.to_string(),
        build_codex_hooks_readme().into_bytes(),
    );
    files.insert(
        HOST_ENTRYPOINT_SYNC_MANIFEST_PATH.to_string(),
        serialize_pretty_json_bytes(&build_host_entrypoint_sync_manifest(&files))?,
    );
    Ok(files)
}

fn build_host_entrypoint_sync_manifest(desired_files: &BTreeMap<String, Vec<u8>>) -> Value {
    let full_text_files = desired_host_entrypoint_text_files(desired_files);
    json!({
        "schema_version": "host-entrypoints-sync-manifest-v1",
        "shared_system": {
            "policy": "host-specific-agent-policy-v1",
            "source_of_truth": "skills/",
            "supported_hosts": ["codex-cli", "cursor"],
            "host_entrypoints": {
                "codex-cli": CODEX_AGENT_POLICY_PATH,
                "cursor": CODEX_AGENT_POLICY_PATH,
            },
        },
        "full_sync": {
            "text_files": full_text_files,
            "json_files": [
                HOST_ENTRYPOINT_SYNC_MANIFEST_PATH,
            ],
        },
        "partial_sync": {
            "text_files": full_text_files,
            "json_files": HOST_ENTRYPOINT_JSON_RELATIVE_PATHS,
        },
    })
}

fn desired_host_entrypoint_text_files(desired_files: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    desired_files
        .keys()
        .filter(|path| path.as_str() != HOST_ENTRYPOINT_SYNC_MANIFEST_PATH)
        .filter(|path| !HOST_ENTRYPOINT_JSON_RELATIVE_PATHS.contains(&path.as_str()))
        .filter(|path| path.as_str() != CODEX_HOOKS_README_PATH)
        .cloned()
        .collect()
}

fn serialize_pretty_json_bytes(payload: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(payload).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sync_host_entrypoints_single_root(
    desired_files: &BTreeMap<String, Vec<u8>>,
    target_root: &Path,
    report_root: &Path,
    apply: bool,
    section: &HostEntrypointSyncSection,
) -> Result<SingleSyncReport, String> {
    let mut report = SingleSyncReport::default();
    for relative in section.text_files.iter().chain(section.json_files.iter()) {
        let desired = desired_files
            .get(relative)
            .ok_or_else(|| format!("missing generated host-entrypoint payload for {}", relative))?;
        sync_host_entrypoint_file(
            desired,
            relative,
            target_root,
            report_root,
            apply,
            &mut report,
        )?;
    }

    Ok(report)
}

fn protected_generated_paths() -> Vec<&'static str> {
    PROTECTED_GENERATED_PATHS.to_vec()
}

fn sync_host_entrypoint_file(
    desired: &[u8],
    relative: &str,
    target_root: &Path,
    report_root: &Path,
    apply: bool,
    report: &mut SingleSyncReport,
) -> Result<(), String> {
    let destination = target_root.join(relative);
    let existing = fs::read(&destination).ok();
    let changed = existing.as_deref() != Some(desired);
    if changed && apply {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&destination, desired).map_err(|err| err.to_string())?;
    }
    let bucket = if changed && apply {
        &mut report.written
    } else if changed {
        &mut report.would_write
    } else {
        &mut report.unchanged
    };
    bucket.push(describe_host_entrypoint_path(
        report_root,
        target_root,
        &destination,
    ));
    Ok(())
}

fn extend_report_array(report: &mut Value, key: &str, items: Vec<String>) -> Result<(), String> {
    let array = report
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("host-entrypoint sync report missing {key} array"))?;
    array.extend(items.into_iter().map(Value::String));
    Ok(())
}

fn sort_report_array(report: &mut Value, key: &str) -> Result<(), String> {
    let array = report
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("host-entrypoint sync report missing {key} array"))?;
    let mut values = array
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    *array = values.into_iter().map(Value::String).collect();
    Ok(())
}

fn normalize_repo_root(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .map_err(|err| err.to_string())?
            .join(path))
    }
}

fn discover_matching_worktrees(root: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let worktree_listing = read_git_stdout(root, &["worktree", "list", "--porcelain"]);
    if worktree_listing.is_none() {
        return (Vec::new(), Vec::new());
    }

    let mut current: BTreeMap<String, String> = BTreeMap::new();
    let mut worktrees = Vec::new();
    for raw_line in worktree_listing.unwrap_or_default().lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                worktrees.push(current);
                current = BTreeMap::new();
            }
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or_default().to_string();
        let value = parts.next().unwrap_or_default().to_string();
        current.insert(key, value);
    }
    if !current.is_empty() {
        worktrees.push(current);
    }

    let mut matches = Vec::new();
    let mut skipped = Vec::new();
    for entry in worktrees {
        let Some(worktree_path) = entry.get("worktree") else {
            continue;
        };
        let candidate = normalize_repo_root(Path::new(worktree_path))
            .unwrap_or_else(|_| PathBuf::from(worktree_path));
        if candidate == root {
            continue;
        }
        if !candidate.exists() {
            skipped.push(format!("{} (missing)", candidate.to_string_lossy()));
            continue;
        }
        matches.push(candidate);
    }
    (matches, skipped)
}

fn read_git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn describe_host_entrypoint_path(report_root: &Path, target_root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(report_root) {
        return relative.to_string_lossy().into_owned();
    }
    if let Ok(relative) = path.strip_prefix(target_root) {
        return format!(
            "{}::{}",
            target_root.to_string_lossy(),
            relative.to_string_lossy()
        );
    }
    path.to_string_lossy().into_owned()
}

pub fn build_codex_hook_projection() -> Value {
    json!({
        "schema_version": "router-rs-codex-hook-projection-v1",
        "authority": CODEX_HOOK_AUTHORITY,
        "codex_agent_policy": build_codex_agent_policy(),
        "codex_hooks_readme": build_codex_hooks_readme(),
        "codex_hooks": build_codex_hook_manifest(),
        "codex_audit_commands": {
            "pre_tool_use": build_codex_hook_command("--event=PreToolUse"),
            "contract_guard": build_codex_hook_command("contract-guard"),
            "review_subagent_gate": build_codex_hook_command("review-subagent-gate"),
        },
    })
}

fn build_codex_agent_policy() -> String {
    include_str!("../../../AGENTS.md").to_string()
}

fn build_codex_hooks_readme() -> String {
    "# Codex Hooks Projection\n\n\
Codex hooks are enabled for this repo and are managed by the Rust `router-rs` control plane.\n\n\
Project-local `.codex/hooks.json` uses the official Codex lifecycle surface: `SessionStart`, `PreToolUse`, `UserPromptSubmit`, `PostToolUse`, and `Stop`.\n\n\
`SessionStart` injects workspace pointer plus a short continuity digest when `artifacts/current/` is populated, `UserPromptSubmit` injects only trigger-specific context, `PreToolUse` blocks direct edits to generated Codex surfaces, `PostToolUse` updates review gate state for subagent tools and appends verification-like shell commands (for example `cargo test`) to `EVIDENCE_INDEX.json` when continuity is active (disable with `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`), and `Stop` enforces review gates then (when unblocked) writes an automatic in-progress continuity checkpoint under `artifacts/current/` unless `ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=0`. Durable cleanup should use explicit refresh commands rather than an extra end-of-session hook.\n\n\
Hook state is transient and lives under `.codex/hook-state/` in the current repository while the session is active.\n\n\
Use `scripts/install_codex_cli_hooks.sh` only when you want to install the same Codex hook projection into a user-level `~/.codex/hooks.json`. The installer keeps existing hooks and idempotently appends the managed command hook without replacing unrelated handlers.\n\n\
Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.\n\n\
Regenerate with:\n\n\
```sh\n\
router-rs codex sync --repo-root \"$PWD\"\n\
```\n"
        .to_string()
}

fn build_hook_binary_preamble(
    project_var: &str,
    env_var: &str,
    missing_binary_fallback: &str,
) -> String {
    let mut command = String::new();
    command.push_str(&format!(
        "{project_var}=\"${{{env_var}:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}\"; "
    ));
    command.push_str(&format!(
        "ROUTER_RS_BIN=\"\"; \
if [ -x \"${project_var}/scripts/router-rs/target/release/router-rs\" ]; then ROUTER_RS_BIN=\"${project_var}/scripts/router-rs/target/release/router-rs\"; \
elif [ -x \"${project_var}/scripts/router-rs/target/debug/router-rs\" ]; then ROUTER_RS_BIN=\"${project_var}/scripts/router-rs/target/debug/router-rs\"; \
elif [ -x \"${project_var}/target/release/router-rs\" ]; then ROUTER_RS_BIN=\"${project_var}/target/release/router-rs\"; \
elif [ -x \"${project_var}/target/debug/router-rs\" ]; then ROUTER_RS_BIN=\"${project_var}/target/debug/router-rs\"; \
else ROUTER_RS_BIN=\"$(command -v router-rs 2>/dev/null || true)\"; fi; "
    ));
    command.push_str("if [ ! -x \"$ROUTER_RS_BIN\" ]; then ");
    command.push_str(missing_binary_fallback);
    command.push_str("; fi; ");
    command
}

fn build_codex_hook_command(event: &str) -> String {
    let mut command =
        build_hook_binary_preamble("CODEX_PROJECT_ROOT", "CODEX_PROJECT_ROOT", "printf '%s\\n' '{\"decision\":\"block\",\"message\":\"router-rs binary unavailable for Codex hook\",\"reason\":\"router-rs binary unavailable; fail-closed instead of silently bypassing critical hook enforcement\"}'; exit 1");
    command.push_str(&format!(
        "\"$ROUTER_RS_BIN\" codex hook {event} --repo-root \"$CODEX_PROJECT_ROOT\""
    ));
    command
}

fn build_project_hook_command(event: &str) -> String {
    build_install_hook_command(Path::new("."), event)
}

struct InstallLock {
    path: PathBuf,
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_install_lock(codex_home: &Path) -> Result<InstallLock, String> {
    let lock_path = codex_home.join(".install.lock");
    for _ in 0..30 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let stamp = format!("pid={} ts={now_ms}\n", std::process::id());
                use std::io::Write as _;
                file.write_all(stamp.as_bytes())
                    .map_err(|err| format!("install_lock_write_failed: {err}"))?;
                file.sync_all()
                    .map_err(|err| format!("install_lock_sync_failed: {err}"))?;
                return Ok(InstallLock { path: lock_path });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(format!("install_lock_acquire_failed: {err}")),
        }
    }
    Err("install_lock_timeout".to_string())
}

fn projection_version_older(manifest_version: &str, current: &str) -> bool {
    fn parse(value: &str) -> Option<(u64, u64, u64)> {
        let cleaned = value.trim().trim_start_matches('v');
        let mut parts = cleaned.split('.');
        Some((
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        ))
    }
    match (parse(manifest_version), parse(current)) {
        (Some(found), Some(expected)) => found < expected,
        _ => true,
    }
}

fn codex_projection_drift_warning(repo_root: &Path) -> Option<String> {
    let warning = "[router-rs] hook projection drift detected; consider re-running scripts/install_codex_cli_hooks.sh.".to_string();
    let local_codex_home = repo_root.join("codex-home");
    let manifest_path = if local_codex_home.is_dir() {
        local_codex_home.join(".router-rs-install.manifest.json")
    } else {
        resolve_codex_home(None)
            .ok()?
            .join(".router-rs-install.manifest.json")
    };
    let text = match fs::read_to_string(manifest_path) {
        Ok(v) => v,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(_) => return Some(warning),
    };
    let manifest: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Some(warning),
    };
    let projection = manifest
        .get("projection_version")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if projection_version_older(projection, ROUTER_RS_HOOK_PROJECTION_VERSION) {
        return Some(warning);
    }
    None
}

pub fn resolve_codex_home(arg: Option<&Path>) -> Result<PathBuf, String> {
    let candidate = if let Some(path) = arg {
        path.to_path_buf()
    } else if let Some(path) = env::var_os("CODEX_HOME") {
        PathBuf::from(path)
    } else if let Some(home) = env::var_os("HOME") {
        PathBuf::from(home).join(".codex")
    } else {
        return Err(
            "Could not resolve codex home: missing --codex-home, CODEX_HOME, and HOME".to_string(),
        );
    };
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        env::current_dir()
            .map_err(|err| format!("Could not resolve current directory: {err}"))?
            .join(candidate)
    };
    fs::create_dir_all(&absolute)
        .map_err(|err| format!("Failed to create codex home {}: {err}", absolute.display()))?;
    absolute.canonicalize().map_err(|err| {
        format!(
            "Failed to canonicalize codex home {}: {err}",
            absolute.display()
        )
    })
}

pub fn install_codex_cli_hooks(
    codex_home: &Path,
    repo_root: &Path,
    mode: InstallMode,
) -> Result<Value, String> {
    let apply = matches!(mode, InstallMode::Apply);
    let resolved_codex_home = resolve_codex_home(Some(codex_home))?;
    let resolved_repo_root = if repo_root.is_absolute() {
        repo_root.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|err| format!("Could not resolve current directory: {err}"))?
            .join(repo_root)
    };
    let resolved_repo_root = resolved_repo_root.canonicalize().map_err(|err| {
        format!(
            "Failed to canonicalize repo root {}: {err}",
            resolved_repo_root.display()
        )
    })?;
    if !resolved_repo_root.exists() {
        return Err(format!(
            "Repo root does not exist: {}",
            resolved_repo_root.display()
        ));
    }

    let config_path = resolved_codex_home.join("config.toml");
    let hooks_path = resolved_codex_home.join("hooks.json");
    let hook_commands = INSTALL_EVENTS
        .iter()
        .map(|event| {
            (
                (*event).to_string(),
                build_install_hook_command(&resolved_repo_root, event),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let command_digest = sha256_hex(&serialize_ascii_json_pretty(&json!(hook_commands))?);
    let _install_guard = if apply {
        Some(acquire_install_lock(&resolved_codex_home)?)
    } else {
        None
    };

    let existing_config = fs::read_to_string(&config_path).ok();
    let (merged_config, config_status) = merge_features_codex_hooks(existing_config.as_deref());
    let config_changed = existing_config.as_deref() != Some(merged_config.as_str());
    if apply && config_changed {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create config parent directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        write_atomic_text(&config_path, &merged_config)?;
    }

    let hooks_existed = hooks_path.exists();
    if apply
        && hooks_existed
        && fs::symlink_metadata(&hooks_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err(format!(
            "Refusing to update symlinked hooks.json: {}",
            hooks_path.display()
        ));
    }
    let hooks_text = fs::read_to_string(&hooks_path).ok();
    let hooks_value = if let Some(text) = hooks_text.as_deref() {
        Some(
            serde_json::from_str::<Value>(text)
                .map_err(|err| format!("Failed to parse {}: {err}", hooks_path.display()))?,
        )
    } else {
        None
    };
    let (merged_hooks, hooks_stat) = merge_hooks_json(hooks_value, &hook_commands)?;
    let hooks_serialized = serialize_ascii_json_pretty(&merged_hooks)?;
    let hooks_changed = hooks_text.as_deref() != Some(hooks_serialized.as_str());
    let mut backup_path: Option<PathBuf> = None;

    if apply && hooks_changed {
        if let Some(parent) = hooks_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "Failed to create hooks parent directory {}: {err}",
                    parent.display()
                )
            })?;
        }
        if hooks_existed {
            let backup = PathBuf::from(format!(
                "{}.bak.{}",
                hooks_path.display(),
                Utc::now().format("%Y%m%d%H%M%S")
            ));
            fs::copy(&hooks_path, &backup).map_err(|err| {
                format!(
                    "Failed to backup hooks {} -> {}: {err}",
                    hooks_path.display(),
                    backup.display()
                )
            })?;
            backup_path = Some(backup);
        }
        let write_result = write_atomic_text(&hooks_path, &hooks_serialized);
        if let Err(err) = write_result {
            if let Some(backup) = backup_path.as_ref() {
                let _ = fs::copy(backup, &hooks_path);
            }
            return Err(err);
        }
        if !hooks_existed {
            #[cfg(unix)]
            {
                let _ = fs::set_permissions(&hooks_path, fs::Permissions::from_mode(0o644));
            }
        }
    }
    if apply {
        let manifest = json!({
            "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
            "command_digest": command_digest,
        });
        let manifest_text = serialize_ascii_json_pretty(&manifest)?;
        write_atomic_text(
            &resolved_codex_home.join(".router-rs-install.manifest.json"),
            &manifest_text,
        )?;
    }

    Ok(json!({
        "schema_version": "router-rs-codex-install-hooks-v1",
        "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
        "command_digest": command_digest,
        "authority": "rust-codex-install-hooks",
        "codex_home": resolved_codex_home.to_string_lossy().into_owned(),
        "repo_root": resolved_repo_root.to_string_lossy().into_owned(),
        "applied": apply,
        "config_toml": {
            "path": config_path.to_string_lossy().into_owned(),
            "status": mode_status(config_status, mode),
        },
        "hooks_json": {
            "path": hooks_path.to_string_lossy().into_owned(),
            "status": mode_status(hooks_stat.status, mode),
            "events": INSTALL_EVENTS,
            "preserved_existing_entries": hooks_stat.preserved_existing_entries,
            "added_entries": hooks_stat.added_entries,
            "removed_legacy_entries": hooks_stat.removed_legacy_entries,
            "backup_path": backup_path.map(|v| v.to_string_lossy().into_owned()),
        },
        "hook_commands": hook_commands,
    }))
}

fn mode_status(status: &'static str, mode: InstallMode) -> &'static str {
    match mode {
        InstallMode::Apply => status,
        InstallMode::Check => match status {
            "created" => "would-create",
            "updated" => "would-update",
            "unchanged" => "would-leave-unchanged",
            _ => "would-update",
        },
    }
}

fn write_atomic_text(path: &Path, text: &str) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("atomic-write-target");
    let ts_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let nonce = ATOMIC_WRITE_NONCE.fetch_add(1, Ordering::Relaxed);
    let tmp_path = parent.join(format!(
        ".{stem}.tmp-{}-{ts_nanos}-{nonce}",
        std::process::id()
    ));
    #[cfg(test)]
    if FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.get()) {
        return Err("forced atomic write failure".to_string());
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&tmp_path)
        .map_err(|err| {
            format!(
                "Failed to write temporary file {}: {err}",
                tmp_path.display()
            )
        })?;
    use std::io::Write as _;
    file.write_all(text.as_bytes()).map_err(|err| {
        format!(
            "Failed to write temporary file {}: {err}",
            tmp_path.display()
        )
    })?;
    file.sync_all().map_err(|err| {
        format!(
            "Failed to fsync temporary file {}: {err}",
            tmp_path.display()
        )
    })?;
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "Failed to replace {} with {}: {err}",
            path.display(),
            tmp_path.display()
        )
    })?;
    #[cfg(unix)]
    if let Ok(dir) = OpenOptions::new().read(true).open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

fn serialize_ascii_json_pretty(value: &Value) -> Result<String, String> {
    let pretty = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    let mut out = String::with_capacity(pretty.len() + 1);
    for ch in pretty.chars() {
        if ch.is_ascii() {
            out.push(ch);
            continue;
        }
        let mut buf = [0u16; 2];
        for unit in ch.encode_utf16(&mut buf).iter() {
            out.push_str(&format!("\\u{:04x}", unit));
        }
    }
    out.push('\n');
    Ok(out)
}

fn hook_event_status_message(event_name: &str) -> &'static str {
    match event_name {
        "SessionStart" => INSTALL_STATUS_SESSION_START,
        "PreToolUse" => INSTALL_STATUS_PRE_TOOL,
        "UserPromptSubmit" => INSTALL_STATUS_USER_PROMPT,
        "PostToolUse" => INSTALL_STATUS_POST_TOOL,
        "Stop" => INSTALL_STATUS_STOP,
        _ => "",
    }
}

fn build_install_hook_command(_repo_root: &Path, event: &str) -> String {
    let audit_command = format!("--event={event}");
    let missing_binary_fallback = if matches!(event, "SessionStart" | "PostToolUse") {
        "echo \"[codex-hook] router-rs binary missing; state update skipped\" >&2; exit 0"
    } else {
        "printf '%s\\n' '{{\"decision\":\"block\",\"message\":\"router-rs binary unavailable for Codex hook\",\"reason\":\"router-rs binary unavailable; fail-closed instead of silently bypassing critical hook enforcement\"}}'; exit 1"
    };
    format!(
        "/usr/bin/env bash -lc 'CODEX_PROJECT_ROOT=\"${{CODEX_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}}\"; ROUTER_RS_BIN=\"\"; if [ -x \"$CODEX_PROJECT_ROOT/scripts/router-rs/target/release/router-rs\" ]; then ROUTER_RS_BIN=\"$CODEX_PROJECT_ROOT/scripts/router-rs/target/release/router-rs\"; elif [ -x \"$CODEX_PROJECT_ROOT/scripts/router-rs/target/debug/router-rs\" ]; then ROUTER_RS_BIN=\"$CODEX_PROJECT_ROOT/scripts/router-rs/target/debug/router-rs\"; elif [ -x \"$CODEX_PROJECT_ROOT/target/release/router-rs\" ]; then ROUTER_RS_BIN=\"$CODEX_PROJECT_ROOT/target/release/router-rs\"; elif [ -x \"$CODEX_PROJECT_ROOT/target/debug/router-rs\" ]; then ROUTER_RS_BIN=\"$CODEX_PROJECT_ROOT/target/debug/router-rs\"; else ROUTER_RS_BIN=\"$(command -v router-rs 2>/dev/null || true)\"; fi; if [ ! -x \"$ROUTER_RS_BIN\" ]; then {missing_binary_fallback}; fi; \"$ROUTER_RS_BIN\" codex hook {audit_command} --repo-root \"$CODEX_PROJECT_ROOT\"'"
    )
}

fn merge_features_codex_hooks(existing: Option<&str>) -> (String, &'static str) {
    match existing {
        None => ("[features]\nhooks = true\n\n".to_string(), "created"),
        Some(text) => {
            let lines = text.lines().collect::<Vec<_>>();
            let mut out = Vec::new();
            let mut in_features = false;
            let mut features_seen = false;
            let mut hooks_set = false;
            for line in lines {
                let stripped = line.trim();
                if stripped.starts_with('[') && stripped.ends_with(']') {
                    if in_features && !hooks_set {
                        out.push("hooks = true".to_string());
                        hooks_set = true;
                    }
                    in_features = stripped == "[features]";
                    if in_features {
                        features_seen = true;
                    }
                    out.push(line.to_string());
                    continue;
                }
                if in_features
                    && (is_named_setting(line, "codex_hooks") || is_named_setting(line, "hooks"))
                {
                    out.push("hooks = true".to_string());
                    hooks_set = true;
                } else {
                    out.push(line.to_string());
                }
            }
            if in_features && !hooks_set {
                out.push("hooks = true".to_string());
            }
            if !features_seen {
                if out.last().is_some_and(|line| !line.trim().is_empty()) {
                    out.push(String::new());
                }
                out.push("[features]".to_string());
                out.push("hooks = true".to_string());
            }
            let merged = format!("{}\n", out.join("\n").trim_end());
            let canonical_existing = format!("{}\n", text.trim_end());
            if (text.ends_with('\n') && merged == canonical_existing) || merged == text {
                (merged, "unchanged")
            } else {
                (merged, "updated")
            }
        }
    }
}

fn is_named_setting(line: &str, key: &str) -> bool {
    line.split_once('=')
        .map(|(name, _)| name.trim() == key)
        .unwrap_or(false)
}

fn merge_hooks_json(
    existing: Option<Value>,
    hook_commands: &BTreeMap<String, String>,
) -> Result<(Value, HooksMergeStat), String> {
    let created = existing.is_none();
    let mut data = match existing {
        None => json!({}),
        Some(value) => {
            if !value.is_object() {
                return Err("Invalid hooks.json root type: expected object".to_string());
            }
            value
        }
    };
    let root = data
        .as_object_mut()
        .ok_or_else(|| "Invalid hooks.json root type: expected object".to_string())?;
    if !root.contains_key("hooks") {
        root.insert("hooks".to_string(), json!({}));
    }
    let hooks_root = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Invalid hooks.json: `hooks` must be an object".to_string())?;

    let mut preserved_existing_entries = 0usize;
    let mut added_entries = 0usize;
    let mut removed_legacy_entries = 0usize;

    for event in INSTALL_EVENTS {
        let hook_command = hook_commands
            .get(event)
            .ok_or_else(|| format!("Missing install hook command for event {event}"))?;
        if !hooks_root.contains_key(event) {
            hooks_root.insert(event.to_string(), Value::Array(Vec::new()));
        }
        let entries = hooks_root
            .get_mut(event)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| format!("Invalid hooks.json: hooks.{event} must be an array"))?;
        removed_legacy_entries += remove_legacy_python_codex_hooks(entries);
        preserved_existing_entries += entries.len();

        let exists = entries.iter().any(|entry| {
            entry
                .as_object()
                .and_then(|obj| obj.get("hooks"))
                .and_then(Value::as_array)
                .is_some_and(|hooks| {
                    hooks.iter().any(|hook| {
                        hook.as_object().is_some_and(|hook_obj| {
                            hook_obj.get("type").and_then(Value::as_str) == Some("command")
                                && hook_obj.get("command").and_then(Value::as_str)
                                    == Some(hook_command.as_str())
                        })
                    })
                })
        });
        if !exists {
            entries.push(json!({
                "hooks": [{
                    "type": "command",
                    "command": hook_command,
                    "timeout": 10,
                    "statusMessage": hook_event_status_message(event),
                }]
            }));
            added_entries += 1;
        }
    }
    let status = if created {
        "created"
    } else if added_entries > 0 || removed_legacy_entries > 0 {
        "updated"
    } else {
        "unchanged"
    };
    Ok((
        data,
        HooksMergeStat {
            status,
            preserved_existing_entries,
            added_entries,
            removed_legacy_entries,
        },
    ))
}

fn remove_legacy_python_codex_hooks(entries: &mut Vec<Value>) -> usize {
    let mut removed = 0usize;
    for entry in entries.iter_mut() {
        let Some(hooks) = entry
            .as_object_mut()
            .and_then(|obj| obj.get_mut("hooks"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = hooks.len();
        hooks.retain(|hook| {
            !hook
                .as_object()
                .and_then(|obj| obj.get("command"))
                .and_then(Value::as_str)
                .is_some_and(is_legacy_python_codex_hook_command)
        });
        removed += before.saturating_sub(hooks.len());
    }
    entries.retain(|entry| {
        entry
            .as_object()
            .and_then(|obj| obj.get("hooks"))
            .and_then(Value::as_array)
            .is_none_or(|hooks| !hooks.is_empty())
    });
    removed
}

fn is_legacy_python_codex_hook_command(command: &str) -> bool {
    command.contains("review_subagent_gate.py")
        || command.contains(".codex/hooks/review_subagent_gate.py")
}

pub fn run_codex_audit_hook(command: &str, repo_root: &Path) -> Result<Option<Value>, String> {
    let canonical = canonical_codex_audit_command(command)?;
    let mut payload = match read_stdin_payload() {
        Ok(payload) => payload,
        Err(err) if canonical == "review-subagent-gate" => {
            return Ok(Some(review_gate_input_error(&format!(
                "Review gate input JSON invalid: {err}",
            ))));
        }
        Err(err) => return Err(err),
    };
    if let Some(event_name) = codex_lifecycle_event_name(command) {
        if payload.is_object()
            && payload.get("hook_event_name").is_none()
            && payload.get("event").is_none()
        {
            payload["hook_event_name"] = json!(event_name);
        }
    }
    match canonical {
        "pre-tool-use" => run_codex_pre_tool_use(repo_root, &payload),
        "contract-guard" => run_codex_contract_guard(repo_root, &payload),
        "review-subagent-gate" => run_codex_review_subagent_gate(repo_root, &payload),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
}

fn sha256_hex(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn run_codex_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    run_pre_tool_use(repo_root, payload)
}

fn run_codex_contract_guard(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    let envelope = build_framework_contract_summary_envelope(repo_root)?;
    let summary = envelope
        .get("contract_summary")
        .ok_or_else(|| "framework contract summary missing contract_summary".to_string())?;
    let drift_flags = detect_contract_drift(summary, payload);
    let explicit_update = payload_bool(payload, "contract_update_intent")
        || payload_bool(payload, "allow_contract_update")
        || payload_bool(payload, "explicit_contract_update");
    let live_digest = summary
        .get("contract_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let decision = if !drift_flags.is_empty() && !explicit_update {
        "block"
    } else {
        "approve"
    };
    let reason = if drift_flags.is_empty() {
        "contract guard passed; no drift detected".to_string()
    } else if explicit_update {
        format!(
            "contract guard observed drift but explicit update intent was provided: {}",
            drift_flags.join(", ")
        )
    } else {
        format!(
            "contract guard blocked drift without explicit contract update intent: {}",
            drift_flags.join(", ")
        )
    };
    let mut response = json!({
        "decision": decision,
        "authority": CODEX_HOOK_AUTHORITY,
        "contract_guard": {
            "schema_version": "router-rs-codex-contract-guard-v1",
            "live_contract_digest": live_digest,
            "drift_flags": drift_flags,
            "explicit_contract_update": explicit_update,
            "prompt_lines": summary.get("prompt_lines").cloned().unwrap_or(Value::Array(Vec::new())),
            "reason": reason,
        },
    });
    if decision == "block" {
        response["hookSpecificOutput"] = json!({
            "hookEventName": "ContractGuard",
            "permissionDecision": "deny",
            "permissionDecisionReason": response["contract_guard"]["reason"].clone(),
        });
    }
    Ok(Some(response))
}

fn canonical_codex_audit_command(command: &str) -> Result<&'static str, String> {
    if let Some(event_name) = codex_lifecycle_event_name(command) {
        if event_name == "PreToolUse" {
            return Ok("pre-tool-use");
        }
        return Ok("review-subagent-gate");
    }
    match command {
        "pre-tool-use" => Ok("pre-tool-use"),
        "contract-guard" => Ok("contract-guard"),
        "review-subagent-gate" => Ok("review-subagent-gate"),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
}

fn codex_lifecycle_event_name(command: &str) -> Option<&'static str> {
    match command.trim().to_ascii_lowercase().as_str() {
        "sessionstart" => Some("SessionStart"),
        "pretooluse" => Some("PreToolUse"),
        "userpromptsubmit" => Some("UserPromptSubmit"),
        "posttooluse" => Some("PostToolUse"),
        "stop" => Some("Stop"),
        _ => None,
    }
}

fn detect_contract_drift(summary: &Value, payload: &Value) -> Vec<String> {
    let mut flags = Vec::new();
    let live_digest = summary
        .get("contract_digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(expected) = payload_string(payload, "expected_contract_digest")
        .or_else(|| payload_string(payload, "contract_digest"))
    {
        let expected = expected.strip_prefix("sha256:").unwrap_or(&expected);
        if !expected.is_empty() && expected != live_digest {
            flags.push("contract_digest_drift".to_string());
        }
    }

    let live_owner = summary
        .get("primary_owner")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(proposed_owner) = payload_string(payload, "proposed_primary_owner")
        .or_else(|| payload_string(payload, "primary_owner"))
    {
        if !live_owner.is_empty() && proposed_owner != live_owner {
            flags.push("owner_drift".to_string());
        }
    }

    let contract_active = summary
        .get("contract_guard")
        .and_then(|guard| guard.get("contract_active"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if contract_active {
        let live_task = summary
            .get("continuity")
            .and_then(|continuity| continuity.get("task"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(proposed_task) =
            payload_string(payload, "proposed_task").or_else(|| payload_string(payload, "task"))
        {
            if !live_task.is_empty() && proposed_task != live_task {
                flags.push("scope_drift".to_string());
            }
        }

        let live_goal = scalar_contract_text(summary.get("goal"));
        if let Some(proposed_goal) =
            payload_string(payload, "proposed_goal").or_else(|| payload_string(payload, "goal"))
        {
            if !live_goal.is_empty() && proposed_goal != live_goal {
                flags.push("scope_drift".to_string());
            }
        }

        let live_evidence = string_array(summary.get("evidence_required"));
        let proposed_evidence_exists = payload.get("proposed_evidence_required").is_some();
        let proposed_evidence = string_array(payload.get("proposed_evidence_required"));
        let drops_evidence = payload_bool(payload, "drops_evidence_required");
        let evidence_changed = proposed_evidence_exists
            && normalized_string_set(&proposed_evidence) != normalized_string_set(&live_evidence);
        if (drops_evidence && !live_evidence.is_empty()) || evidence_changed {
            flags.push("evidence_drift".to_string());
        }
    }

    flags.sort();
    flags.dedup();
    flags
}

fn payload_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn payload_bool(payload: &Value, key: &str) -> bool {
    payload.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn scalar_contract_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        _ => String::new(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalized_string_set(values: &[String]) -> Vec<String> {
    let mut deduped = HashSet::new();
    let mut normalized = values
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .filter_map(|item| {
            let lower = item.to_ascii_lowercase();
            deduped.insert(lower.clone()).then_some(lower)
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized
}

fn block_codex_pre_tool_use(reason: String) -> Option<Value> {
    Some(json!({
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    }))
}

fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    let mut rel_paths = HashSet::new();
    for path in iter_payload_paths(payload) {
        rel_paths.insert(relative_candidate_path(&path, repo_root));
    }
    for path in rel_paths.iter().cloned().collect::<Vec<_>>() {
        if classify_protected_generated_path(&path).is_some() {
            let message = pre_tool_use_message(&path);
            return Ok(block_codex_pre_tool_use(message));
        }
    }
    if let Some(path) = bash_generated_write_target(payload) {
        let message = pre_tool_use_message(&path);
        return Ok(block_codex_pre_tool_use(message));
    }
    Ok(None)
}

fn read_stdin_payload() -> Result<Value, String> {
    let mut stdin = io::stdin().lock();
    let input = read_codex_stdin_limited(&mut stdin)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(trimmed).map_err(|err| format!("stdin_json_invalid: {err}"))
}

fn review_gate_input_error(message: &str) -> Value {
    json!({
        "decision": "block",
        "message": message,
        "reason": message,
        "hookSpecificOutput": {
            "hookEventName": "ReviewSubagentGate",
            "permissionDecision": "deny",
            "permissionDecisionReason": message,
        },
    })
}

fn read_codex_stdin_limited<R: Read>(reader: &mut R) -> Result<String, String> {
    const LIMIT: u64 = 4 * 1024 * 1024;
    let mut input = String::new();
    let mut limited = reader.take(LIMIT);
    limited
        .read_to_string(&mut input)
        .map_err(|err| err.to_string())?;
    if limited.limit() == 0 {
        let inner = limited.into_inner();
        let mut probe = [0u8; 1];
        if inner.read(&mut probe).map_err(|err| err.to_string())? > 0 {
            return Err("stdin payload exceeds 4 MiB limit".to_string());
        }
    }
    Ok(input)
}

fn iter_candidate_paths(payload: &Value) -> Vec<String> {
    let mut candidates = Vec::new();
    for key in [
        "file_path",
        "changed_path",
        "path",
        "config_path",
        "target_path",
    ] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            let normalized = text.replace('\\', "/");
            if !normalized.is_empty() {
                candidates.push(normalized);
            }
        }
    }
    if let Some(items) = payload.get("changed_files").and_then(Value::as_array) {
        for item in items {
            if let Some(text) = item.as_str() {
                let normalized = text.replace('\\', "/");
                if !normalized.is_empty() {
                    candidates.push(normalized);
                }
            }
        }
    }
    candidates
}

fn iter_payload_paths(payload: &Value) -> Vec<String> {
    let mut candidates = iter_candidate_paths(payload);
    if let Some(tool_input) = payload.get("tool_input") {
        candidates.extend(iter_candidate_paths(tool_input));
    }
    candidates
}

fn relative_candidate_path(path: &str, repo_root: &Path) -> String {
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
            return normalize_repo_relative_path(&rel.to_string_lossy());
        }
    }
    normalize_repo_relative_path(path)
}

fn normalize_repo_relative_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else {
                    parts.push(part);
                }
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn classify_protected_generated_path(path: &str) -> Option<&'static str> {
    let normalized = normalize_repo_relative_path(path);
    if protected_generated_paths().contains(&normalized.as_str()) {
        return Some("generated_file");
    }
    if PROTECTED_GENERATED_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return Some("generated_file");
    }
    None
}

fn pre_tool_use_message(path: &str) -> String {
    format!(
        "[codex-pre-tool-use] blocked direct edits to generated Codex agent surface {path}; rerun `{}` instead."
        ,
        HOST_ENTRYPOINT_SYNC_HINT
    )
}

fn bash_generated_write_target(payload: &Value) -> Option<String> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;
    if tool_name != "Bash" {
        return None;
    }
    let command = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        for hint in protected_generated_paths() {
            if bash_segment_mentions_generated_path(&segment, hint)
                && (looks_mutating || bash_segment_redirects_to_hint(&segment, hint))
            {
                return Some(hint.to_string());
            }
        }
    }
    None
}

fn split_bash_segments(command: &str) -> Vec<String> {
    let chars = command.chars().collect::<Vec<_>>();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut idx = 0usize;

    while idx < chars.len() {
        let current = chars[idx];
        let next = chars.get(idx + 1).copied();
        let prev = if idx > 0 { Some(chars[idx - 1]) } else { None };
        let mut separator_len = 0usize;

        if current == ';' {
            separator_len = 1;
        } else if next == Some(current) && matches!(current, '&' | '|') {
            separator_len = 2;
        } else if current == '|' && prev != Some('>') {
            separator_len = 1;
        }

        if separator_len > 0 {
            let segment = chars[start..idx].iter().collect::<String>();
            let trimmed = segment.trim();
            if !trimmed.is_empty() {
                segments.push(trimmed.to_string());
            }
            idx += separator_len;
            start = idx;
            continue;
        }

        idx += 1;
    }

    let tail = chars[start..].iter().collect::<String>();
    let trimmed = tail.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }

    if segments.is_empty() {
        vec![command.trim().to_string()]
    } else {
        segments
    }
}

fn bash_command_looks_mutating(command: &str) -> bool {
    [
        r"^\s*(mv|cp|install|touch|rm|unlink|truncate)\b",
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

fn bash_segment_mentions_generated_path(segment: &str, hint: &str) -> bool {
    segment
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '\'' | '"' | ';' | '&' | '|'))
        .map(|token| token.trim_start_matches('>').trim_start_matches("of="))
        .any(|token| normalize_repo_relative_path(token) == hint)
}

fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    let escaped = regex::escape(hint);
    [
        format!(r#"(>>?|>\|)\s*['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#),
        format!(r#"\btee\b(?:\s+-a)?\s+['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#),
        format!(r#"\bdd\b[^\n;&|]*\bof=['\"]?[^'\"\n;&|]*{escaped}[^'\"\n;&|]*['\"]?"#),
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(segment))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn protected_generated_paths_match_lexical_variants() {
        assert_eq!(normalize_repo_relative_path("./AGENTS.md"), "AGENTS.md");
        assert_eq!(
            normalize_repo_relative_path(".codex/../.codex/host_entrypoints_sync_manifest.json"),
            ".codex/host_entrypoints_sync_manifest.json"
        );
        assert!(classify_protected_generated_path("./AGENTS.md").is_some());
        assert!(classify_protected_generated_path(
            ".codex/../.codex/host_entrypoints_sync_manifest.json"
        )
        .is_some());
        assert!(classify_protected_generated_path("./.codex/prompts/gitx.md").is_none());
    }

    #[test]
    fn pre_tool_use_blocks_normalized_direct_paths() {
        let payload = json!({"tool_input": {"file_path": "./AGENTS.md"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({"tool_input": {"file_path": ".codex/../.codex/host_entrypoints_sync_manifest.json"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({"tool_input": {"file_path": ".codex/../.codex/prompts/autopilot.md"}});
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());
    }

    #[test]
    fn pre_tool_use_blocks_normalized_bash_write_targets() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x > ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x | tee .codex/../.codex/host_entrypoints_sync_manifest.json"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x | tee .codex/prompts/gitx.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());

        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "printf x >| ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_some());
    }

    #[test]
    fn pre_tool_use_allows_read_only_bash_commands_on_protected_paths() {
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "cat ./AGENTS.md"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());

        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {"command": "rg contract_digest .codex/host_entrypoints_sync_manifest.json"}
        });
        assert!(run_pre_tool_use(Path::new("."), &payload)
            .unwrap()
            .is_none());
    }

    #[test]
    fn sync_host_entrypoints_reports_would_write_in_dry_run() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("router-rs-codex-hooks-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(root.join(".codex")).unwrap();
        fs::write(root.join("AGENTS.md"), "stale").unwrap();
        fs::write(
            root.join(".codex/host_entrypoints_sync_manifest.json"),
            "{}",
        )
        .unwrap();

        let report = sync_host_entrypoints(&root, false).unwrap();
        let would_write = report
            .get("would_write")
            .and_then(Value::as_array)
            .unwrap()
            .len();
        let written = report
            .get("written")
            .and_then(Value::as_array)
            .unwrap()
            .len();
        assert!(would_write > 0);
        assert_eq!(written, 0);

        fs::remove_dir_all(&root).unwrap();
    }

    mod install_codex_cli_hooks_tests {
        use super::*;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::SystemTime;

        static INSTALL_SEQ: AtomicU64 = AtomicU64::new(0);

        fn fresh_path(label: &str) -> PathBuf {
            let base = std::env::temp_dir().join(format!(
                "install-codex-cli-hooks-{}-{}-{}",
                label,
                std::process::id(),
                INSTALL_SEQ.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir_all(&base).unwrap();
            base
        }

        fn run_install(codex_home: &Path, repo_root: &Path, mode: InstallMode) -> Value {
            install_codex_cli_hooks(codex_home, repo_root, mode).unwrap()
        }

        fn install_hook_commands(repo_root: &Path) -> BTreeMap<String, String> {
            INSTALL_EVENTS
                .iter()
                .map(|event| {
                    (
                        (*event).to_string(),
                        build_install_hook_command(repo_root, event),
                    )
                })
                .collect()
        }

        #[test]
        fn empty_codex_home_creates_config_and_hooks() {
            let root = fresh_path("empty");
            let codex_home = root.join("new-codex-home");
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let config_path = codex_home.join("config.toml");
            let hooks_path = codex_home.join("hooks.json");
            assert!(config_path.exists());
            assert!(hooks_path.exists());
            assert_eq!(payload["config_toml"]["status"].as_str(), Some("created"));
            assert_eq!(payload["hooks_json"]["status"].as_str(), Some("created"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn existing_config_with_features_block_preserves_other_keys() {
            let root = fresh_path("features-preserve");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(
                codex_home.join("config.toml"),
                "[features]\nother_flag = true\n",
            )
            .unwrap();
            run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
            assert!(text.contains("other_flag = true"));
            assert!(text.contains("hooks = true"));
            assert!(!text.contains("codex_hooks"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn existing_config_with_codex_hooks_false_under_features_replaces() {
            let root = fresh_path("replace");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(
                codex_home.join("config.toml"),
                "[features]\ncodex_hooks = false\n",
            )
            .unwrap();
            run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
            assert_eq!(text, "[features]\nhooks = true\n");
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn existing_config_with_codex_hooks_under_other_section_untouched() {
            let root = fresh_path("other-section");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(
                codex_home.join("config.toml"),
                "[custom]\ncodex_hooks = false\n[features]\nother = 1\n",
            )
            .unwrap();
            run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
            assert!(text.contains("[custom]\ncodex_hooks = false"));
            assert!(text.contains("[features]\nother = 1\nhooks = true"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn config_without_features_appends_section() {
            let root = fresh_path("append-features");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[custom]\nvalue = 1\n").unwrap();
            run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("config.toml")).unwrap();
            assert!(text.ends_with("[features]\nhooks = true\n"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn existing_hooks_json_preserves_existing_entry() {
            let root = fresh_path("preserve-hooks");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(
                codex_home.join("hooks.json"),
                "{\n  \"hooks\": {\n    \"Stop\": [\n      {\"hooks\": [{\"type\": \"command\", \"command\": \"echo keep\"}]}\n    ]\n  }\n}\n",
            )
            .unwrap();
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            assert!(text.contains("echo keep"));
            assert!(
                payload["hooks_json"]["preserved_existing_entries"]
                    .as_u64()
                    .unwrap()
                    >= 1
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_removes_legacy_python_codex_hooks() {
            let root = fresh_path("remove-legacy-python");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(
                codex_home.join("hooks.json"),
                r#"{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/usr/bin/env python3 \"/Users/joe/Documents/skill/.codex/hooks/review_subagent_gate.py\"",
            "timeout": 10
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {"type": "command", "command": "echo keep"},
          {"type": "command", "command": "python3 review_subagent_gate.py"}
        ]
      }
    ]
  }
}
"#,
            )
            .unwrap();
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let text = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            assert!(!text.contains("review_subagent_gate.py"));
            assert!(text.contains("echo keep"));
            assert!(text.contains("codex hook --event=UserPromptSubmit"));
            assert_eq!(
                payload["hooks_json"]["removed_legacy_entries"].as_u64(),
                Some(2)
            );
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn idempotent_install() {
            let root = fresh_path("idempotent");
            let codex_home = root.join("codex");
            let first = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let second = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            assert_eq!(first["config_toml"]["status"].as_str(), Some("created"));
            assert_eq!(second["config_toml"]["status"].as_str(), Some("unchanged"));
            assert_eq!(second["hooks_json"]["status"].as_str(), Some("unchanged"));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn check_mode_does_not_write() {
            let root = fresh_path("check-mode");
            let codex_home = root.join("codex-check-do-not-write");
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Check);
            assert_eq!(
                payload["config_toml"]["status"].as_str(),
                Some("would-create")
            );
            assert_eq!(
                payload["hooks_json"]["status"].as_str(),
                Some("would-create")
            );
            assert!(!codex_home.join("config.toml").exists());
            assert!(!codex_home.join("hooks.json").exists());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn hook_command_format_pure_router_rs_binary() {
            let repo_root = Path::new("/Users/joe/Documents/skill");
            let stop_command = build_install_hook_command(repo_root, "Stop");
            assert!(stop_command.contains("codex hook --event=Stop"));
            assert!(stop_command.contains("router-rs binary unavailable for Codex hook"));
            assert!(!stop_command.contains("/Users/joe/Documents/skill"));
            let pre_tool_command = build_install_hook_command(repo_root, "PreToolUse");
            assert!(pre_tool_command.contains("codex hook --event=PreToolUse"));
            assert!(!pre_tool_command.contains("codex hook pre-tool-use"));
        }

        #[test]
        fn hook_command_ignores_repo_root_shell_content() {
            let repo_root = Path::new("/tmp/repo-with-'quote");
            let command = build_install_hook_command(repo_root, "UserPromptSubmit");
            assert!(!command.contains("/tmp/repo-with-"));
            assert!(command.contains("git rev-parse --show-toplevel"));
            assert!(command.contains("exit 1"));
            let status = Command::new("bash")
                .arg("-n")
                .arg("-c")
                .arg(&command)
                .status()
                .unwrap();
            assert!(status.success());
        }

        #[test]
        fn apply_creates_backup_when_hooks_existed() {
            let root = fresh_path("backup");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
            let before = fs::metadata(codex_home.join("hooks.json"))
                .unwrap()
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let backup = payload["hooks_json"]["backup_path"]
                .as_str()
                .map(PathBuf::from)
                .unwrap();
            assert!(backup.exists());
            let after = fs::metadata(codex_home.join("hooks.json"))
                .unwrap()
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH);
            assert!(after >= before);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_payload_contains_projection_version_and_digest() {
            let root = fresh_path("payload-meta");
            let codex_home = root.join("codex");
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            assert_eq!(
                payload["projection_version"].as_str(),
                Some(ROUTER_RS_HOOK_PROJECTION_VERSION)
            );
            assert!(payload["command_digest"]
                .as_str()
                .is_some_and(|v| v.len() == 64));
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_writes_manifest_file_with_version() {
            let root = fresh_path("manifest");
            let codex_home = root.join("codex");
            let payload = run_install(&codex_home, Path::new("."), InstallMode::Apply);
            let manifest_path = codex_home.join(".router-rs-install.manifest.json");
            let manifest_text = fs::read_to_string(manifest_path).unwrap();
            let manifest: Value = serde_json::from_str(&manifest_text).unwrap();
            assert_eq!(
                manifest["projection_version"].as_str(),
                Some(ROUTER_RS_HOOK_PROJECTION_VERSION)
            );
            assert_eq!(manifest["command_digest"], payload["command_digest"]);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_backup_failure_bubbles_error() {
            let root = fresh_path("backup-failure");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
            #[cfg(unix)]
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o500)).unwrap();
            let before = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
            #[cfg(unix)]
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(result.is_err());
            let after = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            assert_eq!(before, after);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_write_failure_restores_backup() {
            let root = fresh_path("write-failure");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            fs::write(codex_home.join("hooks.json"), "{\"hooks\":{}}\n").unwrap();
            let before = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.set(true));
            let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
            FORCE_ATOMIC_WRITE_FAIL.with(|flag| flag.set(false));
            assert!(result.is_err());
            let after = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
            assert_eq!(before, after);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_permission_denied_fails_cleanly() {
            let root = fresh_path("permission-denied");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            #[cfg(unix)]
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o500)).unwrap();
            let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
            #[cfg(unix)]
            fs::set_permissions(&codex_home, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(result.is_err());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_symlink_target_handled_safely() {
            let root = fresh_path("symlink-hooks");
            let codex_home = root.join("codex");
            fs::create_dir_all(&codex_home).unwrap();
            fs::write(codex_home.join("config.toml"), "[features]\n").unwrap();
            let target = root.join("actual-hooks.json");
            fs::write(&target, "{\"hooks\":{}}\n").unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, codex_home.join("hooks.json")).unwrap();
            let result = install_codex_cli_hooks(&codex_home, Path::new("."), InstallMode::Apply);
            assert!(result.is_err());
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn install_hooks_invalid_root_returns_error() {
            let result = merge_hooks_json(Some(json!([])), &install_hook_commands(Path::new(".")));
            assert!(result
                .err()
                .unwrap_or_default()
                .contains("root type: expected object"));
        }

        #[test]
        fn install_hooks_invalid_hooks_field_returns_error() {
            let result = merge_hooks_json(
                Some(json!({"hooks":"not-an-object"})),
                &install_hook_commands(Path::new(".")),
            );
            assert!(result
                .err()
                .unwrap_or_default()
                .contains("`hooks` must be an object"));
        }

        #[test]
        fn install_hooks_invalid_event_array_returns_error() {
            let result = merge_hooks_json(
                Some(json!({"hooks":{"Stop":{"x":1}}})),
                &install_hook_commands(Path::new(".")),
            );
            assert!(result
                .err()
                .unwrap_or_default()
                .contains("hooks.Stop must be an array"));
        }

        #[test]
        fn atomic_write_completes_normally_with_fsync() {
            let root = fresh_path("atomic-fsync");
            let output = root.join("file.txt");
            write_atomic_text(&output, "hello").unwrap();
            assert_eq!(fs::read_to_string(output).unwrap(), "hello");
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn codex_hook_rejects_oversized_stdin() {
            let large = vec![b'a'; 5 * 1024 * 1024];
            let mut cursor = std::io::Cursor::new(large);
            let err = read_codex_stdin_limited(&mut cursor).unwrap_err();
            assert!(err.contains("exceeds 4 MiB"));
        }
    }

    mod review_gate_tests {
        use super::*;
        use serde_json::json;
        use std::sync::atomic::{AtomicU64, Ordering};

        static SEQ: AtomicU64 = AtomicU64::new(0);

        fn fresh_repo() -> std::path::PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "codex-review-gate-test-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(dir.join(".codex/hook-state")).unwrap();
            dir
        }

        #[test]
        fn user_prompt_submit_review_emits_additional_context() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-1",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review全仓找bug"
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap();
            let ctx = out
                .and_then(|v| v.get("hookSpecificOutput").cloned())
                .unwrap()
                .get("additionalContext")
                .and_then(Value::as_str)
                .unwrap()
                .to_string();
            assert!(ctx.contains("Subagent gate active:"));
            assert!(ctx.contains("Review gate active:"));
            assert!(ctx.len() <= CODEX_ADDITIONAL_CONTEXT_MAX_CHARS);
        }

        #[test]
        fn user_prompt_submit_with_override_does_not_emit() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-ovr",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review全仓找bug，不要用子代理"
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn additional_context_is_deduped_and_capped() {
            let duplicate =
                "Subagent gate active: start a bounded subagent lane first.".to_string();
            let long_line = "x".repeat(CODEX_ADDITIONAL_CONTEXT_MAX_CHARS);
            let ctx = codex_compact_contexts(vec![
                duplicate.clone(),
                duplicate,
                long_line.clone(),
                long_line,
            ])
            .unwrap();
            assert!(ctx.len() <= CODEX_ADDITIONAL_CONTEXT_MAX_CHARS);
            assert_eq!(
                ctx.matches("Subagent gate active: start a bounded subagent lane first.")
                    .count(),
                1
            );
        }

        #[test]
        fn post_tool_use_with_subagent_marks_seen() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore"}
            });
            let out = run_codex_review_subagent_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(state.review_subagent_seen);
            assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#explore"));
        }

        #[test]
        fn post_tool_use_without_subagent_type_does_not_mark_seen() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2b",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2b",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"prompt":"no type field"}
            });
            let out = run_codex_review_subagent_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(!state.review_subagent_seen);
            assert!(state.review_subagent_tool.is_none());
        }

        #[test]
        fn saw_subagent_codex_requires_typed_evidence() {
            assert!(!saw_subagent_codex(
                "Task",
                &json!({"prompt":"missing type"})
            ));
        }

        #[test]
        fn saw_subagent_codex_accepts_subagent_type_field() {
            assert!(saw_subagent_codex(
                "Task",
                &json!({"subagent_type":"explore"})
            ));
        }

        #[test]
        fn saw_subagent_codex_accepts_agent_type_field() {
            assert!(saw_subagent_codex(
                "Task",
                &json!({"agent_type":"ci-investigator"})
            ));
        }

        #[test]
        fn saw_subagent_codex_accepts_native_codex_agent_types() {
            for agent_type in ["default", "explorer", "worker"] {
                assert!(
                    saw_subagent_codex("functions.spawn_agent", &json!({"agent_type":agent_type})),
                    "expected native Codex agent_type={agent_type} to count as a subagent"
                );
            }
        }

        #[test]
        fn saw_subagent_codex_rejects_unknown_type() {
            assert!(!saw_subagent_codex(
                "Task",
                &json!({"subagent_type":"random-thing"})
            ));
        }

        #[test]
        fn post_tool_use_without_state_fails_closed() {
            let repo = fresh_repo();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2c",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore"}
            });
            let out = run_codex_review_subagent_gate(&repo, &post)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
            let reason = out
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or_default();
            assert!(reason.contains("missing"));
        }

        #[test]
        fn post_tool_use_with_invalid_state_fails_closed() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2d",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let state_path = codex_state_path(&repo, &start);
            fs::write(&state_path, "{invalid").unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2d",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore"}
            });
            let out = run_codex_review_subagent_gate(&repo, &post)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
            let reason = out
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or_default();
            assert!(reason.contains("state_json_invalid"));
        }

        #[test]
        fn stop_without_state_blocks_when_required() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-3",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let out = run_codex_review_subagent_gate(&repo, &payload)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
        }

        #[test]
        fn stop_without_state_still_blocks_when_no_text() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-4",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":""
            });
            let out = run_codex_review_subagent_gate(&repo, &payload)
                .unwrap()
                .unwrap();
            let reason = out.get("reason").and_then(Value::as_str).unwrap();
            assert!(reason.contains("Stop payload had no review context"));
        }

        #[test]
        fn stop_with_review_required_no_subagent_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-5",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-5",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_codex_review_subagent_gate(&repo, &stop)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
        }

        #[test]
        fn stop_with_delegation_required_blocks() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-6",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"前端后端测试并行推进"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-6",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_codex_review_subagent_gate(&repo, &stop)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
        }

        #[test]
        fn stop_with_subagent_seen_resets_state() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"subagent_type":"explore"}
            });
            let _ = run_codex_review_subagent_gate(&repo, &post).unwrap();
            let stop = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-7",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"继续"
            });
            let out = run_codex_review_subagent_gate(&repo, &stop).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &stop).unwrap().unwrap();
            assert_eq!(state.seq, 0);
            assert!(!state.review_subagent_seen);
        }

        #[test]
        fn stop_hook_active_returns_none() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Stop",
                "session_id":"sm-8",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"全面review",
                "stop_hook_active": true
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap();
            assert!(out.is_none());
        }

        #[test]
        fn no_drift_warn_when_manifest_missing() {
            let repo = fresh_repo();
            let codex_home = repo.join("codex-home");
            fs::create_dir_all(&codex_home).unwrap();
            std::env::set_var("CODEX_HOME", &codex_home);
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-drift-1",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"普通提问"
            });
            let out = run_codex_review_subagent_gate(&repo, &payload)
                .unwrap()
                .unwrap();
            let ctx = out["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            assert!(!ctx.contains("hook projection drift detected"));
        }

        #[test]
        fn no_drift_warn_when_manifest_matches() {
            let repo = fresh_repo();
            let codex_home = repo.join("codex-home");
            fs::create_dir_all(&codex_home).unwrap();
            std::env::set_var("CODEX_HOME", &codex_home);
            let manifest = json!({
                "projection_version": ROUTER_RS_HOOK_PROJECTION_VERSION,
                "command_digest": "abc",
            });
            fs::write(
                codex_home.join(".router-rs-install.manifest.json"),
                serde_json::to_string(&manifest).unwrap(),
            )
            .unwrap();
            let payload = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-drift-2",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"普通提问"
            });
            let out = run_codex_review_subagent_gate(&repo, &payload).unwrap();
            if let Some(value) = out {
                let ctx = value["hookSpecificOutput"]["additionalContext"]
                    .as_str()
                    .unwrap_or_default();
                assert!(!ctx.contains("hook projection drift detected"));
            }
        }

        #[test]
        fn each_codex_reject_reason_token_is_accepted() {
            for token in [
                "small_task",
                "shared_context_heavy",
                "write_scope_overlap",
                "next_step_blocked",
                "verification_missing",
                "token_overhead_dominates",
            ] {
                let text = format!("全面review，但reject reason: {token}");
                assert!(saw_reject_reason(&text));
                let inferred_overridden = has_override(&text)
                    || has_review_override(&text)
                    || has_delegation_override(&text)
                    || saw_reject_reason(&text);
                assert!(inferred_overridden);
            }
        }

        #[test]
        fn v1_migration_preserves_override_flag() {
            let repo = fresh_repo();
            let event = json!({"session_id":"v1-override"});
            let state_path = codex_state_path(&repo, &event);
            fs::write(
                state_path,
                r#"{"schema_version":1,"override":true,"subagent_required":true}"#,
            )
            .unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert!(state.review_override);
            assert!(state.delegation_override);
        }

        #[test]
        fn v1_migration_preserves_reject_reason_flag() {
            let repo = fresh_repo();
            let event = json!({"session_id":"v1-reject"});
            let state_path = codex_state_path(&repo, &event);
            fs::write(
                state_path,
                r#"{"schema_version":1,"reject_reason_seen":true}"#,
            )
            .unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert!(state.reject_reason_seen);
        }

        #[test]
        fn v1_delegation_only_maps_to_phase1() {
            let repo = fresh_repo();
            let event = json!({"session_id":"v1-phase"});
            let state_path = codex_state_path(&repo, &event);
            fs::write(
                state_path,
                r#"{"schema_version":1,"delegation_required":true,"review_subagent_seen":false}"#,
            )
            .unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert_eq!(state.seq, 1);
        }

        #[test]
        fn codex_session_key_fallback_is_invocation_scoped_without_identifiers() {
            let event = json!({});
            let a = codex_session_key(&event);
            let b = codex_session_key(&event);
            assert_ne!(a, b);
            assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
            assert_eq!(a.len(), 32);
        }

        #[test]
        fn codex_session_key_ignores_cwd_without_identifiers() {
            let event = json!({"cwd":"/tmp/shared-worktree"});
            let key = codex_session_key(&event);
            let cwd_hash = {
                let mut hasher = Sha256::new();
                hasher.update("/tmp/shared-worktree".as_bytes());
                hasher
                    .finalize()
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
                    .chars()
                    .take(32)
                    .collect::<String>()
            };
            assert_ne!(key, cwd_hash);
            assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
            assert_eq!(key.len(), 32);
        }

        #[test]
        fn saw_subagent_codex_accepts_agent_type_camel_case_field() {
            assert!(saw_subagent_codex(
                "Task",
                &json!({"agentType":"browser-use"})
            ));
        }

        #[test]
        fn post_tool_use_with_agent_type_camel_case_marks_seen() {
            let repo = fresh_repo();
            let start = json!({
                "hook_event_name":"UserPromptSubmit",
                "session_id":"sm-2e",
                "cwd": repo.to_string_lossy().to_string(),
                "prompt":"please do deep review"
            });
            let _ = run_codex_review_subagent_gate(&repo, &start).unwrap();
            let post = json!({
                "hook_event_name":"PostToolUse",
                "session_id":"sm-2e",
                "cwd": repo.to_string_lossy().to_string(),
                "tool_name":"Task",
                "tool_input":{"agentType":"explore"}
            });
            let out = run_codex_review_subagent_gate(&repo, &post).unwrap();
            assert!(out.is_none());
            let state = codex_load_state(&repo, &post).unwrap().unwrap();
            assert!(state.review_subagent_seen);
            assert_eq!(state.review_subagent_tool.as_deref(), Some("Task#explore"));
        }

        #[test]
        fn dispatch_unknown_event_blocks_with_message() {
            let repo = fresh_repo();
            let payload = json!({
                "hook_event_name":"Other",
                "session_id":"sm-9",
                "cwd": repo.to_string_lossy().to_string()
            });
            let out = run_codex_review_subagent_gate(&repo, &payload)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
            assert!(out
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("unsupported"));
        }

        #[test]
        fn dispatch_missing_event_blocks_with_message() {
            let repo = fresh_repo();
            let payload = json!({"session_id":"sm-10"});
            let out = run_codex_review_subagent_gate(&repo, &payload)
                .unwrap()
                .unwrap();
            assert_eq!(out.get("decision").and_then(Value::as_str), Some("block"));
            assert!(out
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("missing"));
        }

        #[test]
        fn codex_state_lock_recovers_from_stale_lock() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-stale"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
            fs::write(&lock_path, "pid=999999 ts=1\n").unwrap();
            let lock = acquire_codex_state_lock(&state_path);
            assert!(lock.is_ok());
        }

        #[test]
        fn codex_state_lock_recovers_from_corrupt_lock_metadata() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-corrupt"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
            fs::write(&lock_path, "not-a-lock-metadata-line\n").unwrap();
            let lock = acquire_codex_state_lock(&state_path);
            assert!(lock.is_ok());
        }

        #[test]
        fn codex_state_lock_recovers_from_unparseable_pid_and_ts() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-unparseable"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
            fs::write(&lock_path, "pid=bad ts=bad\n").unwrap();
            let lock = acquire_codex_state_lock(&state_path);
            assert!(lock.is_ok());
        }

        #[test]
        fn codex_state_lock_blocks_when_held() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-held"});
            let state_path = codex_state_path(&repo, &event);
            fs::create_dir_all(state_path.parent().unwrap()).unwrap();
            let guard = acquire_codex_state_lock(&state_path).unwrap();
            let started = std::time::Instant::now();
            let second = acquire_codex_state_lock(&state_path);
            assert!(second.is_err());
            assert!(started.elapsed() >= Duration::from_millis(1200));
            drop(guard);
        }

        #[test]
        fn codex_state_lock_serializes_concurrent_writes() {
            let repo = fresh_repo();
            let event = json!({"session_id":"lock-inc"});
            let repo_a = repo.clone();
            let repo_b = repo.clone();
            let event_a = event.clone();
            let event_b = event.clone();
            let worker = move |repo_root: PathBuf, ev: Value| {
                for _ in 0..1000 {
                    with_codex_state_lock(&repo_root, &ev, |loaded| {
                        let mut state = loaded.unwrap_or_default();
                        state.seq += 1;
                        Ok((Some(state), ()))
                    })
                    .unwrap();
                }
            };
            let t1 = std::thread::spawn(move || worker(repo_a, event_a));
            let t2 = std::thread::spawn(move || worker(repo_b, event_b));
            t1.join().unwrap();
            t2.join().unwrap();
            let state = codex_load_state(&repo, &event).unwrap().unwrap();
            assert_eq!(state.seq, 2000);
        }
    }
}
