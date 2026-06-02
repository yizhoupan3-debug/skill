use super::{lifecycle_host, CodexLifecycleContextState, CodexLifecycleHostKind};
use crate::router_env_flags::router_rs_env_enabled_default_true;
use sha2::{Digest, Sha256};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Once;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static CODEX_SESSION_KEY_FALLBACK_WARN: Once = Once::new();
static ATOMIC_WRITE_NONCE: AtomicU64 = AtomicU64::new(0);

/// Structured error type for Codex hook operations.
/// Replaces ad-hoc `Result<_, String>` with typed, matchable errors.
#[derive(Debug)]
pub(crate) enum CodexHookError {
    /// Failed to create the hook-state directory.
    StateDirCreate(io::Error),
    /// Failed to open the lock file.
    StateLockOpen(io::Error),
    /// Timed out waiting for the state lock.
    StateLockTimeout,
    /// flock(2) system call failed.
    StateLockFlock(io::Error),
    /// Lock acquisition failed (non-Unix platforms).
    StateLockAcquire(String),
    /// Failed to write lock metadata.
    StateLockWrite(io::Error),
    /// Failed to sync lock file.
    StateLockSync(io::Error),
    /// Failed to read hook-state file.
    StateReadIo(io::Error),
    /// Hook-state JSON is invalid or schema mismatch.
    StateJsonInvalid(String),
    /// Failed to write hook-state file.
    StateWriteFailed,
    /// Failed to serialize payload to JSON.
    PayloadSerialization(serde_json::Error),
    /// router-rs binary not found.
    BinaryUnavailable,
    /// Install lock timeout.
    InstallLockTimeout,
    /// Generic error for install/sync operations.
    InstallSync(String),
}

