use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const STATE_VERSION: u32 = 2;

fn compile_patterns(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|p| Regex::new(p).expect("invalid regex"))
        .collect()
}

fn review_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)\b(code|security|architecture|architect)\s+review\b",
            r"(?i)\breview\s+this\s+(pr|pull request)\b",
            r"(?i)\breview\s+(my\s+)?(pr|pull request)\b",
            r"(?i)\b(pr|pull request)\s+review\b",
            r"(?i)\breview\s+(code|security|architecture)\b",
            r"(?i)^\s*review\b.*\bagain\b",
            r"(?i)\bfocus on finding\b.*\bproblems\b",
            r"(?i)(深度|全面|全仓|仓库级|跨模块|多模块|多维)\s*review",
            r"(?i)review.*(仓库|全仓|跨模块|多模块|严重程度|findings|severity|repo|repository|cross[- ]module|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
            r"(?i)(深度|全面|全仓|仓库级|跨模块|多模块|多维).*(审查|审核|审计|评审)",
            r"(?i)(审查|审核|审计|评审).*(仓库|全仓|跨模块|多模块|严重程度|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
            r"(?i)(代码审查|安全审查|架构审查|审查这个\s*PR|审查这段代码)",
            r"(?i)(审查|评审|审核).*(PR|pull request|合并请求)",
        ])
    })
}

fn parallel_delegation_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)(并行|同时|分头|分路|分三路|多路|多线).*(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证|模块|方向)",
            r"(?i)(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证).*(并行|同时|分头|分路|分三路|多路|多线)",
            r"(?i)(多个|多条|多路|多维|多方向|独立).*(假设|模块|方向|维度|lane|lanes)",
            r"(?i)\b(parallel|concurrent|in parallel|split lanes|split work)\b.*\b(frontend|backend|test|testing|database|security|performance|architecture|implementation|verification)\b",
            r"(?i)\b(parallel|concurrent|in parallel|split lanes?|independent lanes?)\b",
            r"(?i)(并行|分路|分头|独立).*(lane|路线|路)",
        ])
    })
}

fn override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)do not use (a )?subagent",
            r"(?i)without (a )?subagent",
            r"(?i)handle (this|it) locally",
            r"(?i)do it yourself",
            r"(?i)no (parallel|delegation|delegating|split)",
            r"(?i)不要.*subagent",
            r"(?i)不用.*subagent",
            r"(?i)不要.*子代理",
            r"(?i)不用.*子代理",
            r"(?i)(你|你自己).*(本地处理|直接处理|自己做)",
            r"(?i)(不要|不用).*(分工|并行|分路|分头)",
        ])
    })
}

fn review_override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)do not use (a )?subagent",
            r"(?i)without (a )?subagent",
            r"(?i)handle (this|it) locally",
            r"(?i)do it yourself",
            r"(?i)不要.*subagent",
            r"(?i)不用.*subagent",
            r"(?i)不要.*子代理",
            r"(?i)不用.*子代理",
            r"(?i)(你|你自己).*(本地处理|直接处理|自己做)",
        ])
    })
}

fn delegation_override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)no (parallel|delegation|delegating|split)",
            r"(?i)(不要|不用).*(分工|并行|分路|分头)",
        ])
    })
}

fn reject_reason_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            "small_task",
            "shared_context_heavy",
            "write_scope_overlap",
            "next_step_blocked",
            "verification_missing",
            "token_overhead_dominates",
        ]
        .iter()
        .map(|reason| {
            Regex::new(&format!(
                "(?i)(^|[^a-z0-9_])({})($|[^a-z0-9_])",
                regex::escape(reason)
            ))
            .expect("invalid reject regex")
        })
        .collect()
    })
}

fn subagent_types() -> &'static [&'static str] {
    &[
        "generalpurpose",
        "explore",
        "shell",
        "browser-use",
        "browseruse",
        "cursor-guide",
        "cursorguide",
        "ci-investigator",
        "ciinvestigator",
        "best-of-n-runner",
        "bestofnrunner",
        "explorer",
    ]
}

