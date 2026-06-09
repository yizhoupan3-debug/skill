//! E7 step 2: Claude hook session keying, review_gate / touch_state disk I/O, and flock locks.

use router_rs::framework_error::{FrameworkError, FrameworkResult};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(not(unix))]
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use super::{active_stdio_agent_hook_host, hook_state_base};

#[derive(Default)]
pub struct TouchState {
    pub settings: bool,
    pub framework: bool,
    pub settings_validated: bool,
    pub framework_tested: bool,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ReviewGateState {
    #[serde(default)]
    pub version: u32,
    pub review_required: bool,
    pub review_override: bool,
    pub independent_reviewer_seen: bool,
}

impl router_rs::hosts::hook_state_common::HookStateVersion for ReviewGateState {
    const STATE_VERSION: u32 = 1;
    fn version(&self) -> u32 {
        self.version
    }
}

#[derive(Debug, Clone)]
pub enum AgentDiskState<T> {
    Absent,
    Ok(T),
    Unreadable,
}

pub fn try_extract_session_string(payload: &Value) -> Option<String> {
    let map = payload.as_object()?;
    try_session_ids_from_object(map)
}

fn try_session_ids_from_object(map: &Map<String, Value>) -> Option<String> {
    for key in [
        "session_id",
        "conversation_id",
        "thread_id",
        "chat_id",
        "transcript_path",
        "conversationId",
        "threadId",
        "sessionId",
    ] {
        if let Some(value) = map.get(key).and_then(Value::as_str) {
            let t = value.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    if let Some(meta) = map.get("metadata").and_then(Value::as_object) {
        for key in ["sessionId", "conversationId", "chatId", "threadId"] {
            if let Some(value) = meta.get(key).and_then(Value::as_str) {
                let t = value.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

fn first_nonempty_payload_str(payload: &Value, keys: &[&str]) -> String {
    let Some(map) = payload.as_object() else {
        return String::new();
    };
    for key in keys {
        if let Some(s) = map.get(*key).and_then(Value::as_str) {
            let t = s.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    String::new()
}

fn repo_fallback_token(repo_root: &Path) -> String {
    let resolved = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let label = active_stdio_agent_hook_host().log_label();
    format!(
        "{label}-repo::{}",
        resolved.to_string_lossy().replace('\\', "/")
    )
}

fn short_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    hex_lower(&digest[..16])
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = FmtWrite::write_fmt(&mut s, format_args!("{:02x}", byte));
    }
    s
}

/// 与 Cursor `session_key` 同类：**显式会话串** → **宿主 `ROUTER_RS_*_SESSION_NAMESPACE`** → **`cwd` 类字段** → **repo 稳定 token**。
pub fn session_key(repo_root: &Path, payload: &Value) -> String {
    if let Some(raw) = try_extract_session_string(payload) {
        return short_hash(&raw);
    }
    if let Ok(ns) = std::env::var(active_stdio_agent_hook_host().session_namespace_env()) {
        let t = ns.trim();
        if !t.is_empty() {
            return short_hash(&format!("env::{t}"));
        }
    }
    const CWD_KEYS: &[&str] = &[
        "cwd",
        "workspaceFolder",
        "workspace_folder",
        "workspaceRoot",
        "workspace_root",
        "root",
    ];
    let cwd = first_nonempty_payload_str(payload, CWD_KEYS);
    if !cwd.is_empty() {
        return short_hash(&format!("cwd::{cwd}"));
    }
    short_hash(&repo_fallback_token(repo_root))
}

pub fn review_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    hook_state_base(repo_root).join(format!(
        "review_gate_{}.json",
        session_key(repo_root, payload)
    ))
}

fn legacy_review_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    repo_root.join(".claude").join(format!(
        "review_gate_{}.json",
        session_key(repo_root, payload)
    ))
}

fn review_gate_state_from_json(value: &Value) -> ReviewGateState {
    ReviewGateState {
        version: value
            .get("version")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        review_required: value
            .get("review_required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        review_override: value
            .get("review_override")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        independent_reviewer_seen: value
            .get("independent_reviewer_seen")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn read_review_gate_file(path: &Path) -> AgentDiskState<ReviewGateState> {
    if !path.is_file() {
        return AgentDiskState::Absent;
    }
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return AgentDiskState::Unreadable,
    };
    if raw.trim().is_empty() {
        return AgentDiskState::Unreadable;
    }
    let value: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return AgentDiskState::Unreadable,
    };
    AgentDiskState::Ok(review_gate_state_from_json(&value))
}

#[cfg(unix)]
struct ClaudeReviewStateLock {
    file: std::fs::File,
}

#[cfg(unix)]
impl Drop for ClaudeReviewStateLock {
    fn drop(&mut self) {
        let fd = self.file.as_raw_fd();
        unsafe {
            let _ = libc::flock(fd, libc::LOCK_UN);
        }
    }
}

#[cfg(not(unix))]
struct ClaudeReviewStateLock {
    path: PathBuf,
}

#[cfg(not(unix))]
impl Drop for ClaudeReviewStateLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
fn acquire_claude_review_state_lock(state_path: &Path) -> FrameworkResult<ClaudeReviewStateLock> {
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FrameworkError::other(format!("claude_state_dir_create_failed: {e}"))
        })?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| FrameworkError::other(format!("claude_state_lock_open_failed: {e}")))?;
    let fd = file.as_raw_fd();
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
    if rc != 0 {
        return Err(FrameworkError::other(format!(
            "claude_state_lock_flock_failed: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(ClaudeReviewStateLock { file })
}

#[cfg(not(unix))]
fn acquire_claude_review_state_lock(state_path: &Path) -> FrameworkResult<ClaudeReviewStateLock> {
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            FrameworkError::other(format!("claude_state_dir_create_failed: {e}"))
        })?;
    }
    let started = SystemTime::now();
    loop {
        match std::fs::OpenOptions::new()
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
                file.write_all(stamp.as_bytes()).map_err(|e| {
                    FrameworkError::other(format!("claude_state_lock_write_failed: {e}"))
                })?;
                file.sync_all().map_err(|e| {
                    FrameworkError::other(format!("claude_state_lock_sync_failed: {e}"))
                })?;
                return Ok(ClaudeReviewStateLock { path: lock_path });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if started.elapsed().unwrap_or(Duration::ZERO) > Duration::from_secs(20) {
                    if let Ok(meta) = fs::metadata(&lock_path) {
                        if let Ok(modified) = meta.modified() {
                            if modified.elapsed().unwrap_or(Duration::ZERO) > Duration::from_secs(60)
                            {
                                let _ = fs::remove_file(&lock_path);
                                continue;
                            }
                        }
                    }
                    return Err(FrameworkError::other("claude_state_lock_timeout"));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                return Err(FrameworkError::other(format!(
                    "claude_state_lock_open_failed: {err}"
                )));
            }
        }
    }
}

pub fn with_claude_review_state_lock<T, F>(state_path: &Path, f: F) -> FrameworkResult<T>
where
    F: FnOnce() -> FrameworkResult<T>,
{
    let _guard = acquire_claude_review_state_lock(state_path)?;
    f()
}

pub fn load_review_gate_disk(
    repo_root: &Path,
    payload: &Value,
) -> AgentDiskState<ReviewGateState> {
    let path = review_state_path(repo_root, payload);
    match read_review_gate_file(&path) {
        AgentDiskState::Ok(state) => return AgentDiskState::Ok(state),
        AgentDiskState::Unreadable => return AgentDiskState::Unreadable,
        AgentDiskState::Absent => {}
    }
    let legacy = legacy_review_state_path(repo_root, payload);
    match read_review_gate_file(&legacy) {
        AgentDiskState::Ok(state) => {
            if let Err(err) = with_claude_review_state_lock(&path, || {
                write_review_state_unlocked(&path, &state)
            }) {
                eprintln!(
                    "[router-rs] claude review_gate legacy migrate failed (using in-memory state): {err}"
                );
            }
            AgentDiskState::Ok(state)
        }
        other => other,
    }
}

pub fn load_touch_state_disk(
    repo_root: &Path,
    payload: &Value,
) -> AgentDiskState<TouchState> {
    let path = touch_state_path(repo_root, payload);
    if !path.is_file() {
        return AgentDiskState::Absent;
    }
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return AgentDiskState::Unreadable,
    };
    if raw.trim().is_empty() {
        return AgentDiskState::Unreadable;
    }
    let payload_val: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return AgentDiskState::Unreadable,
    };
    AgentDiskState::Ok(TouchState {
        settings: payload_val
            .get("settings")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        framework: payload_val
            .get("framework")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        settings_validated: payload_val
            .get("settings_validated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        framework_tested: payload_val
            .get("framework_tested")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

pub fn write_review_state_unlocked(
    path: &Path,
    state: &ReviewGateState,
) -> FrameworkResult<()> {
    let value = json!({
        "version": state.version,
        "review_required": state.review_required,
        "review_override": state.review_override,
        "independent_reviewer_seen": state.independent_reviewer_seen,
    });
    let body = format!("{value}\n");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(FrameworkError::IoSimple)?;
    }
    fs::write(path, &body).map_err(FrameworkError::IoSimple)?;
    Ok(())
}

pub fn clear_review_state(repo_root: &Path, payload: &Value) {
    let _ = fs::remove_file(review_state_path(repo_root, payload));
}

pub fn legacy_touch_state_path(repo_root: &Path) -> PathBuf {
    hook_state_base(repo_root).join("hook_state.json")
}

pub fn touch_state_path(repo_root: &Path, payload: &Value) -> PathBuf {
    hook_state_base(repo_root).join(format!(
        "hook_state_{}.json",
        session_key(repo_root, payload)
    ))
}

pub fn persist_touch_state(
    repo_root: &Path,
    session_payload: &Value,
    settings: bool,
    framework: bool,
    settings_validated: bool,
    framework_tested: bool,
) {
    let path = touch_state_path(repo_root, session_payload);
    let lock_path = path.clone();
    if let Err(err) = with_claude_review_state_lock(&lock_path, || {
        let current = match load_touch_state_disk(repo_root, session_payload) {
            AgentDiskState::Unreadable => {
                eprintln!(
                    "[router-rs] {} hook_state unreadable; skip merge (path {}): repair JSON or remove file",
                    active_stdio_agent_hook_host().log_label(),
                    path.display()
                );
                return Err(FrameworkError::other("hook_state_unreadable"));
            }
            AgentDiskState::Absent => TouchState::default(),
            AgentDiskState::Ok(s) => s,
        };
        let state_payload = json!({
            "settings": current.settings || settings,
            "framework": current.framework || framework,
            "settings_validated": current.settings_validated || settings_validated,
            "framework_tested": current.framework_tested || framework_tested,
        });
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(FrameworkError::IoSimple)?;
        }
        let _ = fs::remove_file(legacy_touch_state_path(repo_root));
        fs::write(&path, format!("{state_payload}\n")).map_err(FrameworkError::IoSimple)?;
        Ok(())
    }) {
        if err.to_hook_exit() != "hook_state_unreadable" {
            eprintln!(
                "[router-rs] {} hook state write failed (hook_state): {err}",
                active_stdio_agent_hook_host().log_label()
            );
        }
    }
}

pub fn clear_touch_state(repo_root: &Path, payload: &Value) {
    let _ = fs::remove_file(touch_state_path(repo_root, payload));
    let _ = fs::remove_file(legacy_touch_state_path(repo_root));
}