impl std::fmt::Display for CodexHookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StateDirCreate(e) => write!(f, "state_dir_create_failed: {e}"),
            Self::StateLockOpen(e) => write!(f, "state_lock_open_failed: {e}"),
            Self::StateLockTimeout => write!(f, "state_lock_timeout"),
            Self::StateLockFlock(e) => write!(f, "state_lock_flock_failed: {e}"),
            Self::StateLockAcquire(msg) => write!(f, "state_lock_acquire_failed: {msg}"),
            Self::StateLockWrite(e) => write!(f, "state_lock_write_failed: {e}"),
            Self::StateLockSync(e) => write!(f, "state_lock_sync_failed: {e}"),
            Self::StateReadIo(e) => write!(f, "state_read_failed: {e}"),
            Self::StateJsonInvalid(msg) => write!(f, "state_json_invalid: {msg}"),
            Self::StateWriteFailed => write!(f, "state_write_failed"),
            Self::PayloadSerialization(e) => write!(f, "payload_serialization_failed: {e}"),
            Self::BinaryUnavailable => write!(f, "router-rs binary unavailable"),
            Self::InstallLockTimeout => write!(f, "install_lock_timeout"),
            Self::InstallSync(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CodexHookError {}

/// Backward compatibility: convert CodexHookError to String for callers
/// that still use `Result<_, String>`.
impl From<String> for CodexHookError {
    fn from(msg: String) -> Self {
        CodexHookError::InstallSync(msg)
    }
}

impl From<CodexHookError> for String {
    fn from(err: CodexHookError) -> String {
        err.to_string()
    }
}

pub(super) fn codex_merge_legacy_subagent_gate_evidence(state: &mut CodexLifecycleContextState) {
    if state.review_subagent_seen
        && !state.generic_subagent_seen
        && !state.review_lane_seen
        && !state.parallel_lane_seen
    {
        state.generic_subagent_seen = true;
        state.review_lane_seen = true;
        state.parallel_lane_seen = true;
    }
}

pub(super) fn codex_state_dir(repo_root: &Path) -> PathBuf {
    repo_root
        .join(lifecycle_host().state_dir_leaf)
        .join("hook-state")
}

/// Unix: `flock(2)` on `<state>.lock` so same-process threads serialize correctly.
/// Non-Unix: `O_EXCL` lock file + stale detection (legacy).
#[cfg(unix)]
pub(super) struct CodexStateLock {
    file: File,
}

#[cfg(unix)]
impl Drop for CodexStateLock {
    fn drop(&mut self) {
        let fd = self.file.as_raw_fd();
        // SAFETY: `fd` is a valid file descriptor held by `self.file`;
        // `LOCK_UN` releases any advisory lock held by this process on `fd`.
        unsafe {
            let _ = libc::flock(fd, libc::LOCK_UN);
        }
    }
}

#[cfg(not(unix))]
pub(super) struct CodexStateLock {
    path: PathBuf,
}

#[cfg(not(unix))]
impl Drop for CodexStateLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Stable session identifier for hook-state filenames.
pub(super) fn codex_stable_session_raw(event: &Value) -> Option<String> {
    fn trimmed_nonempty(value: &str) -> Option<String> {
        let t = value.trim();
        (!t.is_empty()).then(|| t.to_string())
    }
    for key in [
        "session_id",
        "sessionId",
        "conversation_id",
        "conversationId",
        "thread_id",
        "threadId",
    ] {
        if let Some(s) = event
            .get(key)
            .and_then(Value::as_str)
            .and_then(trimmed_nonempty)
        {
            return Some(s);
        }
    }
    let env_keys: &[&str] = match lifecycle_host() {
        CodexLifecycleHostKind::ANTIGRAVITY_CLI => &[
            "ANTIGRAVITY_CLI_SESSION_ID",
            "ANTIGRAVITY_CLI_CONVERSATION_ID",
            "CODEX_SESSION_ID",
            "CODEX_CONVERSATION_ID",
        ],
        _ => &["CODEX_SESSION_ID", "CODEX_CONVERSATION_ID"],
    };
    for env_key in env_keys {
        if let Ok(v) = env::var(env_key) {
            if let Some(s) = trimmed_nonempty(&v) {
                return Some(s);
            }
        }
    }
    None
}

pub(super) fn codex_require_stable_session_key_enabled() -> bool {
    router_rs_env_enabled_default_true(lifecycle_host().require_stable_session_key_env())
}

/// Fallback hook-state key material when no stable session id (repo-scoped, not one global file).
pub(super) fn codex_unstable_session_key_raw(repo_root: &Path, event: &Value) -> String {
    let repo = repo_root.to_string_lossy();
    let cwd = event
        .get("cwd")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    if cwd.is_empty() {
        eprintln!(
            "[router-rs] codex hook-state: unstable fallback with empty cwd — prefer stable session ids or set ROUTER_RS_CODEX_HOOK_STATE_SALT"
        );
    }
    let payload_session = codex_stable_session_raw(event).unwrap_or_default();
    let salt = env::var(lifecycle_host().hook_state_salt_env())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let cwd_key = if cwd.is_empty() {
        "<empty-cwd>"
    } else {
        cwd.as_ref()
    };
    if salt.is_empty() {
        format!("unstable:repo={repo}|cwd={cwd_key}|payload_session={payload_session}")
    } else {
        format!("unstable:repo={repo}|cwd={cwd_key}|payload_session={payload_session}|salt={salt}")
    }
}

pub(super) fn codex_session_key(repo_root: &Path, event: &Value) -> String {
    let raw = codex_stable_session_raw(event).unwrap_or_else(|| {
        CODEX_SESSION_KEY_FALLBACK_WARN.call_once(|| {
            eprintln!(
                "[router-rs] codex hook-state: no stable session id (set CODEX_SESSION_ID / CODEX_CONVERSATION_ID or include session_id / sessionId / conversation_id / thread_id in hook payloads). With ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY disabled, hook-state uses a deterministic fallback keyed by repo (+ cwd / ROUTER_RS_CODEX_HOOK_STATE_SALT) — not a stable per-conversation id."
            );
        });
        codex_unstable_session_key_raw(repo_root, event)
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

pub(super) fn codex_state_path(repo_root: &Path, event: &Value) -> PathBuf {
    codex_state_dir(repo_root)
        .join(format!("review-subagent-{}.json", codex_session_key(repo_root, event)))
}

pub(super) fn parse_lock_metadata(text: &str) -> (Option<u32>, Option<u64>) {
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
pub(super) fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // Avoid spawning `kill` (PATH / sandbox failures must not look like "process dead").
    // SAFETY: signal 0 is a POSIX existence check that delivers no signal;
    // `pid` is parsed from lock-file metadata (not user input) and cast to `pid_t`.
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
pub(super) fn process_is_alive(_pid: u32) -> bool {
    true
}

pub(super) fn lock_is_stale(path: &Path) -> bool {
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

#[cfg(unix)]
pub(super) fn acquire_codex_state_lock(state_path: &Path) -> Result<CodexStateLock, CodexHookError> {
    let lock_path = PathBuf::from(format!("{}.lock", state_path.display()));
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).map_err(CodexHookError::StateDirCreate)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(CodexHookError::StateLockOpen)?;
    let fd = file.as_raw_fd();
    let deadline = SystemTime::now() + Duration::from_secs(20);
    loop {
        // SAFETY: `fd` is a valid fd from `OpenOptions::open`;
        // `LOCK_EX|LOCK_NB` is a non-blocking exclusive lock attempt.
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            break;
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EWOULDBLOCK)
            || err.raw_os_error() == Some(libc::EAGAIN)
        {
            if SystemTime::now() >= deadline {
                return Err(CodexHookError::StateLockTimeout);
            }
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        return Err(CodexHookError::StateLockFlock(err));
    }
    Ok(CodexStateLock { file })
}

#[cfg(not(unix))]
pub(super) fn acquire_codex_state_lock(state_path: &Path) -> Result<CodexStateLock, CodexHookError> {
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
                    .map_err(CodexHookError::StateLockWrite)?;
                file.sync_all()
                    .map_err(CodexHookError::StateLockSync)?;
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
            Err(err) => return Err(CodexHookError::StateLockAcquire(err.to_string())),
        }
    }
    Err(CodexHookError::StateLockTimeout)
}

pub(super) fn codex_load_state_from_path(path: &Path) -> Result<Option<CodexLifecycleContextState>, CodexHookError> {
    let text = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CodexHookError::StateReadIo(io::Error::new(io::ErrorKind::Other, "state_read_failed"))),
    };
    let mut value: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("[router-rs] hook-state JSON parse failed ({err}); backing up and resetting");
            let _ = fs::rename(path, path.with_extension("json.bak"));
            return Ok(None);
        }
    };
    if let Some(obj) = value.as_object_mut() {
        let schema_v1 = obj
            .get("schema_version")
            .and_then(Value::as_i64)
            .is_some_and(|v| v == 1);
        if schema_v1
            && obj
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
    serde_json::from_value::<CodexLifecycleContextState>(value)
        .map(|mut parsed| {
            codex_merge_legacy_subagent_gate_evidence(&mut parsed);
            Some(parsed)
        })
        .map_err(|err| {
            eprintln!("[router-rs] hook-state schema mismatch ({err}); backing up and resetting");
            let _ = fs::rename(path, path.with_extension("json.bak"));
            CodexHookError::StateJsonInvalid(err.to_string())
        })
        .or_else(|_| Ok(None))
}