fn subagent_tool_names() -> &'static [&'static str] {
    &[
        "task",
        "functions.task",
        "functions.subagent",
        "functions.spawn_agent",
        "subagent",
        "spawn_agent",
    ]
}

fn review_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\breview\b").expect("invalid regex"))
}

fn pr_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(pr|pull request)\b").expect("invalid regex"))
}

fn deep_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(深度|全面|全仓|跨模块|多模块|多维|架构|安全|回归风险|严重程度|findings)")
            .expect("invalid regex")
    })
}

fn narrow_review_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*review\s+(/|\.|[A-Za-z0-9_-].*\.(md|rs|tsx?|jsx?|py|json|toml))")
            .expect("invalid regex")
    })
}

fn framework_entrypoint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(^|\s)([/$])(autopilot|team|gitx)\b").expect("invalid regex"))
}

fn autopilot_entrypoint_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(^|\s)([/$])autopilot\b").expect("invalid regex"))
}

fn goal_contract_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(goal|done when|validation commands|checkpoint plan|non-goals)\b|(目标|完成条件|验证命令|检查点|非目标)",
        )
        .expect("invalid regex")
    })
}

fn goal_progress_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(checkpoint|milestone|progress|next step)\b|(检查点|里程碑|进度|下一步)")
            .expect("invalid regex")
    })
}

fn goal_verify_or_block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(verified|verification|test passed|blocker)\b|(已验证|验证通过|测试通过|阻塞)",
        )
        .expect("invalid regex")
    })
}

pub fn is_narrow_review_prompt(text: &str) -> bool {
    if !review_keyword_re().is_match(text) {
        return false;
    }
    if pr_keyword_re().is_match(text) {
        return false;
    }
    if deep_keyword_re().is_match(text) {
        return false;
    }
    narrow_review_prefix_re().is_match(text)
}

pub fn is_review_prompt(text: &str) -> bool {
    if is_narrow_review_prompt(text) {
        return false;
    }
    review_patterns().iter().any(|p| p.is_match(text))
}

pub fn is_parallel_delegation_prompt(text: &str) -> bool {
    parallel_delegation_patterns()
        .iter()
        .any(|p| p.is_match(text))
}

fn is_framework_entrypoint_prompt(text: &str) -> bool {
    framework_entrypoint_re().is_match(text)
}

fn is_autopilot_entrypoint_prompt(text: &str) -> bool {
    autopilot_entrypoint_re().is_match(text)
}

fn has_goal_contract_signal(text: &str) -> bool {
    goal_contract_re().is_match(text)
}

fn has_goal_progress_signal(text: &str) -> bool {
    goal_progress_re().is_match(text)
}

fn has_goal_verify_or_block_signal(text: &str) -> bool {
    goal_verify_or_block_re().is_match(text)
}

pub fn has_override(text: &str) -> bool {
    override_patterns().iter().any(|p| p.is_match(text))
}

pub fn has_review_override(text: &str) -> bool {
    review_override_patterns().iter().any(|p| p.is_match(text))
}

pub fn has_delegation_override(text: &str) -> bool {
    delegation_override_patterns()
        .iter()
        .any(|p| p.is_match(text))
}

pub fn saw_reject_reason(text: &str) -> bool {
    reject_reason_patterns().iter().any(|p| p.is_match(text))
}

pub fn normalize_subagent_type(value: Option<&str>) -> String {
    value
        .map(|s| s.trim().to_lowercase().replace('_', "-"))
        .unwrap_or_default()
}

