// Review gate state, persistence, lock, and gate logic (P4 handlers split).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewGateState {
    pub version: u32,
    pub phase: u32,
    pub review_required: bool,
    pub review_override: bool,
    pub delegation_override: bool,
    pub reject_reason_seen: bool,
    /// Claude canonical: independent reviewer evidence (`fork_context=false` on countable lane).
    pub independent_reviewer_seen: bool,
    #[serde(default)]
    pub active_subagent_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_subagent_last_started_at: Option<String>,
    /// 仅统计 **`SubagentStart`** 上 qualifying review 入队次数；**`PostToolUse`**  multiset 入队不递增（与 `review_subagent_pending_cycle_keys` 长度不同步属刻意）。
    pub subagent_start_count: u32,
    pub subagent_stop_count: u32,
    pub followup_count: u32,
    pub review_followup_count: u32,
    pub goal_followup_count: u32,
    pub goal_required: bool,
    /// `/implementx|/verifyx` 本轮已武装（my-light 下可不设 `goal_required` 但仍跟踪 pre-goal）。
    #[serde(default)]
    pub goal_drive_entry_active: bool,
    pub goal_contract_seen: bool,
    pub goal_progress_seen: bool,
    pub goal_verify_or_block_seen: bool,
    /// My implement pre-goal：在 goal 契约与收口证据之前，要求独立上下文 subagent 预检（或拒绝原因词）。
    #[serde(default)]
    pub pre_goal_review_satisfied: bool,
    /// 连续触发 beforeSubmit 的 pre-goal 提示次数（清门或自动放行后归零）。
    #[serde(default)]
    pub pre_goal_nag_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_subagent_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_subagent_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_intent_matches: Option<bool>,
    #[serde(default)]
    pub review_subagent_cycle_open: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_subagent_cycle_key: Option<String>,
    /// 武装 review gate 后，每次 qualifying subagent **start**（PostToolUse / subagentStart）压入一条 cycle key（multiset）；qualifying **stop** 命中时**移除一条**同 key 记录，**仅当**本队列为空时升相位 3 并记 `subagent_stop_count`。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_subagent_pending_cycle_keys: Vec<String>,
    /// **review-lite** only (`id:` keys); strict path must not write here. Satisfaction: empty + phase≥3.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_lite_pending_cycle_keys: Vec<String>,
    /// Set when multiset push refused at cap (operator-visible on Stop).
    #[serde(default)]
    pub review_pending_cap_refused: bool,
    /// PostTool / subagentStart pending push timestamp for orphan stale recovery when count==0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_pending_last_pushed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl crate::hosts::hook_state_common::HookStateVersion for ReviewGateState {
    const STATE_VERSION: u32 = core_policy::HOOK_REVIEW_DISK_VERSION;
    fn disk_version(&self) -> u32 {
        core_policy::HOOK_REVIEW_DISK_VERSION
    }
}

impl ReviewGateState {
    fn review_gate_fields(&self) -> core_policy::HookReviewGateFields {
        core_policy::hook_review_gate_fields_from_parts(
            self.review_required,
            self.review_override,
            self.independent_reviewer_seen,
            self.reject_reason_seen,
        )
    }
}

fn hook_lock_unavailable_notice_json() -> Value {
    json!({
        "additional_context": "router-rs：`.cursor/hook-state` 锁不可用，本钩未写入 review gate 状态。请检查权限/争用后重试。"
    })
}

fn hook_state_lock_fail_closed_for_review_json() -> Value {
    json!({
        "permission": "deny",
        "user_message": "router-rs：`.cursor/hook-state` 锁不可用，review 证据路径已 fail-closed。请检查目录权限或争用后重试。"
    })
}

fn post_tool_armed_hook_state_lock_fail_closed_json() -> Value {
    let msg = "router-rs：`.cursor/hook-state` 锁不可用，review 证据路径已 fail-closed。请检查目录权限或争用后重试。";
    json!({
        "continue": false,
        "user_message": msg,
        "followup_message": msg
    })
}

/// Best-effort read without holding the session lock (TOCTOU-safe only for fail-closed branches).
fn peek_review_hard_armed(repo_root: &Path, event: &Value) -> bool {
    for _ in 0..3 {
        match load_state(repo_root, event) {
            Ok(Some(ref state)) => return review_hard_armed(state),
            Ok(None) => return false,
            Err(_) => {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    false
}

fn hook_state_lock_failure_output(repo_root: &Path, event: &Value) -> Value {
    if peek_review_hard_armed(repo_root, event) {
        hook_state_lock_fail_closed_for_review_json()
    } else {
        hook_lock_unavailable_notice_json()
    }
}

/// Live subagent cycle evidence (start/stop/pending). Excludes legacy `phase>=2` alone (wave-2 / P0-4).
fn review_subagent_live_evidence_seen(state: &ReviewGateState) -> bool {
    state.subagent_start_count > 0
        || state.subagent_stop_count > 0
        || !state.review_subagent_pending_cycle_keys.is_empty()
        || !state.review_lite_pending_cycle_keys.is_empty()
}

/// My execution-zone commands arm goal continuity gates (`/implementx`, `/verifyx`).
fn is_framework_goal_drive_entry_prompt(prompt: &str, signal_text: &str) -> bool {
    let _ = signal_text;
    core_policy::hook_common::is_framework_goal_entry_prompt(prompt)
}

/// 显式委托/并行入口走 bounded sidecar gate；goal 入口（My 执行区 `/implementx` 等）只走 goal 机。
fn framework_prompt_arms_delegation(text: &str) -> bool {
    core_policy::hook_common::is_framework_non_goal_entrypoint_prompt(text)
}

fn state_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("review-subagent-{}.json", session_key(event)))
}

fn state_lock_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("review-subagent-{}.lock", session_key(event)))
}