pub(super) fn codex_load_state(
    repo_root: &Path,
    event: &Value,
) -> Result<Option<CodexLifecycleContextState>, CodexHookError> {
    codex_load_state_from_path(&codex_state_path(repo_root, event))
}

pub(super) fn codex_save_state_to_path(state_path: &Path, state: &CodexLifecycleContextState) -> bool {
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
    drop(tmp_file);
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

pub(super) fn prune_stale_hook_state_files(dir: &Path) {
    const MAX_FILES: usize = 50;
    const MAX_AGE_SECS: u64 = 7 * 24 * 3600;

    let entries: Vec<_> = match fs::read_dir(dir) {
        Ok(it) => it
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let s = name.to_string_lossy();
                s.starts_with("review-subagent-") && s.ends_with(".json")
            })
            .collect(),
        Err(_) => return,
    };

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut with_mtime: Vec<(u64, PathBuf)> = entries
        .iter()
        .filter_map(|e| {
            let mtime = e
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();
            Some((mtime, e.path()))
        })
        .collect();
    with_mtime.sort_by_key(|(mtime, _)| *mtime);

    let mut to_remove: Vec<PathBuf> = with_mtime
        .iter()
        .filter(|(mtime, _)| now_secs.saturating_sub(*mtime) > MAX_AGE_SECS)
        .map(|(_, p)| p.clone())
        .collect();

    let to_remove_set: std::collections::HashSet<&Path> = to_remove.iter().map(PathBuf::as_path).collect();
    let remaining: Vec<_> = with_mtime
        .iter()
        .filter(|(_, p)| !to_remove_set.contains(p.as_path()))
        .collect();

    if remaining.len() > MAX_FILES {
        let excess = remaining.len() - MAX_FILES;
        for (_, path) in remaining.iter().take(excess) {
            to_remove.push(path.clone());
        }
    }

    for path in to_remove {
        let _ = fs::remove_file(&path);
    }
}

pub(super) fn with_codex_state_lock<T, F>(repo_root: &Path, event: &Value, f: F) -> Result<T, CodexHookError>
where
    F: FnOnce(
        Option<CodexLifecycleContextState>,
    ) -> Result<(Option<CodexLifecycleContextState>, T), String>,
{
    let state_path = codex_state_path(repo_root, event);
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).map_err(CodexHookError::StateDirCreate)?;
        static LAST_PRUNE: AtomicU64 = AtomicU64::new(0);
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now_secs.saturating_sub(LAST_PRUNE.load(Ordering::Relaxed)) > 300 {
            prune_stale_hook_state_files(parent);
            LAST_PRUNE.store(now_secs, Ordering::Relaxed);
        }
    }
    let _guard = acquire_codex_state_lock(&state_path)?;
    let loaded = codex_load_state_from_path(&state_path)?;
    let (next_state, output) = f(loaded)?;
    if let Some(state) = next_state {
        if !codex_save_state_to_path(&state_path, &state) {
            return Err(CodexHookError::StateWriteFailed);
        }
    }
    Ok(output)
}