pub fn normalize_tool_name(value: Option<&str>) -> String {
    value
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewGateState {
    pub version: u32,
    pub phase: u32,
    pub review_required: bool,
    pub delegation_required: bool,
    pub review_override: bool,
    pub delegation_override: bool,
    pub reject_reason_seen: bool,
    pub subagent_start_count: u32,
    pub subagent_stop_count: u32,
    pub followup_count: u32,
    pub review_followup_count: u32,
    pub goal_followup_count: u32,
    pub goal_required: bool,
    pub goal_contract_seen: bool,
    pub goal_progress_seen: bool,
    pub goal_verify_or_block_seen: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_subagent_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_subagent_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_intent_matches: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

fn prompt_text(event: &Value) -> String {
    for key in ["prompt", "user_prompt", "message", "input", "text"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    String::new()
}

fn agent_response_text(event: &Value) -> String {
    for key in ["response", "agent_response", "content", "text", "message", "output"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    String::new()
}

fn tool_name_of(event: &Value) -> String {
    event
        .get("tool_name")
        .or_else(|| event.get("tool"))
        .or_else(|| event.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn tool_input_of(event: &Value) -> Value {
    let value = event
        .get("tool_input")
        .or_else(|| event.get("input"))
        .or_else(|| event.get("arguments"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    if value.is_object() {
        value
    } else {
        json!({})
    }
}

fn loop_count_of(event: &Value) -> i64 {
    event
        .get("loop_count")
        .or_else(|| event.get("loopCount"))
        .or_else(|| event.get("loop"))
        .and_then(|v| {
            if let Some(n) = v.as_i64() {
                Some(n)
            } else {
                v.as_str().and_then(|s| s.parse::<i64>().ok())
            }
        })
        .unwrap_or(0)
}

fn session_key(event: &Value) -> String {
    for key in ["session_id", "conversation_id", "thread_id", "agent_id"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            if !value.trim().is_empty() {
                return short_hash(value);
            }
        }
    }
    if let Some(cwd) = event.get("cwd").and_then(Value::as_str) {
        if !cwd.trim().is_empty() {
            return short_hash(&format!("cwd::{cwd}"));
        }
    }
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0);
    short_hash(&format!("ephemeral::{micros}"))
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
        let _ = std::fmt::Write::write_fmt(&mut s, format_args!("{:02x}", byte));
    }
    s
}

fn state_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".cursor").join("hook-state")
}

fn state_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("review-subagent-{}.json", session_key(event)))
}

fn state_lock_path(repo_root: &Path, event: &Value) -> PathBuf {
    state_dir(repo_root).join(format!("review-subagent-{}.lock", session_key(event)))
}

struct LockGuard {
    path: PathBuf,
}

fn acquire_state_lock(repo_root: &Path, event: &Value) -> Option<LockGuard> {
    let dir = state_dir(repo_root);
    if fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let lock_path = state_lock_path(repo_root, event);
    for _ in 0..20 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(_) => return Some(LockGuard { path: lock_path }),
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    None
}

fn release_state_lock(lock: Option<LockGuard>) {
    if let Some(lock) = lock {
        let _ = fs::remove_file(lock.path);
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
        subagent_start_count: 0,
        subagent_stop_count: 0,
        followup_count: 0,
        review_followup_count: 0,
        goal_followup_count: 0,
        goal_required: false,
        goal_contract_seen: false,
        goal_progress_seen: false,
        goal_verify_or_block_seen: false,
        last_prompt: None,
        last_subagent_type: None,
        last_subagent_tool: None,
        lane_intent_matches: None,
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
    if raw.get("version").and_then(Value::as_u64).unwrap_or(0) < STATE_VERSION as u64 {
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
    if fs::write(&tmp, payload).is_err() {
        return false;
    }
    if fs::rename(&tmp, &target).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    true
}

fn is_armed(state: &ReviewGateState) -> bool {
    state.review_required || state.delegation_required
}

fn is_overridden(state: &ReviewGateState) -> bool {
    state.review_override || state.delegation_override
}

fn is_satisfied(state: &ReviewGateState) -> bool {
    if !is_armed(state) {
        return true;
    }
    if is_overridden(state) {
        return true;
    }
    if state.reject_reason_seen {
        return true;
    }
    state.phase >= 2
}

fn goal_is_satisfied(state: &ReviewGateState) -> bool {
    if !state.goal_required {
        return true;
    }
    if is_overridden(state) || state.reject_reason_seen {
        return true;
    }
    state.goal_contract_seen && state.goal_progress_seen && state.goal_verify_or_block_seen
}

fn bump_phase(state: &mut ReviewGateState, target: u32) {
    state.phase = state.phase.max(target);
}

fn goal_followup_message() -> &'static str {
    "Autopilot goal mode requires completion evidence before closeout. Provide: 1) goal contract (Goal/Done when/Validation commands), 2) checkpoint progress + next step, and 3) verification result or explicit blocker."
}

fn review_followup_message() -> &'static str {
    "Broad/deep review (or independent parallel lanes) was requested, but no independent subagent/sidecar was observed. Spawn a suitable subagent lane now, or explicitly state why spawning is rejected."
}

fn review_missing_parts(_state: &ReviewGateState) -> String {
    "independent_subagent_or_reject_reason".to_string()
}

fn goal_missing_parts(state: &ReviewGateState) -> String {
    let mut missing = Vec::new();
    if !state.goal_contract_seen {
        missing.push("goal_contract");
    }
    if !state.goal_progress_seen {
        missing.push("checkpoint_progress");
    }
    if !state.goal_verify_or_block_seen {
        missing.push("verification_or_blocker");
    }
    missing.join(", ")
}

fn state_lock_degraded_followup() -> &'static str {
    "Cursor review gate state lock is unavailable under .cursor/hook-state, so enforcement is fail-closed/degraded for this turn. Do not finalize until an independent subagent lane is observed or an explicit reject reason is stated in assistant output."
}