#[cfg(unix)]
struct UnixLockState {
    #[allow(dead_code)] // Retained for debugging; _file holds the actual flock.
    path: PathBuf,
    _file: std::fs::File,
}

#[cfg(windows)]
struct WindowsStateMutex {
    handle: *mut std::ffi::c_void,
}

#[cfg(windows)]
impl WindowsStateMutex {
    fn acquire(session_key: &str, timeout_ms: u32) -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr::null_mut;

        type LPCWSTR = *const u16;
        type HANDLE = *mut std::ffi::c_void;
        type DWORD = u32;
        type BOOL = i32;

        const WAIT_OBJECT_0: DWORD = 0x00000000;
        const WAIT_ABANDONED: DWORD = 0x00000080;

        #[link(name = "kernel32")]
        extern "system" {
            fn CreateMutexW(lpMutexAttributes: *mut std::ffi::c_void, bInitialOwner: BOOL, lpName: LPCWSTR) -> HANDLE;
            fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
            fn CloseHandle(hObject: HANDLE) -> BOOL;
            fn GetLastError() -> DWORD;
        }

        let mutex_name = format!("Local\\review-subagent-lock-{}", session_key);
        let mut name_w: Vec<u16> = OsStr::new(&mutex_name).encode_wide().collect();
        name_w.push(0);

        unsafe {
            let handle = CreateMutexW(null_mut(), 0, name_w.as_ptr());
            if handle.is_null() {
                return Err(format!("CreateMutexW failed with GetLastError={}", GetLastError()));
            }
            let wait_res = WaitForSingleObject(handle, timeout_ms);
            if wait_res == WAIT_OBJECT_0 || wait_res == WAIT_ABANDONED {
                Ok(Self { handle })
            } else {
                CloseHandle(handle);
                Err(format!("WaitForSingleObject lock timeout/failed (res={})", wait_res))
            }
        }
    }

    fn release(self) {
        unsafe {
            #[link(name = "kernel32")]
            extern "system" {
                fn ReleaseMutex(hMutex: *mut std::ffi::c_void) -> i32;
                fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            }
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

pub struct LockGuard {
    #[cfg(unix)]
    unix: UnixLockState,
    #[cfg(windows)]
    windows: WindowsStateMutex,
}

fn acquire_state_lock(repo_root: &Path, event: &Value) -> Option<LockGuard> {
    #[cfg(test)]
    if should_force_hook_state_lock_failure_for_test() {
        return None;
    }
    let wait_start = std::time::Instant::now();
    let dir = state_dir(repo_root);
    if fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let _session = session_key(event);
    let lock_path = state_lock_path(repo_root, event);

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        use fs2::FileExt;

        let retries = hooks::router_rs_cursor_hook_state_lock_retries();
        for _ in 0..retries {
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
                    let fd_metadata = match file.metadata() {
                        Ok(meta) => meta,
                        Err(_) => {
                            thread::sleep(Duration::from_millis(50));
                            continue;
                        }
                    };
                    let fd_inode = fd_metadata.ino();

                    let path_inode = match fs::metadata(&lock_path) {
                        Ok(meta) => Some(meta.ino()),
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                        Err(_) => {
                            thread::sleep(Duration::from_millis(50));
                            continue;
                        }
                    };

                    if Some(fd_inode) != path_inode {
                        drop(file);
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }

                    let lock_text = format!("pid={} ts={}\n", std::process::id(), now_millis());
                    let mut owned = file;
                    let _ = owned.set_len(0);
                    use std::io::Seek;
                    let _ = owned.seek(std::io::SeekFrom::Start(0));
                    let _ = owned.write_all(lock_text.as_bytes());

                    hooks::add_lock_wait_ms(wait_start.elapsed().as_millis() as u64);
                    return Some(LockGuard {
                        unix: UnixLockState {
                            path: lock_path,
                            _file: owned,
                        }
                    });
                }
                Err(_) => {
                    drop(file);
                    const HOOK_STATE_LOCK_STALE_MS: u64 = 30_000;
                    if let Ok(existing) = fs::read_to_string(&lock_path) {
                        if let Some((pid, ts_ms)) = parse_lock_metadata(&existing) {
                            let age_ms = now_millis().saturating_sub(ts_ms);
                            if !is_process_alive(pid) {
                                // Do not delete to preserve POSIX flock inode guarantee.
                            } else if age_ms > HOOK_STATE_LOCK_STALE_MS {
                                eprintln!(
                                    "[router-rs] hook-state lock held (pid={pid} age_ms={age_ms}); waiting (no remove_file)"
                                );
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
        None
    }

    #[cfg(windows)]
    {
        match WindowsStateMutex::acquire(&session, 3500) {
            Ok(win_lock) => {
                hooks::add_lock_wait_ms(wait_start.elapsed().as_millis() as u64);
                Some(LockGuard {
                    windows: win_lock
                })
            }
            Err(e) => {
                eprintln!("[router-rs] Windows NamedMutex acquisition failed: {}", e);
                None
            }
        }
    }
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
fn is_process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::raw::HANDLE;

        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> HANDLE;
            fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: *mut u32) -> i32;
            fn CloseHandle(hObject: HANDLE) -> i32;
        }

        if pid == 0 {
            return false;
        }

        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return std::io::Error::last_os_error().raw_os_error() != Some(87);
            }

            let mut exit_code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut exit_code);
            CloseHandle(handle);

            if ok != 0 {
                exit_code == STILL_ACTIVE
            } else {
                true
            }
        }
    }
    #[cfg(not(windows))]
    {
        true
    }
}

