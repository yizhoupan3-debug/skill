fn state_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".cursor").join("hook-state")
}

fn state_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("review-subagent-{}.json", session_key(event)))
}

fn state_lock_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("review-subagent-{}.lock", session_key(event)))
}

/// 已移除 `/loop` adversarial 功能；保留路径与清扫逻辑，便于 SessionEnd 清理历史 `adversarial-loop-*.json` 与 `.tmp-adv-loop-*` 孤儿文件。
fn adversarial_loop_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("adversarial-loop-{}.json", session_key(event)))
}

fn session_terminal_ledger_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("session-terminals-{}.json", session_key(event)))
}

fn remove_adversarial_loop(repo_root: &Path, event: &Value) {
    let _ = fs::remove_file(adversarial_loop_path(repo_root, event));
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PendingShellRecord {
    /// `normalize_shell_command` 产物，用作 FIFO 配对键。
    command_norm: String,
    /// Shell 钩子声明的 cwd 原始字符串（通常已是绝对路径）。
    cwd_raw: String,
    /// `beforeShellExecution` 入队单调时钟近似（毫秒，Unix）。
    queued_ms: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct SessionTerminalLedger {
    version: u32,
    baseline_pids: Vec<u32>,
    owned_pids: Vec<u32>,
    #[serde(default)]
    pending_shells: Vec<PendingShellRecord>,
}

const SESSION_TERMINAL_LEDGER_VERSION: u32 = 2;

fn load_session_terminal_ledger(repo_root: &Path, event: &Value) -> SessionTerminalLedger {
    let path = session_terminal_ledger_path(repo_root, event);
    let Ok(raw) = fs::read_to_string(path) else {
        return SessionTerminalLedger {
            version: SESSION_TERMINAL_LEDGER_VERSION,
            baseline_pids: Vec::new(),
            owned_pids: Vec::new(),
            pending_shells: Vec::new(),
        };
    };
    serde_json::from_str::<SessionTerminalLedger>(&raw).unwrap_or(SessionTerminalLedger {
        version: SESSION_TERMINAL_LEDGER_VERSION,
        baseline_pids: Vec::new(),
        owned_pids: Vec::new(),
        pending_shells: Vec::new(),
    })
}

fn save_session_terminal_ledger(repo_root: &Path, event: &Value, ledger: &SessionTerminalLedger) {
    let path = session_terminal_ledger_path(repo_root, event);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(ledger) {
        let _ = fs::write(path, text);
    }
}

/// **`ROUTER_RS_CURSOR_TERMINAL_KILL_MODE`**：默认 `scoped`（仅杀掉本会话账本 `owned_pids` 内的活跃 terminal）。
/// 设为 `legacy`/`all`/`repo`/`repo-wide`/`repowide` 时恢复旧行为：**仓库 cwd 范围内**扫描所有 stale active terminal（与是否本会话无关）。
fn cursor_terminal_kill_use_scoped_ownership() -> bool {
    match std::env::var("ROUTER_RS_CURSOR_TERMINAL_KILL_MODE") {
        Ok(raw) => {
            let t = raw.trim().to_ascii_lowercase();
            !matches!(
                t.as_str(),
                "legacy" | "all" | "repo" | "repo-wide" | "repowide"
            )
        }
        Err(_) => true,
    }
}

fn ensure_session_terminal_ledger_initialized(repo_root: &Path, event: &Value) {
    let path = session_terminal_ledger_path(repo_root, event);
    if path.is_file() {
        return;
    }
    maybe_init_session_terminal_ledger(repo_root, event);
}

fn trim_pending_shell_records(ledger: &mut SessionTerminalLedger) {
    while ledger.pending_shells.len() > MAX_PENDING_SHELL_RECORDS {
        ledger.pending_shells.remove(0);
    }
}

fn canonical_path_or_clone(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn shell_cwd_hint_matches_saved_record(saved_raw: &str, hint: Option<&Path>) -> bool {
    let Some(h) = hint else {
        return true;
    };
    let saved_trim = saved_raw.trim();
    if saved_trim.is_empty() {
        return true;
    }
    let saved_p = Path::new(saved_trim);
    let sp = canonical_path_or_clone(saved_p);
    let hp = canonical_path_or_clone(h);
    sp == hp || sp.starts_with(&hp) || hp.starts_with(&sp)
}

fn pop_matching_pending_shell(
    ledger: &mut SessionTerminalLedger,
    cmd_norm: &str,
    cwd_hint: Option<&Path>,
) -> Option<u64> {
    if cmd_norm.is_empty() {
        return None;
    }
    let idx = ledger.pending_shells.iter().position(|p| {
        p.command_norm == cmd_norm && shell_cwd_hint_matches_saved_record(&p.cwd_raw, cwd_hint)
    })?;
    Some(ledger.pending_shells.remove(idx).queued_ms)
}

fn augment_event_shell_command_cwd(
    base: &Value,
    command: Option<String>,
    cwd: Option<String>,
) -> Value {
    let mut obj = base
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    if let Some(c) = command {
        obj.insert("command".to_string(), Value::String(c));
    }
    if let Some(c) = cwd {
        obj.insert("cwd".to_string(), Value::String(c));
    }
    Value::Object(obj)
}

fn tool_input_shell_command_and_cwd(tool_input: &Value) -> (Option<String>, Option<String>) {
    let cmd = tool_input
        .get("command")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            tool_input
                .get("cmd")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| match tool_input.get("arguments") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        });
    let cwd = [
        "working_directory",
        "workingDirectory",
        "cwd",
        "workspace",
        "root",
        "workspaceRoot",
    ]
    .into_iter()
    .find_map(|k| {
        tool_input
            .get(k)
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    (cmd, cwd)
}

fn parse_terminal_started_at_unix_ms(raw: &str) -> Option<u64> {
    let s = raw.trim().trim_matches('"');
    if s.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp_millis().max(0) as u64)
}

fn cursor_post_tool_shell_terminal_track(repo_root: &Path, event: &Value) {
    let ti = tool_input_of(event);
    let (cmd, cwd) = tool_input_shell_command_and_cwd(&ti);
    let Some(cmd_s) = cmd else {
        return;
    };
    if cmd_s.trim().is_empty() {
        return;
    }
    ensure_session_terminal_ledger_initialized(repo_root, event);
    let augmented = augment_event_shell_command_cwd(event, Some(cmd_s), cwd);
    maybe_track_shell_owned_terminals(repo_root, &augmented, None);
}

fn merge_additional_context(output: &mut Value, extra: &str) {
    let extra = crate::autopilot_goal::scrub_spoof_host_followup_lines(extra);
    match output.get_mut("additional_context") {
        Some(Value::String(s)) => {
            s.push_str("\n\n");
            s.push_str(&extra);
            *s = crate::autopilot_goal::scrub_spoof_host_followup_lines(s);
        }
        _ => {
            output["additional_context"] = Value::String(extra);
        }
    }
}

struct LockGuard {
    path: PathBuf,
    /// 保持 POSIX `flock(LOCK_EX)` 风格独占锁存活；销毁句柄才释放。同路径仍写入 `pid=`/`ts=`
    /// 供日志与在无独立锁 API 环境下的 stale 判断（fallback）。
    _file: std::fs::File,
}

fn acquire_state_lock(repo_root: &Path, event: &Value) -> Option<LockGuard> {
    #[cfg(test)]
    if should_force_hook_state_lock_failure_for_test() {
        return None;
    }
    let dir = state_dir(repo_root);
    if fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let lock_path = state_lock_path(repo_root, event);
    for _ in 0..30 {
        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
        {
            Ok(file) => file,
            Err(_) => {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        match file.try_lock_exclusive() {
            Ok(()) => {
                let lock_text = format!("pid={} ts={}\n", std::process::id(), now_millis());
                let mut owned = file;
                let _ = owned.set_len(0);
                let _ = owned.seek(std::io::SeekFrom::Start(0));
                let _ = owned.write_all(lock_text.as_bytes());
                let _ = owned.sync_all();
                return Some(LockGuard {
                    path: lock_path,
                    _file: owned,
                });
            }
            Err(_) => {
                drop(file);
                if let Ok(existing) = fs::read_to_string(&lock_path) {
                    if let Some((pid, ts_ms)) = parse_lock_metadata(&existing) {
                        let age_ms = now_millis().saturating_sub(ts_ms);
                        if age_ms > 30_000 || !is_process_alive(pid) {
                            let _ = fs::remove_file(&lock_path);
                        }
                    }
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
    None
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn parse_lock_metadata(text: &str) -> Option<(u32, u64)> {
    let pid = text
        .split_whitespace()
        .find_map(|part| part.strip_prefix("pid="))
        .and_then(|v| v.parse::<u32>().ok())?;
    let ts = text
        .split_whitespace()
        .find_map(|part| part.strip_prefix("ts="))
        .and_then(|v| v.parse::<u64>().ok())?;
    Some((pid, ts))
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
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
fn is_process_alive(_pid: u32) -> bool {
    true
}

fn release_state_lock(lock: &mut Option<LockGuard>) {
    if let Some(guard) = lock.take() {
        let path = guard.path.clone();
        drop(guard);
        let _ = fs::remove_file(path);
    }
}

fn empty_state() -> ReviewGateState {
    ReviewGateState {
        version: STATE_VERSION,
        phase: 0,
        review_required: false,
        delegation_required: false,
        review_override: false,
        delegation_override: false,
        reject_reason_seen: false,
        active_subagent_count: 0,
        active_subagent_last_started_at: None,
        subagent_start_count: 0,
        subagent_stop_count: 0,
        followup_count: 0,
        review_followup_count: 0,
        goal_followup_count: 0,
        goal_required: false,
        goal_contract_seen: false,
        goal_progress_seen: false,
        goal_verify_or_block_seen: false,
        pre_goal_review_satisfied: false,
        pre_goal_nag_count: 0,
        last_prompt: None,
        last_subagent_type: None,
        last_subagent_tool: None,
        lane_intent_matches: None,
        review_subagent_cycle_open: false,
        review_subagent_cycle_key: None,
        updated_at: None,
    }
}

fn migrate_v1(raw: &Value) -> ReviewGateState {
    let mut state = empty_state();
    state.review_required = raw
        .get("review_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.delegation_required = raw
        .get("delegation_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.review_override = raw
        .get("review_override")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.delegation_override = raw
        .get("delegation_override")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    state.reject_reason_seen = raw
        .get("reject_reason_seen")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if raw
        .get("review_subagent_seen")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        state.phase = 2;
    } else if state.review_required || state.delegation_required {
        state.phase = 1;
    }
    state.followup_count = raw
        .get("followup_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    state.review_followup_count = raw
        .get("review_followup_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    state.goal_followup_count = raw
        .get("goal_followup_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    state
}

fn load_state(repo_root: &Path, event: &Value) -> Result<Option<ReviewGateState>, String> {
    let path = state_path(repo_root, event);
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("state_read_failed".to_string()),
    };
    let raw: Value = serde_json::from_str(&text).map_err(|_| "state_json_invalid".to_string())?;
    if !raw.is_object() {
        return Err("state_not_object".to_string());
    }
    // 仅迁移 legacy v1；v2 JSON 直接走 serde（避免吞掉 v2 字段）。
    if raw.get("version").and_then(Value::as_u64).unwrap_or(0) < 2 {
        return Ok(Some(migrate_v1(&raw)));
    }
    let mut base = empty_state();
    if let Ok(parsed) = serde_json::from_value::<ReviewGateState>(raw.clone()) {
        base = parsed;
    } else if let Some(obj) = raw.as_object() {
        if let Some(v) = obj.get("phase").and_then(Value::as_u64) {
            base.phase = v as u32;
        }
        if let Some(v) = obj.get("review_required").and_then(Value::as_bool) {
            base.review_required = v;
        }
        if let Some(v) = obj.get("delegation_required").and_then(Value::as_bool) {
            base.delegation_required = v;
        }
        if let Some(v) = obj.get("review_override").and_then(Value::as_bool) {
            base.review_override = v;
        }
        if let Some(v) = obj.get("delegation_override").and_then(Value::as_bool) {
            base.delegation_override = v;
        }
        if let Some(v) = obj.get("reject_reason_seen").and_then(Value::as_bool) {
            base.reject_reason_seen = v;
        }
        if let Some(v) = obj.get("active_subagent_count").and_then(Value::as_u64) {
            base.active_subagent_count = v as u32;
        }
        if let Some(v) = obj
            .get("active_subagent_last_started_at")
            .and_then(Value::as_str)
        {
            base.active_subagent_last_started_at = Some(v.to_string());
        }
        if let Some(v) = obj.get("subagent_start_count").and_then(Value::as_u64) {
            base.subagent_start_count = v as u32;
        }
        if let Some(v) = obj.get("subagent_stop_count").and_then(Value::as_u64) {
            base.subagent_stop_count = v as u32;
        }
        if let Some(v) = obj.get("followup_count").and_then(Value::as_u64) {
            base.followup_count = v as u32;
        }
        if let Some(v) = obj.get("review_followup_count").and_then(Value::as_u64) {
            base.review_followup_count = v as u32;
        }
        if let Some(v) = obj.get("goal_followup_count").and_then(Value::as_u64) {
            base.goal_followup_count = v as u32;
        }
        if let Some(v) = obj
            .get("pre_goal_review_satisfied")
            .and_then(Value::as_bool)
        {
            base.pre_goal_review_satisfied = v;
        }
    }
    base.version = STATE_VERSION;
    Ok(Some(base))
}

fn save_state(repo_root: &Path, event: &Value, state: &mut ReviewGateState) -> bool {
    let directory = state_dir(repo_root);
    let target = state_path(repo_root, event);
    let _ = fs::create_dir_all(&directory);
    state.version = STATE_VERSION;
    state.updated_at = Some(Utc::now().to_rfc3339());
    let payload = match serde_json::to_string_pretty(state) {
        Ok(text) => format!("{text}\n"),
        Err(_) => return false,
    };
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    let tmp = directory.join(format!(
        ".tmp-{}-{}-{}",
        std::process::id(),
        micros,
        target
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("state.json")
    ));
    let mut file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
    {
        Ok(f) => f,
        Err(_) => return false,
    };
    if file.write_all(payload.as_bytes()).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    if file.sync_all().is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    if fs::rename(&tmp, &target).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    #[cfg(unix)]
    {
        if let Ok(dir_file) = OpenOptions::new().read(true).open(&directory) {
            let _ = dir_file.sync_all();
        }
    }
    true
}