fn handle_before_submit(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({
            "continue": true,
            "followup_message": state_lock_degraded_followup()
        });
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let text = prompt_text(event);
    let review = is_review_prompt(&text);
    let framework_entrypoint = is_framework_entrypoint_prompt(&text);
    let autopilot_entrypoint = is_autopilot_entrypoint_prompt(&text);
    let delegation = is_parallel_delegation_prompt(&text) || framework_entrypoint;
    let review_override = has_review_override(&text) || has_override(&text);
    let delegation_override = has_delegation_override(&text) || has_override(&text);

    state.review_required = state.review_required || review;
    state.delegation_required = state.delegation_required || delegation;
    state.review_override = state.review_override || review_override;
    state.delegation_override = state.delegation_override || delegation_override;
    state.goal_required = state.goal_required || autopilot_entrypoint;
    state.goal_contract_seen = state.goal_contract_seen || has_goal_contract_signal(&text);
    state.goal_progress_seen = state.goal_progress_seen || has_goal_progress_signal(&text);
    state.goal_verify_or_block_seen =
        state.goal_verify_or_block_seen || has_goal_verify_or_block_signal(&text);
    if review || delegation {
        state.last_prompt = Some(text.chars().take(500).collect());
    }
    if is_armed(&state) && !is_overridden(&state) && !state.reject_reason_seen {
        bump_phase(&mut state, 1);
    }

    let persisted = save_state(repo_root, event, &mut state);
    release_state_lock(lock);

    let needs_followup = is_armed(&state)
        && !is_overridden(&state)
        && !state.reject_reason_seen
        && state.phase < 2;
    let mut output = json!({ "continue": true });
    if needs_followup {
        let is_first_followup = state.review_followup_count == 0;
        state.followup_count += 1;
        state.review_followup_count += 1;
        let msg = if is_first_followup {
            if state.review_required {
                "Broad/deep review detected. Spawn an independent reviewer subagent lane now; if you will not spawn, provide one explicit reject reason before finalizing.".to_string()
            } else {
                "Parallel lane request detected. Spawn bounded subagent lanes now; if you will not spawn, provide one explicit reject reason before finalizing.".to_string()
            }
        } else {
            format!("RG_FOLLOWUP missing_parts={}", review_missing_parts(&state))
        };
        output["followup_message"] = Value::String(msg);
    }
    let persisted_after_followup = if needs_followup {
        save_state(repo_root, event, &mut state)
    } else {
        persisted
    };
    if !persisted || !persisted_after_followup {
        let warning = "Cursor review gate state could not be persisted under .cursor/hook-state. Review/delegation enforcement may be degraded for this turn.";
        let merged = output
            .get("followup_message")
            .and_then(Value::as_str)
            .map(|s| format!("{s} {warning}"))
            .unwrap_or_else(|| warning.to_string());
        output["followup_message"] = Value::String(merged.trim().to_string());
    }
    output
}