fn release_state_lock(lock: &mut Option<LockGuard>) {
    if let Some(guard) = lock.take() {
        #[cfg(unix)]
        {
            drop(guard.unix);
        }
        #[cfg(windows)]
        {
            guard.windows.release();
        }
    }
}

fn empty_state() -> ReviewGateState {
    ReviewGateState {
        version: STATE_VERSION,
        phase: 0,
        review_required: false,
        review_override: false,
        delegation_override: false,
        reject_reason_seen: false,
        independent_reviewer_seen: false,
        active_subagent_count: 0,
        active_subagent_last_started_at: None,
        subagent_start_count: 0,
        subagent_stop_count: 0,
        followup_count: 0,
        review_followup_count: 0,
        goal_followup_count: 0,
        goal_required: false,
        goal_drive_entry_active: false,
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
        review_subagent_pending_cycle_keys: Vec::new(),
        review_lite_pending_cycle_keys: Vec::new(),
        review_pending_cap_refused: false,
        review_pending_last_pushed_at: None,
        updated_at: None,
    }
}

fn sync_review_cycle_legacy_fields(state: &mut ReviewGateState) {
    state.review_subagent_cycle_open = !state.review_subagent_pending_cycle_keys.is_empty()
        || !state.review_lite_pending_cycle_keys.is_empty();
    state.review_subagent_cycle_key = state
        .review_subagent_pending_cycle_keys
        .last()
        .or_else(|| state.review_lite_pending_cycle_keys.last())
        .cloned();
}

fn hydrate_legacy_review_cycles_into_pending(state: &mut ReviewGateState) {
    if !state.review_subagent_pending_cycle_keys.is_empty() {
        sync_review_cycle_legacy_fields(state);
        return;
    }
    if state.review_subagent_cycle_open {
        if let Some(k) = state.review_subagent_cycle_key.clone() {
            state.review_subagent_pending_cycle_keys.push(k);
        }
    }
    sync_review_cycle_legacy_fields(state);
}

fn hydrate_review_gate_fields_from_disk(raw: &Value, state: &mut ReviewGateState) {
    core_policy::hydrate_hook_review_gate_fields_from_value(
        raw,
        &mut state.review_required,
        &mut state.review_override,
        &mut state.independent_reviewer_seen,
        &mut state.reject_reason_seen,
    );
}

fn migrate_v1(raw: &Value) -> ReviewGateState {
    let mut state = empty_state();
    hydrate_review_gate_fields_from_disk(raw, &mut state);
    state.delegation_override = raw
        .get("delegation_override")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if raw
        .get("review_subagent_seen")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        state.phase = 2;
    } else if state.review_required
        || raw
            .get("delegation_required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        state.phase = 1;
    }
    state.followup_count = raw
        .get("followup_count")
        .and_then(Value::as_u64)
        .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
        .unwrap_or(0);
    state.review_followup_count = raw
        .get("review_followup_count")
        .and_then(Value::as_u64)
        .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
        .unwrap_or(0);
    state.goal_followup_count = raw
        .get("goal_followup_count")
        .and_then(Value::as_u64)
        .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
        .unwrap_or(0);
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
            base.phase = u32::try_from(v).unwrap_or(u32::MAX);
        }
        if let Some(v) = obj.get("review_required").and_then(Value::as_bool) {
            base.review_required = v;
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
            base.active_subagent_count = u32::try_from(v).unwrap_or(u32::MAX);
        }
        if let Some(v) = obj
            .get("active_subagent_last_started_at")
            .and_then(Value::as_str)
        {
            base.active_subagent_last_started_at = Some(v.to_string());
        }
        if let Some(v) = obj.get("subagent_start_count").and_then(Value::as_u64) {
            base.subagent_start_count = u32::try_from(v).unwrap_or(u32::MAX);
        }
        if let Some(v) = obj.get("subagent_stop_count").and_then(Value::as_u64) {
            base.subagent_stop_count = u32::try_from(v).unwrap_or(u32::MAX);
        }
        if let Some(v) = obj.get("followup_count").and_then(Value::as_u64) {
            base.followup_count = u32::try_from(v).unwrap_or(u32::MAX);
        }
        if let Some(v) = obj.get("review_followup_count").and_then(Value::as_u64) {
            base.review_followup_count = u32::try_from(v).unwrap_or(u32::MAX);
        }
        if let Some(v) = obj.get("goal_followup_count").and_then(Value::as_u64) {
            base.goal_followup_count = u32::try_from(v).unwrap_or(u32::MAX);
        }
        if let Some(v) = obj
            .get("pre_goal_review_satisfied")
            .and_then(Value::as_bool)
        {
            base.pre_goal_review_satisfied = v;
        }
        if let Some(arr) = obj
            .get("review_subagent_pending_cycle_keys")
            .and_then(Value::as_array)
        {
            base.review_subagent_pending_cycle_keys = arr
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect();
        }
        if let Some(arr) = obj
            .get("review_lite_pending_cycle_keys")
            .and_then(Value::as_array)
        {
            base.review_lite_pending_cycle_keys = arr
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect();
        }
        if let Some(v) = obj
            .get("review_subagent_cycle_open")
            .and_then(Value::as_bool)
        {
            base.review_subagent_cycle_open = v;
        }
        if let Some(Value::String(v)) = obj.get("review_subagent_cycle_key") {
            let t = v.trim();
            if !t.is_empty() {
                base.review_subagent_cycle_key = Some(t.to_string());
            }
        }
    }
    hydrate_legacy_review_cycles_into_pending(&mut base);
    hydrate_review_gate_fields_from_disk(&raw, &mut base);
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
    if hooks::router_rs_cursor_hook_state_file_sync_enabled() {
        if file.sync_all().is_err() {
            let _ = fs::remove_file(&tmp);
            return false;
        }
    }
    if fs::rename(&tmp, &target).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    #[cfg(unix)]
    if hooks::router_rs_cursor_hook_state_dir_sync_enabled() {
        if let Ok(dir_file) = OpenOptions::new().read(true).open(&directory) {
            let _ = dir_file.sync_all();
        }
    }
    true
}

/// 仅 **review** 路径的硬门控（独立上下文 subagent 证据链）。
fn review_hard_armed(state: &ReviewGateState) -> bool {
    review_gate_armed(state.review_required, state.review_override)
}

fn review_pending_both_empty(state: &ReviewGateState) -> bool {
    state.review_lite_pending_cycle_keys.is_empty()
        && state.review_subagent_pending_cycle_keys.is_empty()
}

/// Stop advisory nudge — parity with `core_policy::hook_review_stop_advisory_needed`.
/// Pending multiset / phase are telemetry-only, not gate inputs.
fn review_stop_followup_needed(state: &ReviewGateState) -> bool {
    core_policy::hook_review_stop_advisory_needed(&state.review_gate_fields(), "REVIEW_GATE").is_some()
}

/// Compact bump requires live cycle progress beyond orphan `subagent_start_count` (stale hygiene may clear pending).
fn compact_bump_review_evidence_seen(state: &ReviewGateState) -> bool {
    review_subagent_live_evidence_seen(state)
        && (state.subagent_stop_count > 0
            || !state.review_subagent_pending_cycle_keys.is_empty()
            || !state.review_lite_pending_cycle_keys.is_empty())
}

/// 主线程 compact findings **不得**在无可数深度子代理证据时单独升 phase 3 清 REVIEW_GATE（P0-4 / wave-2）。
fn maybe_bump_review_phase_for_main_thread_compact_findings(
    state: &mut ReviewGateState,
    assistant_tail: &str,
) -> bool {
    if !review_hard_armed(state) || state.phase >= 3 {
        return false;
    }
    if !compact_bump_review_evidence_seen(state) {
        return false;
    }
    if !state.review_subagent_pending_cycle_keys.is_empty()
        || !state.review_lite_pending_cycle_keys.is_empty()
    {
        return false;
    }
    if !core_policy::review_output_lint::assistant_has_substantive_compact_review_finding_line(
        assistant_tail,
    ) {
        return false;
    }
    bump_phase(state, 3);
    clear_review_gate_escalation_counters(state);
    true
}

/// When true, Stop skips advisory `review-output-lint` on assistant tail (REVIEW_GATE / AG_FOLLOWUP active).
fn stop_review_output_lint_suppressed(state: &ReviewGateState) -> bool {
    review_stop_followup_needed(state)
        || (tracks_goal_or_drive_entry(state) && !goal_is_satisfied(state))
}

/// Stop / 观测 fixture 共用的 `need=` 段（前缀仍须含 `REVIEW_GATE` 供 `router_rs_observation` 分类）。
pub const REVIEW_GATE_FOLLOWUP_NEED_SEGMENT: &str =
    "need=deep_reviewer_cycle general-purpose|best-of-n|deep-reviewer fork_context=false";

/// Short, stable tail for `REVIEW_GATE incomplete` lines (after `need=`). Does not change the first
/// `router-rs` token (`REVIEW_GATE`) used by observation classification.
pub const REVIEW_GATE_FOLLOWUP_HINT_SEGMENT: &str =
    "hint=fork_context_json_false_not_omitted";

fn review_stop_followup_line(state: &ReviewGateState) -> String {
    let cap_need = if state.review_pending_cap_refused {
        format!(
            " need=pending_cycle_keys_at_cap max={}",
            hooks::router_rs_cursor_review_pending_cycle_max()
        )
    } else {
        String::new()
    };
    format!(
        "router-rs REVIEW_GATE incomplete phase={} {}{} {}",
        state.phase,
        REVIEW_GATE_FOLLOWUP_NEED_SEGMENT,
        cap_need,
        REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    )
}

/// `merge_hook_nudge_paragraph` 去重前缀：首行须与 `REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX` 常量一致以便每轮刷新同一段落。
pub const REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX: &str = "router-rs REVIEW_GATE detail";

pub const CURSOR_HOOK_STATE_UNREADABLE: &str =
    "router-rs CURSOR_HOOK_STATE_UNREADABLE need=repair_hook_state_json_or_permissions";

/// 超过「完整硬行」上限后写入 `followup_message` 的短行（仍以 `router-rs REVIEW_GATE` 开头供观测分类）。
pub fn review_stop_followup_soft_line(
    state: &ReviewGateState,
    full_line_cap: u32,
) -> String {
    format!(
        "router-rs REVIEW_GATE incomplete mode=soft_nag full_line_cap={full_line_cap} phase={} stop_nudge_count={} see=.cursor/hook-state rg_clear|ROUTER_RS_REVIEW_GATE_DISABLE=1|ROUTER_RS_REVIEW_GATE_STOP_MAX_NUDGES=0(strict)|detail=additional_context",
        state.phase, state.review_followup_count
    )
}

/// 完整 `need=`/`hint=` 行：降级到 `additional_context` 时与 `REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX` 首行合并。
pub fn review_stop_followup_detail_paragraph(state: &ReviewGateState) -> String {
    format!(
        "{}\n{}",
        REVIEW_GATE_DETAIL_PARAGRAPH_PREFIX,
        review_stop_followup_line(state)
    )
}