fn handle_subagent_start(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({});
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = is_armed(&state);
    let mut mutated = false;
    if armed {
        bump_phase(&mut state, 2);
        state.subagent_start_count += 1;
        state.lane_intent_matches = Some(true);
        mutated = true;
    }
    let sub_type = normalize_subagent_type(event.get("subagent_type").and_then(Value::as_str));
    let agent_type = normalize_subagent_type(event.get("agent_type").and_then(Value::as_str));
    if armed && (!sub_type.is_empty() || !agent_type.is_empty()) {
        state.last_subagent_type = Some(if !sub_type.is_empty() {
            sub_type
        } else {
            agent_type
        });
        mutated = true;
    }
    if mutated {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);
    json!({})
}

fn handle_subagent_stop(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({});
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    if is_armed(&state) {
        bump_phase(&mut state, 3);
        state.subagent_stop_count += 1;
        state.lane_intent_matches = Some(true);
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);
    json!({})
}

fn handle_post_tool_use(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({});
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = is_armed(&state);
    let name = normalize_tool_name(Some(&tool_name_of(event)));
    let tool_input = tool_input_of(event);
    let sub_type = normalize_subagent_type(
        tool_input
            .get("subagent_type")
            .or_else(|| tool_input.get("subagentType"))
            .or_else(|| event.get("subagent_type"))
            .or_else(|| event.get("subagentType"))
            .and_then(Value::as_str),
    );
    let agent_type = normalize_subagent_type(
        tool_input
            .get("agent_type")
            .or_else(|| tool_input.get("agentType"))
            .or_else(|| event.get("agent_type"))
            .or_else(|| event.get("agentType"))
            .and_then(Value::as_str),
    );
    let typed_subagent = (!sub_type.is_empty() && subagent_types().contains(&sub_type.as_str()))
        || (!agent_type.is_empty() && subagent_types().contains(&agent_type.as_str()));
    if subagent_tool_names().contains(&name.as_str()) && typed_subagent && armed {
        bump_phase(&mut state, 2);
        state.subagent_start_count += 1;
        state.last_subagent_tool = Some(name);
        if !sub_type.is_empty() || !agent_type.is_empty() {
            state.last_subagent_type = Some(if !sub_type.is_empty() {
                sub_type
            } else {
                agent_type
            });
        }
        state.lane_intent_matches = Some(true);
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);
    json!({})
}

fn handle_after_agent_response(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({});
    }
    let mut state = load_state(repo_root, event)
        .ok()
        .flatten()
        .unwrap_or_else(empty_state);
    let armed = is_armed(&state);
    let text = agent_response_text(event);
    if armed && saw_reject_reason(&text) {
        state.reject_reason_seen = true;
        state.goal_verify_or_block_seen = true;
    }
    if armed && has_goal_contract_signal(&text) {
        state.goal_contract_seen = true;
    }
    if armed && has_goal_progress_signal(&text) {
        state.goal_progress_seen = true;
    }
    if armed && has_goal_verify_or_block_signal(&text) {
        state.goal_verify_or_block_seen = true;
    }
    if armed
        && (state.reject_reason_seen
            || state.goal_contract_seen
            || state.goal_progress_seen
            || state.goal_verify_or_block_seen)
    {
        let _ = save_state(repo_root, event, &mut state);
    }
    release_state_lock(lock);
    json!({})
}

fn handle_stop(repo_root: &Path, event: &Value) -> Value {
    let lock = acquire_state_lock(repo_root, event);
    if lock.is_none() {
        return json!({
            "followup_message": state_lock_degraded_followup()
        });
    }
    let loaded = load_state(repo_root, event);
    let text = prompt_text(event);
    let inferred_required = is_review_prompt(&text)
        || is_parallel_delegation_prompt(&text)
        || is_framework_entrypoint_prompt(&text);
    let inferred_overridden = has_review_override(&text)
        || has_delegation_override(&text)
        || has_override(&text);
    let loop_count = loop_count_of(event);
    let output = match loaded {
        Ok(None) => {
            if inferred_required && !inferred_overridden {
                json!({
                    "followup_message": "Cursor review gate state is missing under .cursor/hook-state, so enforcement cannot be verified for this turn. Re-run with subagent lanes or an explicit reject reason before finalizing."
                })
            } else {
                json!({})
            }
        }
        Err(io_error) => json!({
            "followup_message": format!(
                "Cursor review gate state is unreadable under .cursor/hook-state ({}). Enforcement degraded; check hook-state permissions and JSON integrity.",
                io_error
            )
        }),
        Ok(Some(mut state)) => {
            if has_review_override(&text) || has_override(&text) {
                state.review_override = true;
            }
            if has_delegation_override(&text) || has_override(&text) {
                state.delegation_override = true;
            }
            if has_goal_contract_signal(&text) {
                state.goal_contract_seen = true;
            }
            if has_goal_progress_signal(&text) {
                state.goal_progress_seen = true;
            }
            if has_goal_verify_or_block_signal(&text) {
                state.goal_verify_or_block_seen = true;
            }
            if !is_satisfied(&state) {
                let is_first_followup = state.review_followup_count == 0;
                state.followup_count += 1;
                state.review_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                let escalation = if loop_count >= 3 || state.followup_count >= 3 {
                    "This has already looped multiple times; do not silently continue. "
                } else {
                    ""
                };
                let message = if is_first_followup {
                    format!("{} {}", review_followup_message(), escalation).trim().to_string()
                } else {
                    format!(
                        "RG_FOLLOWUP missing_parts={}{}",
                        review_missing_parts(&state),
                        if escalation.is_empty() {
                            String::new()
                        } else {
                            format!(" escalation={}", escalation.trim())
                        }
                    )
                };
                json!({
                    "followup_message": message
                })
            } else if !goal_is_satisfied(&state) {
                let is_first_followup = state.goal_followup_count == 0;
                state.followup_count += 1;
                state.goal_followup_count += 1;
                let _ = save_state(repo_root, event, &mut state);
                let message = if is_first_followup {
                    format!("{} Missing: {}.", goal_followup_message(), goal_missing_parts(&state))
                } else {
                    format!(
                        "AG_FOLLOWUP missing_parts={}",
                        goal_missing_parts(&state)
                    )
                };
                json!({
                    "followup_message": message
                })
            } else {
                let mut reset = empty_state();
                let _ = save_state(repo_root, event, &mut reset);
                json!({})
            }
        }
    };
    release_state_lock(lock);
    output
}

fn handle_pre_compact(repo_root: &Path, event: &Value) -> Value {
    let Ok(Some(state)) = load_state(repo_root, event) else {
        return json!({});
    };
    let summary = format!(
        "Cursor review gate state (preserved across compaction): phase={} review_required={} delegation_required={} override={} rejected={} subagent_starts={} subagent_stops={}",
        state.phase,
        state.review_required,
        state.delegation_required,
        is_overridden(&state),
        state.reject_reason_seen,
        state.subagent_start_count,
        state.subagent_stop_count
    );
    json!({ "additional_context": summary })
}

fn handle_session_end(repo_root: &Path, event: &Value) -> Value {
    let _ = fs::remove_file(state_path(repo_root, event));
    json!({})
}

fn dispatch_event(repo_root: &Path, event_name: &str, payload: &Value) -> Value {
    match event_name.trim().to_lowercase().as_str() {
        "beforesubmitprompt" | "userpromptsubmit" => handle_before_submit(repo_root, payload),
        "subagentstart" => handle_subagent_start(repo_root, payload),
        "subagentstop" => handle_subagent_stop(repo_root, payload),
        "posttooluse" => handle_post_tool_use(repo_root, payload),
        "afteragentresponse" => handle_after_agent_response(repo_root, payload),
        "stop" => handle_stop(repo_root, payload),
        "precompact" => handle_pre_compact(repo_root, payload),
        "sessionend" => handle_session_end(repo_root, payload),
        _ => json!({}),
    }
}

fn read_stdin_json() -> Result<Value, String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| e.to_string())?;
    if buf.trim().is_empty() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_str(&buf).map_err(|_| "stdin_json_invalid".to_string())?;
    if value.is_object() {
        Ok(value)
    } else {
        Ok(json!({}))
    }
}