fn is_overridden(state: &ReviewGateState) -> bool {
    state.review_override || state.delegation_override
}

fn tracks_goal_or_drive_entry(state: &ReviewGateState) -> bool {
    state.goal_required || state.goal_drive_entry_active
}

fn goal_is_satisfied(state: &ReviewGateState) -> bool {
    if !tracks_goal_or_drive_entry(state) {
        return true;
    }
    // 全局 override（例如不要用子代理）仍可跳过整套 gate。
    if is_overridden(state) {
        return true;
    }
    state.goal_contract_seen && state.goal_progress_seen && state.goal_verify_or_block_seen
}

fn bump_phase(state: &mut ReviewGateState, target: u32) {
    state.phase = state.phase.max(target);
}

fn my_pre_goal_followup_message() -> String {
    "My implement (/implementx)：先写清 Goal 契约与验证口径（`GOAL_STATE.json`）；需要时再并行分工与证据索引（建议，非硬门槛）。确为小任务请**单独一行**拒因 token（如 small_task），不要自拟仿宿主 `router-rs …` 续跑行。"
        .to_string()
}

/// 连续 pre-goal 提示上限（**仅**显式 env 启用）：beforeSubmit 每轮在仍缺 pre-goal 时累加计数，达到后自动 `pre_goal_review_satisfied=true`。
/// - **未设置** / `0` / `false` / `off` / `no`：**不**自动放行（默认严格，P1-1）。
/// - 正整数：自定义上限（运维 opt-in）。
fn pre_goal_max_nudges_cap() -> Option<u32> {
    let Ok(raw) = std::env::var("ROUTER_RS_PRE_GOAL_MAX_NUDGES") else {
        return None;
    };
    let t = raw.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
        return None;
    }
    t.parse::<u32>().ok().filter(|v| *v >= 1)
}

fn maybe_pre_goal_nag_cap_release(state: &mut ReviewGateState) -> Option<&'static str> {
    if !hooks::router_rs_pre_goal_enabled() {
        return None;
    }
    if !tracks_goal_or_drive_entry(state)
        || state.pre_goal_review_satisfied
        || is_overridden(state)
        || state.reject_reason_seen
    {
        return None;
    }
    let cap = pre_goal_max_nudges_cap()?;
    state.pre_goal_nag_count = state.pre_goal_nag_count.saturating_add(1);
    if state.pre_goal_nag_count < cap {
        return None;
    }
    state.pre_goal_review_satisfied = true;
    state.pre_goal_nag_count = 0;
    clear_review_gate_escalation_counters(state);
    Some("router-rs：pre-goal 提示已达上限，已自动放行以便继续执行（需要严格不自动放行请设 `ROUTER_RS_PRE_GOAL_MAX_NUDGES=0`）。仍可在用户消息单独一行写 `small_task` 主动清门。")
}

/// Canonical `ROUTER_RS_REVIEW_GATE_DISABLE` or legacy `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`.
fn cursor_review_gate_disabled_by_env() -> bool {
    #[cfg(test)]
    {
        if let Some(v) = TEST_CURSOR_REVIEW_GATE_DISABLE.with(|c| c.get()) {
            return v;
        }
    }
    core_policy::env_flags::router_rs_review_gate_disabled_for_host("cursor")
}

/// Env disable **or** `lifecycle_profile: my-light` (prompt / GOAL_STATE) — profile-scoped, not global.
fn cursor_review_gate_suppressed(repo_root: &Path, text: &str) -> bool {
    if cursor_review_gate_disabled_by_env() {
        return true;
    }
    core_policy::hook_common::review_gate_hard_block_disabled(Some(repo_root), text)
}

/// `subagentStart` 只能拒绝/提示，不能主动关闭既有 subagent；这里用活跃数避免继续堆积。
fn cursor_max_open_subagents() -> Option<u32> {
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_MAX_OPEN_SUBAGENTS") else {
        return Some(DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS);
    };
    let t = raw.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
        return None;
    }
    t.parse::<u32>()
        .ok()
        .filter(|v| *v > 0)
        .map(|v| v.min(MAX_CONCURRENT_SUBAGENTS_LIMIT as u32))
        .or(Some(DEFAULT_CURSOR_MAX_OPEN_SUBAGENTS))
}

fn cursor_open_subagent_stale_after_secs() -> Option<i64> {
    let Ok(raw) = std::env::var("ROUTER_RS_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS") else {
        return Some(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS);
    };
    let t = raw.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "" | "0" | "false" | "off" | "no") {
        return None;
    }
    t.parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
        .or(Some(DEFAULT_CURSOR_OPEN_SUBAGENT_STALE_AFTER_SECS))
}

fn reset_stale_active_subagents(state: &mut ReviewGateState) -> bool {
    if state.active_subagent_count == 0 {
        return false;
    }
    let Some(stale_after_secs) = cursor_open_subagent_stale_after_secs() else {
        return false;
    };
    let Some(started_at) = state.active_subagent_last_started_at.as_deref() else {
        return false;
    };
    let Ok(started_at) = chrono::DateTime::parse_from_rfc3339(started_at) else {
        return false;
    };
    let age = Utc::now().signed_duration_since(started_at.with_timezone(&Utc));
    if age.num_seconds() <= stale_after_secs {
        return false;
    }
    state.active_subagent_count = 0;
    state.active_subagent_last_started_at = None;
    true
}

fn subagent_limit_denial(active: u32, limit: u32) -> Value {
    json!({
        "permission": "deny",
        "user_message": format!(
            "router-rs：当前会话已有 {active} 个 subagent 仍标记为打开（上限 {limit}，等于 `max_concurrent_subagents_limit` 契约）。请先等已有 subagent 结束/关闭，或确认它们已 stale 后清理会话状态；如需临时关闭限流，设置 ROUTER_RS_MAX_OPEN_SUBAGENTS=0。"
        )
    })
}

fn review_pending_cycle_cap_denial(cap: usize) -> Value {
    json!({
        "permission": "deny",
        "user_message": format!(
            "router-rs：review 子代理 pending 已达上限 {cap}（ROUTER_RS_REVIEW_PENDING_CYCLE_MAX）。请先等待已有 review subagentStop 核销 pending，或 Stop 后按 REVIEW_GATE 指引清门（rg_clear / 完成深度 lane）。"
        )
    })
}

/// 应急关闭门控时仍执行 PostToolUse/Subagent 状态更新，但不对模型注入门控类提示（与 SILENT 剥离字段一致）。
fn strip_cursor_hook_user_visible_nags(output: &mut Value) {
    if let Some(obj) = output.as_object_mut() {
        obj.remove("followup_message");
        obj.remove("additional_context");
        hooks::strip_router_rs_observation(output);
    }
}

/// 清门或 subagent 满足 review 后归零，避免 `followup_count` 长期累积导致 **escalation** 粘住。
fn clear_review_gate_escalation_counters(state: &mut ReviewGateState) {
    state.followup_count = 0;
    state.review_followup_count = 0;
    state.pre_goal_nag_count = 0;
}

/// Reset review-cycle progress (phase / pending / subagent counters). Parity with Codex UPS
/// when my-light disarms review, goal drive suppresses review, or a fresh deep-review cycle starts.
///
/// When `preserve_session_guards` is true (fresh deep-review re-arm), retain pending-cap refusal
/// so `ROUTER_RS_REVIEW_PENDING_CYCLE_MAX` cannot be bypassed via UPS. Open-subagent count
/// always resets on re-arm (P1-16: stale count without matching subagentStop).
fn reset_review_cycle_progress(state: &mut ReviewGateState, preserve_session_guards: bool) {
    state.phase = 0;
    state.subagent_start_count = 0;
    state.subagent_stop_count = 0;
    state.active_subagent_count = 0;
    state.active_subagent_last_started_at = None;
    if !preserve_session_guards {
        state.review_pending_cap_refused = false;
    }
    state.review_subagent_pending_cycle_keys.clear();
    state.review_lite_pending_cycle_keys.clear();
    state.review_pending_last_pushed_at = None;
    state.review_followup_count = 0;
    state.independent_reviewer_seen = false;
    sync_review_cycle_legacy_fields(state);
}

/// Same-submit review + My goal drive: review stays disarmed; operator-visible split hint (non-my-light only).
const CURSOR_REVIEW_MY_SAME_ROUND_NUDGE: &str = "router-rs：本轮提交同时包含「代码审查 / review」信号与 My 执行区入口（`/implementx`、`/verifyx`）；门控下 **不会** 在本回合因 review 措辞新武装 `REVIEW_GATE`。若需先跑独立审稿，请拆开用户消息（先发 review-only，再发 `/implementx`）或先落盘 `GOAL_STATE`。";

/// `GOAL_STATE` 列表字段是否含至少一条非空字符串（避免 `[""]` 这种伪非空数组）。
/// 用 `GOAL_STATE.json` + `EVIDENCE_INDEX.json` 补全 goal 门控（只置 true，不收回）；逻辑在 `ship_readiness.rs`。
///
/// `arm_if_goal_file`：**Stop** 路径传 `true` 以便在 GOAL 被 purge 时清除陈旧的 `goal_required`；
/// **不再**因盘上残留 GOAL 而武装 `goal_required`。
/// **beforeSubmit** 传 `false`。
///
/// **`pre_goal_review_satisfied`（磁盘旁路）**：在 `ROUTER_RS_PRE_GOAL_STRICT_DISK` 开启时
/// **不**因仅存在磁盘 GOAL 而置真（beforeSubmit 与 Stop 均适用）；其余 goal 字段的 hydrate
/// （contract/progress/verify 等）仍执行。
fn hydrate_goal_gate_from_disk(
    repo_root: &Path,
    state: &mut ReviewGateState,
    arm_if_goal_file: bool,
    frame: &core_state::task_state::CursorContinuityFrame,
    goal_drive_entrypoint: bool,
) {
    if !state.goal_required
        && !arm_if_goal_file
        && !goal_drive_entrypoint
        && !state.goal_drive_entry_active
    {
        return;
    }
    let Some((goal, task_id)) = frame.hydration_goal.as_ref() else {
        // Stop-only: missing/unparseable pointers or verifyx purge removed GOAL_STATE while
        // hook-state may still carry goal drive arms from /implementx|/verifyx.
        if arm_if_goal_file {
            state.goal_required = false;
            state.goal_drive_entry_active = false;
        }
        return;
    };
    if !hooks::router_rs_cursor_pre_goal_strict_disk_enabled()
        && (state.goal_required || goal_drive_entrypoint)
    {
        state.pre_goal_review_satisfied = true;
        state.pre_goal_nag_count = 0;
    }
    if state.goal_required || arm_if_goal_file || state.goal_drive_entry_active {
        let readiness = hooks::evaluate_goal_readiness_from_disk(
            repo_root,
            goal,
            task_id.as_str(),
        );
        if readiness.contract {
            state.goal_contract_seen = true;
        }
        if readiness.progress {
            state.goal_progress_seen = true;
        }
        if readiness.verification {
            state.goal_verify_or_block_seen = true;
        }
    }
}