pub fn run_cursor_review_gate(event: &str, repo_root: &Path) -> Result<(), String> {
    let payload = read_stdin_json().unwrap_or_else(|_| json!({}));
    let output = dispatch_event(repo_root, event, &payload);
    let mut stdout = std::io::stdout();
    let serialized = serde_json::to_string(&output).map_err(|e| e.to_string())?;
    stdout
        .write_all(format!("{serialized}\n").as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::{env, fs};

    fn fresh_repo() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "router-rs-cursor-hooks-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_micros()
        ));
        fs::create_dir_all(root.join(".cursor/hooks")).expect("mkdir hooks");
        fs::write(
            root.join(".cursor/hooks/review_subagent_gate.py"),
            b"# stub",
        )
        .expect("write hook");
        root
    }

    fn event(session: &str, prompt: &str) -> Value {
        json!({
            "session_id": session,
            "cwd": "/Users/joe/Documents/skill",
            "prompt": prompt
        })
    }

    fn load_state_for(repo: &Path, session: &str) -> ReviewGateState {
        let payload = json!({ "session_id": session, "cwd": "/Users/joe/Documents/skill" });
        load_state(repo, &payload)
            .expect("load ok")
            .expect("state exists")
    }

    #[test]
    fn review_prompt_chinese_full_review_arms_state() {
        let repo = fresh_repo();
        let out = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s1", "请全面review这个仓库找bug"),
        );
        assert!(out.get("followup_message").is_some());
        let state = load_state_for(&repo, "s1");
        assert_eq!(state.phase, 1);
        assert!(state.review_required);
    }

    #[test]
    fn parallel_delegation_arms_delegation() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s2", "请前端后端测试并行分头执行"),
        );
        let state = load_state_for(&repo, "s2");
        assert!(state.delegation_required);
        assert_eq!(state.phase, 1);
    }

    #[test]
    fn override_phrase_in_chinese_disables_arming() {
        let repo = fresh_repo();
        let out = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s3", "全面review这个仓库，不要用子代理"),
        );
        assert!(out.get("followup_message").is_none());
        let state = load_state_for(&repo, "s3");
        assert!(state.review_override);
        assert_eq!(state.phase, 0);
    }

    #[test]
    fn reject_reason_satisfies_stop() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s4", "全面review这个仓库"),
        );
        let _ = dispatch_event(
            &repo,
            "afterAgentResponse",
            &json!({ "session_id": "s4", "response": "reject reason: small_task" }),
        );
        let out = dispatch_event(
            &repo,
            "stop",
            &event("s4", "reject reason: small_task"),
        );
        assert_eq!(out, json!({}));
    }

    #[test]
    fn reject_reason_in_user_prompt_does_not_satisfy_gate() {
        let repo = fresh_repo();
        let _ = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s13", "全面review这个仓库"),
        );
        let out = dispatch_event(
            &repo,
            "stop",
            &event("s13", "reject reason: small_task"),
        );
        let followup = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(followup.contains("Broad/deep review") || followup.contains("RG_FOLLOWUP"));
        let state = load_state_for(&repo, "s13");
        assert!(!state.reject_reason_seen);
    }

    #[test]
    fn before_submit_lock_failure_fails_closed_without_writing_state() {
        let repo = fresh_repo();
        let payload = event("s14", "全面review这个仓库");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        fs::write(&lock_path, b"locked").expect("seed lock");
        let out = dispatch_event(&repo, "beforeSubmitPrompt", &payload);
        assert!(out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("state lock is unavailable"));
        assert!(!state_path(&repo, &payload).exists());
    }

    #[test]
    fn stop_lock_failure_reports_degraded_followup() {
        let repo = fresh_repo();
        let payload = event("s15", "全面review这个仓库");
        let lock_path = state_lock_path(&repo, &payload);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("mkdir");
        fs::write(&lock_path, b"locked").expect("seed lock");
        let out = dispatch_event(&repo, "stop", &payload);
        assert!(out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("state lock is unavailable"));
    }

    #[test]
    fn subagent_start_promotes_phase_to_2() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("s5", "全面review这个仓库"));
        let _ = dispatch_event(
            &repo,
            "subagentStart",
            &json!({ "session_id": "s5", "subagent_type": "explore" }),
        );
        let state = load_state_for(&repo, "s5");
        assert_eq!(state.phase, 2);
        assert_eq!(state.subagent_start_count, 1);
    }

    #[test]
    fn subagent_stop_promotes_phase_to_3() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("s6", "全面review这个仓库"));
        let _ = dispatch_event(
            &repo,
            "subagentStop",
            &json!({ "session_id": "s6", "subagent_type": "explore" }),
        );
        let state = load_state_for(&repo, "s6");
        assert_eq!(state.phase, 3);
        assert_eq!(state.subagent_stop_count, 1);
    }

    #[test]
    fn stop_without_subagent_emits_followup() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("s7", "全面review这个仓库"));
        let out = dispatch_event(&repo, "stop", &event("s7", "继续"));
        let msg = out
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(msg.contains("Broad/deep review") || msg.starts_with("RG_FOLLOWUP missing_parts="));
    }

    #[test]
    fn pre_compact_emits_additional_context_summary() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("s8", "全面review这个仓库"));
        let out = dispatch_event(
            &repo,
            "preCompact",
            &json!({ "session_id": "s8", "cwd": "/Users/joe/Documents/skill" }),
        );
        assert!(out
            .get("additional_context")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("phase=1"));
    }

    #[test]
    fn session_end_clears_state_file() {
        let repo = fresh_repo();
        let payload = event("s9", "全面review这个仓库");
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &payload);
        let path = state_path(&repo, &payload);
        assert!(path.exists());
        let _ = dispatch_event(&repo, "sessionEnd", &payload);
        assert!(!path.exists());
    }

    #[test]
    fn narrow_path_review_does_not_arm() {
        let repo = fresh_repo();
        let out = dispatch_event(
            &repo,
            "beforeSubmitPrompt",
            &event("s10", "review ./README.md"),
        );
        assert_eq!(out, json!({ "continue": true }));
        let state = load_state_for(&repo, "s10");
        assert!(!state.review_required);
        assert_eq!(state.phase, 0);
    }

    #[test]
    fn v1_state_migrates_to_v2_phase() {
        let repo = fresh_repo();
        let payload = json!({ "session_id": "s11" });
        let path = state_path(&repo, &payload);
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(
            &path,
            r#"{"version":1,"review_required":true,"review_subagent_seen":true,"followup_count":2}"#,
        )
        .expect("write v1");
        let state = load_state(&repo, &payload)
            .expect("load")
            .expect("state");
        assert_eq!(state.version, 2);
        assert_eq!(state.phase, 2);
        assert_eq!(state.followup_count, 2);
    }

    #[test]
    fn post_tool_use_subagent_sets_phase() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("s12", "全面review这个仓库"));
        let _ = dispatch_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id":"s12",
                "tool_name":"functions.subagent",
                "tool_input":{"subagent_type":"explore"}
            }),
        );
        let state = load_state_for(&repo, "s12");
        assert!(state.phase >= 2);
    }

    #[test]
    fn review_followup_is_detailed_then_short_code() {
        let repo = fresh_repo();
        let first = dispatch_event(&repo, "beforeSubmitPrompt", &event("s16", "全面review这个仓库"));
        let first_msg = first
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(first_msg.contains("Broad/deep review"));
        let second = dispatch_event(&repo, "stop", &event("s16", "继续"));
        let second_msg = second
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(second_msg.starts_with("RG_FOLLOWUP missing_parts="));
    }

    #[test]
    fn goal_followup_is_detailed_then_short_code() {
        let repo = fresh_repo();
        let _ = dispatch_event(&repo, "beforeSubmitPrompt", &event("s17", "/autopilot 完成任务"));
        let _ = dispatch_event(
            &repo,
            "postToolUse",
            &json!({
                "session_id":"s17",
                "tool_name":"functions.subagent",
                "tool_input":{"subagent_type":"explore"}
            }),
        );
        let first = dispatch_event(&repo, "stop", &event("s17", "继续"));
        let first_msg = first
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(first_msg.contains("Autopilot goal mode requires completion evidence"));
        let second = dispatch_event(&repo, "stop", &event("s17", "继续"));
        let second_msg = second
            .get("followup_message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(second_msg.starts_with("AG_FOLLOWUP missing_parts="));
    }
}