/// Stop 上的 goal 门控短码（磁盘优先 evaluator；见 `ship_readiness.rs`）。
fn goal_stop_followup_line(state: &ReviewGateState) -> String {
    hooks::goal_stop_followup_line(
        state.goal_contract_seen,
        state.goal_progress_seen,
        state.goal_verify_or_block_seen,
        state.goal_followup_count,
    )
}

fn state_lock_degraded_followup() -> &'static str {
    "router-rs：hook-state 锁不可用，本闸门控降级。收口前须见独立 subagent lane，或在**用户消息**中单独一行写拒因。"
}

fn lock_failure_followup_for_before_submit(repo_root: &Path, event: &Value) -> (bool, String) {
    let text = prompt_text(event);
    let signal_text = hook_event_signal_text(event, &text, "");
    let review = is_review_prompt(&text);
    let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
    let review_arms = review && !goal_drive_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_override(&text);
    let disk_review_armed = load_state(repo_root, event)
        .ok()
        .flatten()
        .is_some_and(|s| review_hard_armed(&s));

    let strong_constraint =
        ((review_arms || delegation || goal_drive_entrypoint) && !overridden) || disk_review_armed;
    if strong_constraint {
        return (
            false,
            "router-rs：hook-state 锁不可用，本条为严格 review/委托/My 执行区门控，**已拦截提交**。请修锁/权限后重试，或起 subagent / 写明拒因。"
                .to_string(),
        );
    }

    (
        true,
        "router-rs：hook-state 锁不可用，门控**降级**；非严格提示仍可继续。".to_string(),
    )
}

fn stop_lock_failure_is_fail_closed(repo_root: &Path, event: &Value) -> bool {
    let text = prompt_text(event);
    let response_text = agent_response_text(event);
    let signal_text = hook_event_signal_text(event, &text, &response_text);
    let review = is_review_prompt(&text);
    let goal_drive_entrypoint = is_framework_goal_drive_entry_prompt(&text, &signal_text);
    let review_arms = review && !goal_drive_entrypoint;
    let delegation =
        is_parallel_delegation_prompt(&text) || framework_prompt_arms_delegation(&text);
    let overridden = has_override(&text) || saw_reject_reason(&signal_text, &text);
    let disk_review_armed = load_state(repo_root, event)
        .ok()
        .flatten()
        .is_some_and(|s| review_hard_armed(&s) || s.goal_required);
    ((review_arms || delegation || goal_drive_entrypoint) && !overridden) || disk_review_armed
}

fn review_gate_stop_lock_unavailable_line() -> String {
    format!(
        "router-rs REVIEW_GATE incomplete phase=0 {} hook_state_lock_unavailable {}",
        REVIEW_GATE_FOLLOWUP_NEED_SEGMENT, REVIEW_GATE_FOLLOWUP_HINT_SEGMENT
    )
}

fn lock_failure_followup_for_stop(repo_root: &Path, event: &Value) -> String {
    if stop_lock_failure_is_fail_closed(repo_root, event) {
        return review_gate_stop_lock_unavailable_line();
    }
    state_lock_degraded_followup().to_string()
}
/// 将一条 `review_subagent_cycle_key` 压入 multiset 并同步 legacy 字段。
///
/// **双事件去重**：宿主可能对同一子代理先发 `subagentStart` 再发 `PostToolUse`（同一 `subagent_id`）。对 **`id:`** 前缀的稳定 key，若 pending 已含该字符串，则 **PostToolUse 路径不再 push**，避免「一次 stop 只核销一条」语义下出现双 pending。
///
/// **`subagent_start_count`** 仅在 **`handle_subagent_start`** 的 qualifying review 分支递增；PostToolUse 仅负责 multiset 入队（及 phase bump），**不**增加该计数，以免与宿主双事件重复计数。
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum PendingCyclePush {
    NewlyInserted,
    AlreadyPresent,
    AtCap,
}

/// Qualifying **`subagentStop`** or successful **`PostToolUse`** on a subagent lane: pop one matching
/// pending key; when the multiset empties, bump to phase 3 (parity with `handle_subagent_stop`).
/// PostTool settlement covers hosts that return Task results without a matching `subagentStop`.
fn try_settle_review_subagent_cycle(
    state: &mut ReviewGateState,
    cycle_key: &Option<String>,
    review_kind: bool,
) -> bool {
    if !review_hard_armed(state) || !review_kind || state.phase < 2 {
        return false;
    }
    let Some(k) = cycle_key.as_ref() else {
        return false;
    };
    let lite = core_policy::review_gate_engine::cursor_review_gate_mode()
        == core_policy::review_gate_engine::CursorReviewGateMode::Lite
        && core_policy::review_gate_engine::cycle_key_eligible_for_lite(k);
    let pending = if lite {
        &mut state.review_lite_pending_cycle_keys
    } else {
        &mut state.review_subagent_pending_cycle_keys
    };
    if !pending.iter().any(|p| p == k) {
        return false;
    }
    if let Some(pos) = pending.iter().position(|p| p == k) {
        pending.remove(pos);
    }
    sync_review_cycle_legacy_fields(state);
    if review_pending_both_empty(state) {
        state.review_pending_cap_refused = false;
        bump_phase(state, 3);
        state.subagent_stop_count = state.subagent_stop_count.saturating_add(1);
        state.lane_intent_matches = Some(true);
        state.independent_reviewer_seen = true;
        clear_review_gate_escalation_counters(state);
    }
    true
}

fn push_review_lite_pending_cycle_key(
    state: &mut ReviewGateState,
    k: String,
    from_posttool: bool,
) -> PendingCyclePush {
    if from_posttool && state.review_lite_pending_cycle_keys.contains(&k) {
        return PendingCyclePush::AlreadyPresent;
    }
    if !from_posttool
        && k.starts_with("id:")
        && state.review_lite_pending_cycle_keys.contains(&k)
    {
        return PendingCyclePush::AlreadyPresent;
    }
    let max = hooks::router_rs_cursor_review_pending_cycle_max();
    if state.review_lite_pending_cycle_keys.len() >= max {
        eprintln!("[router-rs] review_lite_pending_at_cap_refused cap={max} key={k}");
        state.review_pending_cap_refused = true;
        return PendingCyclePush::AtCap;
    }
    state.review_lite_pending_cycle_keys.push(k);
    state.review_pending_last_pushed_at = Some(Utc::now().to_rfc3339());
    PendingCyclePush::NewlyInserted
}

fn push_review_pending_cycle_key(
    state: &mut ReviewGateState,
    cycle_key: Option<String>,
    from_posttool: bool,
    lite_stable_id: bool,
) -> PendingCyclePush {
    let Some(k) = cycle_key else {
        return PendingCyclePush::AtCap;
    };
    if core_policy::review_gate_engine::cursor_review_gate_mode()
        == core_policy::review_gate_engine::CursorReviewGateMode::Lite
    {
        if core_policy::review_gate_engine::cycle_key_eligible_for_lite(&k) {
            if lite_stable_id {
                return push_review_lite_pending_cycle_key(state, k, from_posttool);
            }
            eprintln!("[router-rs] review_lite_reject_generic_id key={k}");
        } else {
            eprintln!("[router-rs] review_lite_fallback_strict reason=no_stable_id key={k}");
        }
    }
    if from_posttool && state.review_subagent_pending_cycle_keys.contains(&k) {
        return PendingCyclePush::AlreadyPresent;
    }
    if !from_posttool
        && k.starts_with("id:")
        && state.review_subagent_pending_cycle_keys.contains(&k)
    {
        return PendingCyclePush::AlreadyPresent;
    }
    let max = hooks::router_rs_cursor_review_pending_cycle_max();
    if state.review_subagent_pending_cycle_keys.len() >= max {
        eprintln!("[router-rs] review_pending_cycle_keys_at_cap_refused cap={max} key={k}");
        state.review_pending_cap_refused = true;
        return PendingCyclePush::AtCap;
    }
    state.review_subagent_pending_cycle_keys.push(k);
    state.review_pending_last_pushed_at = Some(Utc::now().to_rfc3339());
    sync_review_cycle_legacy_fields(state);
    PendingCyclePush::NewlyInserted
}

/// Clear pending review cycle keys when subagent activity is stale (avoids permanent REVIEW_GATE).
fn prune_stale_review_pending_cycle_keys(state: &mut ReviewGateState) {
    if state.review_subagent_pending_cycle_keys.is_empty()
        && state.review_lite_pending_cycle_keys.is_empty()
    {
        return;
    }
    let Some(stale_after_secs) = cursor_open_subagent_stale_after_secs() else {
        // Align with `reset_stale_active_subagents`: stale recovery off → do not prune pending.
        return;
    };
    if state.active_subagent_count == 0 {
        let raw = state
            .active_subagent_last_started_at
            .as_deref()
            .or(state.review_pending_last_pushed_at.as_deref());
        let Some(raw) = raw else {
            eprintln!(
                "[router-rs] review_pending_orphan_no_timestamp: skip clear (v1 migrate safety)"
            );
            return;
        };
        let clear = chrono::DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|started_at| {
                let age = Utc::now().signed_duration_since(started_at.with_timezone(&Utc));
                age.num_seconds() > stale_after_secs
            })
            .unwrap_or(false);
        if clear {
            // Anti false-negative: stale orphan recovery must not satisfy Stop without qualifying stop.
            if state.subagent_stop_count == 0 && state.phase >= 3 {
                state.phase = 2;
            }
            eprintln!(
                "[router-rs] cleared review_subagent_pending_cycle_keys (no open subagents, stale pending)"
            );
            state.review_subagent_pending_cycle_keys.clear();
            state.review_lite_pending_cycle_keys.clear();
            sync_review_cycle_legacy_fields(state);
        }
        return;
    }
    let Some(started_at) = state.active_subagent_last_started_at.as_deref() else {
        return;
    };
    let Ok(started_at) = chrono::DateTime::parse_from_rfc3339(started_at) else {
        return;
    };
    let age = Utc::now().signed_duration_since(started_at.with_timezone(&Utc));
    if age.num_seconds() <= stale_after_secs {
        return;
    }
    state.review_subagent_pending_cycle_keys.clear();
    state.review_lite_pending_cycle_keys.clear();
    sync_review_cycle_legacy_fields(state);
}

fn apply_subagent_stale_hygiene(state: &mut ReviewGateState) -> bool {
    let stale_reset = reset_stale_active_subagents(state);
    if stale_reset {
        state.review_subagent_pending_cycle_keys.clear();
        state.review_lite_pending_cycle_keys.clear();
        state.subagent_start_count = 0;
        state.subagent_stop_count = 0;
        if state.phase >= 2 {
            state.phase = 0;
            clear_review_gate_escalation_counters(state);
        }
        sync_review_cycle_legacy_fields(state);
    } else {
        prune_stale_review_pending_cycle_keys(state);
    }
    stale_reset
}
