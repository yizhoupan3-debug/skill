//! Autopilot 宏目标：Rust 真源 `GOAL_STATE.json` + stdio 控制面 + Cursor hook 续跑提示。
//! 不替代 LLM 执行，但把「未完成不得停」写成可校验文件态并由 hook 注入跟进。

use crate::framework_runtime::resolve_repo_root_arg;
use crate::router_env_flags::{router_rs_env_enabled_default_true, router_rs_goal_prompt_verbose};
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const GOAL_STATE_FILENAME: &str = "GOAL_STATE.json";
pub const GOAL_STATE_SCHEMA_VERSION: &str = "router-rs-autopilot-goal-v1";
pub const EVIDENCE_INDEX_FILENAME: &str = "EVIDENCE_INDEX.json";
const AUTOPILOT_DRIVE_HOOK_ENV: &str = "ROUTER_RS_AUTOPILOT_DRIVE_HOOK";

fn autopilot_drive_hook_enabled() -> bool {
    router_rs_env_enabled_default_true(AUTOPILOT_DRIVE_HOOK_ENV)
}

fn write_atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| format!("serialize GOAL_STATE failed: {err}"))?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, text)
        .map_err(|err| format!("write temp file failed for {}: {err}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!(
            "rename temp file failed {} -> {}: {err}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

/// 从 `artifacts/current/active_task.json` 读取 `task_id`。
pub fn read_active_task_id(repo_root: &Path) -> Option<String> {
    let path = repo_root.join("artifacts/current/active_task.json");
    let raw = fs::read_to_string(&path).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    data.get("task_id")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// 从 `artifacts/current/focus_task.json` 读取 `task_id`（与 framework 指针一致，作 active 指针缺失时的回退）。
pub fn read_focus_task_id(repo_root: &Path) -> Option<String> {
    let path = repo_root.join("artifacts/current/focus_task.json");
    let raw = fs::read_to_string(&path).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    data.get("task_id")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn goal_state_path_for_task(repo_root: &Path, task_id: &str) -> PathBuf {
    repo_root
        .join("artifacts/current")
        .join(task_id)
        .join(GOAL_STATE_FILENAME)
}

/// RFV 在同 task 上 `start`/`upsert` 时移除 GOAL（与 RFV 互斥）。
pub(crate) fn deactivate_goal_for_conflict_with_rfv(
    repo_root: &Path,
    task_id: &str,
) -> Result<bool, String> {
    if task_id.trim().is_empty() {
        return Ok(false);
    }
    let path = goal_state_path_for_task(repo_root, task_id);
    if !path.is_file() {
        return Ok(false);
    }
    fs::remove_file(&path).map_err(|e| format!("remove GOAL_STATE for RFV mutex: {e}"))?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, task_id);
    Ok(true)
}

/// 能解析为 JSON 的 `GOAL_STATE` 才返回；读失败或非法 JSON 返回 `None`（便于换指针/扫描回退）。
fn read_goal_state_pair_if_valid(repo_root: &Path, task_id: &str) -> Option<(Value, String)> {
    if task_id.trim().is_empty() {
        return None;
    }
    let path = goal_state_path_for_task(repo_root, task_id);
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(&path).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    Some((value, task_id.trim().to_string()))
}

const GOAL_DISCOVER_MAX_DEPTH: usize = 8;

fn discover_goal_state_task_ids_under_current(
    repo_root: &Path,
) -> Result<Vec<(String, SystemTime)>, String> {
    let current = repo_root.join("artifacts/current");
    if !current.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    visit_goal_state_dirs(&current, &current, GOAL_DISCOVER_MAX_DEPTH, &mut out)?;
    Ok(out)
}

fn visit_goal_state_dirs(
    dir: &Path,
    current_root: &Path,
    depth: usize,
    out: &mut Vec<(String, SystemTime)>,
) -> Result<(), String> {
    if depth == 0 {
        return Ok(());
    }
    let goal_path = dir.join(GOAL_STATE_FILENAME);
    if goal_path.is_file() {
        if let Ok(rel) = dir.strip_prefix(current_root) {
            let tid_norm = rel
                .to_str()
                .map(|s| s.trim().replace('\\', "/"))
                .filter(|s| !s.is_empty());
            if let Some(tid_norm) = tid_norm {
                let mtime = fs::metadata(&goal_path)
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                out.push((tid_norm, mtime));
            }
        }
    }
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("read_dir entry: {e}"))?;
        let p = entry.path();
        if p.is_dir() {
            visit_goal_state_dirs(&p, current_root, depth - 1, out)?;
        }
    }
    Ok(())
}

/// Cursor Stop 门控回补：依次尝试 `active_task.json`、`focus_task.json`；仍无时递归扫描
/// `artifacts/current/**/GOAL_STATE.json`（深度上限见常量），按 mtime 新者优先；非法 JSON 不短路，继续尝试其它路径。
pub fn read_goal_state_for_hydration(repo_root: &Path) -> Result<Option<(Value, String)>, String> {
    let mut try_ids: Vec<String> = Vec::new();
    for tid in [
        read_active_task_id(repo_root),
        read_focus_task_id(repo_root),
    ] {
        let Some(t) = tid else {
            continue;
        };
        if !try_ids.iter().any(|x| x == &t) {
            try_ids.push(t);
        }
    }
    for tid in &try_ids {
        if let Some(pair) = read_goal_state_pair_if_valid(repo_root, tid) {
            return Ok(Some(pair));
        }
    }

    let mut candidates = discover_goal_state_task_ids_under_current(repo_root)?;
    if candidates.is_empty() {
        return Ok(None);
    }
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    for (tid, _) in candidates {
        if let Some(pair) = read_goal_state_pair_if_valid(repo_root, &tid) {
            return Ok(Some(pair));
        }
    }
    Ok(None)
}

fn json_exit_code_is_zero(v: &Value) -> bool {
    v.as_i64() == Some(0) || v.as_u64() == Some(0)
}

fn evidence_entry_implies_success(entry: &Value) -> bool {
    entry
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || entry
            .get("exit_code")
            .map(json_exit_code_is_zero)
            .unwrap_or(false)
}

/// 指定 `task_id` 任务目录下 `EVIDENCE_INDEX.json`：是否存在非空 `artifacts`、是否至少有一条成功验证记录。
pub fn task_evidence_artifacts_summary_for_task(repo_root: &Path, task_id: &str) -> (bool, bool) {
    if task_id.trim().is_empty() {
        return (false, false);
    }
    let goal_path = goal_state_path_for_task(repo_root, task_id);
    let Some(parent) = goal_path.parent() else {
        return (false, false);
    };
    let path = parent.join(EVIDENCE_INDEX_FILENAME);
    if !path.is_file() {
        return (false, false);
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return (false, false);
    };
    let Ok(val) = serde_json::from_str::<Value>(&raw) else {
        return (false, false);
    };
    let Some(arr) = val.get("artifacts").and_then(Value::as_array) else {
        return (false, false);
    };
    if arr.is_empty() {
        return (false, false);
    }
    let any_ok = arr.iter().any(evidence_entry_implies_success);
    (true, any_ok)
}

pub fn read_goal_state(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Result<Option<Value>, String> {
    let task_id = if let Some(t) = task_id_override {
        if t.trim().is_empty() {
            return Err("framework_autopilot_goal: task_id override is empty".to_string());
        }
        t.trim().to_string()
    } else {
        let Some(t) = read_active_task_id(repo_root) else {
            return Ok(None);
        };
        t
    };
    let path = goal_state_path_for_task(repo_root, &task_id);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("read GOAL_STATE: {err}"))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|err| format!("parse GOAL_STATE: {err}"))?;
    Ok(Some(value))
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn base_goal_object(
    goal: String,
    non_goals: Vec<Value>,
    done_when: Vec<Value>,
    validation_commands: Vec<Value>,
    drive_until_done: bool,
    current_horizon: Option<String>,
) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(
        "schema_version".to_string(),
        json!(GOAL_STATE_SCHEMA_VERSION),
    );
    m.insert("drive_until_done".to_string(), json!(drive_until_done));
    m.insert("status".to_string(), json!("running"));
    m.insert("goal".to_string(), json!(goal));
    m.insert("non_goals".to_string(), Value::Array(non_goals));
    m.insert("done_when".to_string(), Value::Array(done_when));
    m.insert(
        "validation_commands".to_string(),
        Value::Array(validation_commands),
    );
    m.insert(
        "current_horizon".to_string(),
        json!(current_horizon.unwrap_or_default()),
    );
    m.insert("checkpoints".to_string(), json!([]));
    m.insert("blocker".to_string(), Value::Null);
    m.insert("updated_at".to_string(), json!(now_iso()));
    m
}

fn value_string_list(payload: &Value, key: &str) -> Vec<Value> {
    payload
        .get(key)
        .and_then(|v| {
            if let Some(arr) = v.as_array() {
                Some(
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(|s| json!(s))
                        .collect(),
                )
            } else if let Some(s) = v.as_str() {
                Some(vec![json!(s)])
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// stdio / CLI：`framework_autopilot_goal`
pub fn framework_autopilot_goal(payload: Value) -> Result<Value, String> {
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    if operation == "status" {
        framework_autopilot_goal_impl(payload)
    } else {
        crate::task_write_lock::apply_task_ledger_mutation(|| {
            framework_autopilot_goal_impl(payload)
        })
    }
}

fn framework_autopilot_goal_impl(payload: Value) -> Result<Value, String> {
    let repo_root = payload
        .get("repo_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "framework_autopilot_goal requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(format!(
            "framework_autopilot_goal: repo_root is not a directory: {}",
            repo_root.display()
        ));
    }
    let repo_root = resolve_repo_root_arg(Some(repo_root.as_path()))?;
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();

    let task_id_override = payload
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match operation.as_str() {
        "status" => {
            let state = read_goal_state(&repo_root, task_id_override)?;
            let tid = if let Some(t) = task_id_override {
                t.to_string()
            } else {
                read_active_task_id(&repo_root).unwrap_or_default()
            };
            let path = if tid.is_empty() {
                PathBuf::new()
            } else {
                goal_state_path_for_task(&repo_root, &tid)
            };
            Ok(json!({
                "ok": true,
                "operation": "status",
                "task_id": tid,
                "goal_state_path": path.display().to_string(),
                "goal_state": state,
            }))
        }
        "start" | "upsert" => {
            let task_id = task_id_override
                .map(|s| s.to_string())
                .or_else(|| read_active_task_id(&repo_root))
                .ok_or_else(|| {
                    "framework_autopilot_goal start requires task_id in payload or active_task.json"
                        .to_string()
                })?;
            let goal = payload
                .get("goal")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    "framework_autopilot_goal start requires non-empty goal".to_string()
                })?;
            let drive_until_done = payload
                .get("drive_until_done")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let mut obj = base_goal_object(
                goal.to_string(),
                value_string_list(&payload, "non_goals"),
                value_string_list(&payload, "done_when"),
                value_string_list(&payload, "validation_commands"),
                drive_until_done,
                payload
                    .get("current_horizon")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
            );
            if let Some(extra) = payload.get("metadata").cloned() {
                obj.insert("metadata".to_string(), extra);
            }
            let path = goal_state_path_for_task(&repo_root, &task_id);
            let value = Value::Object(obj);
            write_atomic_json(&path, &value)?;
            let rfv_loop_superseded =
                crate::rfv_loop::deactivate_rfv_for_conflict_with_autopilot(&repo_root, &task_id)?;
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            Ok(json!({
                "ok": true,
                "operation": "start",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
                "goal_state": value,
                "rfv_loop_superseded": rfv_loop_superseded,
            }))
        }
        "checkpoint" => {
            let task_id = task_id_override
                .map(|s| s.to_string())
                .or_else(|| read_active_task_id(&repo_root))
                .ok_or_else(|| {
                    "framework_autopilot_goal checkpoint requires task_id or active_task.json"
                        .to_string()
                })?;
            let note = payload
                .get("note")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    "framework_autopilot_goal checkpoint requires non-empty note".to_string()
                })?;
            let path = goal_state_path_for_task(&repo_root, &task_id);
            let mut state = read_goal_state(&repo_root, Some(&task_id))?
                .ok_or_else(|| format!("GOAL_STATE missing at {}", path.display()))?;
            let arr = state
                .as_object_mut()
                .and_then(|o| o.get_mut("checkpoints"))
                .and_then(|c| c.as_array_mut())
                .ok_or_else(|| "GOAL_STATE.checkpoints corrupt".to_string())?;
            arr.push(json!({"at": now_iso(), "note": note}));
            if let Some(o) = state.as_object_mut() {
                o.insert("updated_at".to_string(), json!(now_iso()));
            }
            write_atomic_json(&path, &state)?;
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            Ok(json!({
                "ok": true,
                "operation": "checkpoint",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
                "goal_state": state,
            }))
        }
        "pause" => set_terminal_flags(&repo_root, task_id_override, "paused", Some(false), None),
        "resume" => resume_goal_running(&repo_root, task_id_override),
        "complete" => {
            set_terminal_flags(&repo_root, task_id_override, "completed", Some(false), None)
        }
        "block" => {
            let blocker = payload
                .get("blocker")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    "framework_autopilot_goal block requires non-empty blocker".to_string()
                })?;
            set_terminal_flags(
                &repo_root,
                task_id_override,
                "blocked",
                Some(false),
                Some(blocker.to_string()),
            )
        }
        "clear" => clear_goal_state(&repo_root, task_id_override),
        _ => Err(format!(
            "framework_autopilot_goal: unknown operation '{operation}'"
        )),
    }
}

/// 去掉 `followup_message` 中以某前缀开头的段落（`\n\n` 分隔），用于刷新 AUTOPILOT/RFV 合并文案。
pub(crate) fn strip_followup_paragraphs_with_line_prefix(
    text: &str,
    first_line_prefix: &str,
) -> String {
    text.split("\n\n")
        .filter(|seg| {
            !seg.lines()
                .next()
                .map(|l| l.trim_start().starts_with(first_line_prefix))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn clear_goal_state(repo_root: &Path, task_id_override: Option<&str>) -> Result<Value, String> {
    let task_id = task_id_override
        .map(|s| s.to_string())
        .or_else(|| read_active_task_id(repo_root))
        .ok_or_else(|| {
            "framework_autopilot_goal clear requires task_id or active_task.json".to_string()
        })?;
    let path = goal_state_path_for_task(repo_root, &task_id);
    let existed = path.is_file();
    if existed {
        fs::remove_file(&path).map_err(|err| format!("remove GOAL_STATE: {err}"))?;
    }
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &task_id);
    Ok(json!({
        "ok": true,
        "operation": "clear",
        "task_id": task_id,
        "goal_state_path": path.display().to_string(),
        "removed": existed,
    }))
}

fn resume_goal_running(repo_root: &Path, task_id_override: Option<&str>) -> Result<Value, String> {
    let task_id = task_id_override
        .map(|s| s.to_string())
        .or_else(|| read_active_task_id(repo_root))
        .ok_or_else(|| {
            "framework_autopilot_goal requires task_id or active_task.json".to_string()
        })?;
    let path = goal_state_path_for_task(repo_root, &task_id);
    let mut state = read_goal_state(repo_root, Some(&task_id))?
        .ok_or_else(|| format!("GOAL_STATE missing at {}", path.display()))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "GOAL_STATE root must be object".to_string())?;
    obj.insert("status".to_string(), json!("running"));
    obj.insert("updated_at".to_string(), json!(now_iso()));
    write_atomic_json(&path, &state)?;
    let rfv_loop_superseded =
        crate::rfv_loop::deactivate_rfv_for_conflict_with_autopilot(repo_root, &task_id)?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &task_id);
    Ok(json!({
        "ok": true,
        "operation": "resume",
        "task_id": task_id,
        "goal_state_path": path.display().to_string(),
        "goal_state": state,
        "rfv_loop_superseded": rfv_loop_superseded,
    }))
}

fn set_terminal_flags(
    repo_root: &Path,
    task_id_override: Option<&str>,
    status: &str,
    drive_until_done: Option<bool>,
    blocker: Option<String>,
) -> Result<Value, String> {
    let task_id = task_id_override
        .map(|s| s.to_string())
        .or_else(|| read_active_task_id(repo_root))
        .ok_or_else(|| {
            "framework_autopilot_goal requires task_id or active_task.json".to_string()
        })?;
    let path = goal_state_path_for_task(repo_root, &task_id);
    let mut state = read_goal_state(repo_root, Some(&task_id))?
        .ok_or_else(|| format!("GOAL_STATE missing at {}", path.display()))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "GOAL_STATE root must be object".to_string())?;
    obj.insert("status".to_string(), json!(status));
    if let Some(d) = drive_until_done {
        obj.insert("drive_until_done".to_string(), json!(d));
    }
    match blocker {
        Some(b) => obj.insert("blocker".to_string(), json!(b)),
        None if status == "blocked" => None,
        None => obj.insert("blocker".to_string(), Value::Null),
    };
    obj.insert("updated_at".to_string(), json!(now_iso()));
    write_atomic_json(&path, &state)?;
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, &task_id);
    Ok(json!({
        "ok": true,
        "operation": status,
        "task_id": task_id,
        "goal_state_path": path.display().to_string(),
        "goal_state": state,
    }))
}

/// `GOAL_STATE` 是否处于「宏控制应续跑」态（`drive_until_done` + `status=running`）。
pub fn goal_state_requests_continuation(state: &Value) -> bool {
    let drive = state
        .get("drive_until_done")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = state.get("status").and_then(Value::as_str).unwrap_or("");
    drive && status == "running"
}

fn compact_goal_one_line(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut out = normalized
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn build_autopilot_drive_followup_verbose(
    repo_root: &Path,
    task_id: &str,
    goal: &str,
    horizon: &str,
) -> String {
    let path = goal_state_path_for_task(repo_root, task_id);
    let mut lines = vec![
        "AUTOPILOT_DRIVE: 目标尚未进入完成/暂停/阻塞终态，`drive_until_done` 仍为真且 `status=running`。".to_string(),
        format!("GOAL_STATE: {}", path.display()),
        format!("Goal: {}", goal),
    ];
    if !horizon.is_empty() {
        lines.push(format!("Current horizon: {}", horizon));
    }
    lines.push(
        "继续执行：推进实现与验证，更新 SESSION_SUMMARY / NEXT_ACTIONS；满足 Done when 后运行 \
         `router-rs --stdio-json` 发送 op `framework_autopilot_goal` operation=complete。"
            .to_string(),
    );
    let nudges = crate::harness_operator_nudges::resolve_harness_operator_nudges(repo_root);
    if !nudges.autopilot_drive_verbose_reasoning_depth.is_empty() {
        lines.push(nudges.autopilot_drive_verbose_reasoning_depth);
    }
    lines.join("\n")
}

/// Cursor stop / beforeSubmit：若 goal 仍在 drive 且 running，生成续跑提示（已解析的 `GOAL_STATE`）。
/// 默认极短；`ROUTER_RS_GOAL_PROMPT_VERBOSE=1` 恢复冗长版。
pub fn build_autopilot_drive_followup_message_from_state(
    repo_root: &Path,
    task_id: &str,
    state: &Value,
) -> Option<String> {
    if !autopilot_drive_hook_enabled() {
        return None;
    }
    if !goal_state_requests_continuation(state) {
        return None;
    }
    let goal = state
        .get("goal")
        .and_then(Value::as_str)
        .unwrap_or("(no goal text in GOAL_STATE)");
    let horizon = state
        .get("current_horizon")
        .and_then(Value::as_str)
        .unwrap_or("");
    if router_rs_goal_prompt_verbose() {
        return Some(build_autopilot_drive_followup_verbose(
            repo_root, task_id, goal, horizon,
        ));
    }
    let st = state.get("status").and_then(Value::as_str).unwrap_or("?");
    let rel = format!("artifacts/current/{task_id}/GOAL_STATE.json");
    let goal_short = compact_goal_one_line(goal, 140);
    let mut lines = vec![
        format!("AUTOPILOT_DRIVE: {st} · drive 未停 → 续跑（`{rel}`）。"),
        format!("Goal: {goal_short}"),
    ];
    if !horizon.is_empty() {
        lines.push(format!("Horizon: {}", compact_goal_one_line(horizon, 100)));
    }
    let nudges = crate::harness_operator_nudges::resolve_harness_operator_nudges(repo_root);
    if !nudges.autopilot_drive_compact_reasoning_depth.is_empty() {
        lines.push(nudges.autopilot_drive_compact_reasoning_depth);
    }
    lines.push("Done → `framework_autopilot_goal` operation=complete.".to_string());
    Some(lines.join("\n"))
}

/// Cursor stop / beforeSubmit：若 goal 仍在 drive 且 running，生成续跑提示。
/// 默认极短，避免每轮提交淹没用户主关注；`ROUTER_RS_GOAL_PROMPT_VERBOSE=1` 恢复冗长版。
pub fn build_autopilot_drive_followup_message(repo_root: &Path) -> Option<String> {
    let state = read_goal_state(repo_root, None).ok()??;
    let task_id = read_active_task_id(repo_root)?;
    build_autopilot_drive_followup_message_from_state(repo_root, &task_id, &state)
}

/// 将带首行前缀的段落合并进 `followup_message` 或 `additional_context`（`\n\n` 分段，与 AUTOPILOT/RFV 刷新逻辑一致）。
pub fn merge_hook_nudge_paragraph(
    output: &mut Value,
    msg: &str,
    paragraph_first_line_prefix: &str,
    use_followup_message: bool,
) {
    let field = if use_followup_message {
        "followup_message"
    } else {
        "additional_context"
    };
    match output.get_mut(field) {
        Some(Value::String(existing)) => {
            let cleaned =
                strip_followup_paragraphs_with_line_prefix(existing, paragraph_first_line_prefix);
            *existing = if cleaned.is_empty() {
                msg.to_string()
            } else {
                format!("{cleaned}\n\n{msg}")
            };
        }
        _ => {
            if let Some(obj) = output.as_object_mut() {
                obj.insert(field.to_string(), Value::String(msg.to_string()));
            }
        }
    }
}

/// 合并进 hook JSON；已有同前缀段落时先剥离再追加（默认写入 `additional_context`，见 `router_rs_cursor_hook_chat_followup_enabled`）。
/// Cursor `review-gate` 路径使用带 `CursorContinuityFrame` 的合并；本函数仅单测覆盖无 frame 的合并行为。
#[cfg(test)]
pub(crate) fn merge_autopilot_drive_followup(repo_root: &Path, output: &mut Value) {
    let Some(msg) = build_autopilot_drive_followup_message(repo_root) else {
        return;
    };
    if msg.is_empty() {
        return;
    }
    merge_hook_nudge_paragraph(
        output,
        &msg,
        "AUTOPILOT_DRIVE",
        crate::router_env_flags::router_rs_cursor_hook_chat_followup_enabled(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn goal_start_writes_and_status_reads() {
        let _nudge_env = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-autopilot-goal-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/my-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"my-task"}"#,
        )
        .expect("write pointer");

        let rr = repo.display().to_string();
        let out = framework_autopilot_goal(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "my-task",
            "goal": "ship feature X",
            "non_goals": ["rewrite unrelated modules"],
            "done_when": ["tests green"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("start");
        assert_eq!(out["ok"], json!(true));

        let st = framework_autopilot_goal(json!({
            "repo_root": rr,
            "operation": "status",
        }))
        .expect("status");
        assert!(st["goal_state"].is_object());

        let msg = build_autopilot_drive_followup_message(&repo).expect("drive msg");
        assert!(msg.contains("AUTOPILOT_DRIVE"));
        assert!(
            msg.contains("深度") && msg.contains("证据链"),
            "compact autopilot nudge from registry; msg={msg:?}"
        );

        framework_autopilot_goal(json!({
            "repo_root": rr,
            "operation": "complete",
        }))
        .expect("complete");
        assert!(build_autopilot_drive_followup_message(&repo).is_none());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_clear_removes_state_file() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-autopilot-clear-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/cl-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"cl-task"}"#,
        )
        .expect("write pointer");
        let rr = repo.display().to_string();
        framework_autopilot_goal(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "cl-task",
            "goal": "g",
            "drive_until_done": true,
        }))
        .expect("start");
        let path = goal_state_path_for_task(&repo, "cl-task");
        assert!(path.is_file());
        let out = framework_autopilot_goal(json!({
            "repo_root": rr,
            "operation": "clear",
        }))
        .expect("clear");
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["removed"], json!(true));
        assert!(!path.is_file());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn resume_does_not_force_drive_until_done_after_pause() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-autopilot-resume-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rs-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"rs-task"}"#,
        )
        .expect("write pointer");
        let rr = repo.display().to_string();
        framework_autopilot_goal(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "rs-task",
            "goal": "g",
            "drive_until_done": true,
        }))
        .expect("start");
        framework_autopilot_goal(json!({
            "repo_root": rr.clone(),
            "operation": "pause",
        }))
        .expect("pause");
        let paused = read_goal_state(&repo, None).expect("read").expect("some");
        assert_eq!(paused["drive_until_done"], json!(false));
        framework_autopilot_goal(json!({
            "repo_root": rr,
            "operation": "resume",
        }))
        .expect("resume");
        let running = read_goal_state(&repo, None).expect("read2").expect("some2");
        assert_eq!(running["status"], json!("running"));
        assert_eq!(
            running["drive_until_done"],
            json!(false),
            "resume must not flip drive_until_done back to true after pause"
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn task_evidence_summary_detects_success_row() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-evidence-sum-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/te")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"te"}"#,
        )
        .expect("active");
        fs::write(
            repo.join("artifacts/current/te/EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"command_preview":"cargo test","exit_code":0}]}"#,
        )
        .expect("evidence");
        assert_eq!(
            task_evidence_artifacts_summary_for_task(&repo, "te"),
            (true, true)
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn hydration_reads_goal_when_active_task_missing() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-hydr-fallback-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/fb-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/fb-task/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"orphan pointer","status":"running","checkpoints":[{"note":"n"}],"done_when":["d"],"validation_commands":["cargo test"]}"#,
        )
        .expect("goal");
        let got = read_goal_state_for_hydration(&repo).expect("hydr read");
        let (g, tid) = got.expect("some goal");
        assert_eq!(tid, "fb-task");
        assert_eq!(g["goal"], json!("orphan pointer"));
        assert_eq!(
            task_evidence_artifacts_summary_for_task(&repo, "fb-task"),
            (false, false)
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn hydration_reads_goal_from_focus_task_when_active_missing() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-hydr-focus-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/focus-only")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/focus_task.json"),
            r#"{"task_id":"focus-only"}"#,
        )
        .expect("focus");
        fs::write(
            repo.join("artifacts/current/focus-only/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"via focus","status":"running","checkpoints":[],"done_when":[],"validation_commands":["cargo test"]}"#,
        )
        .expect("goal");
        let got = read_goal_state_for_hydration(&repo).expect("hydr");
        let (g, tid) = got.expect("pair");
        assert_eq!(tid, "focus-only");
        assert_eq!(g["goal"], json!("via focus"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn hydration_skips_corrupt_active_goal_and_uses_other_task() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-hydr-corrupt-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/bad-task")).expect("mkdir");
        fs::create_dir_all(repo.join("artifacts/current/good-task")).expect("mkdir2");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"bad-task"}"#,
        )
        .expect("active");
        fs::write(
            repo.join("artifacts/current/bad-task/GOAL_STATE.json"),
            "{ not valid json",
        )
        .expect("bad goal");
        fs::write(
            repo.join("artifacts/current/good-task/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"recovered","status":"running","checkpoints":[{"n":1}],"done_when":["x"],"validation_commands":["cargo test"]}"#,
        )
        .expect("good goal");
        let got = read_goal_state_for_hydration(&repo).expect("hydr");
        let (g, tid) = got.expect("must find good-task");
        assert_eq!(tid, "good-task");
        assert_eq!(g["goal"], json!("recovered"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn hydration_reads_nested_path_under_current() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-hydr-nested-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/ns/sub")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/ns/sub/GOAL_STATE.json"),
            r#"{"schema_version":"router-rs-autopilot-goal-v1","goal":"nested tid","status":"running","checkpoints":[],"done_when":[],"validation_commands":[]}"#,
        )
        .expect("goal");
        let got = read_goal_state_for_hydration(&repo).expect("hydr");
        let (_g, tid) = got.expect("pair");
        assert_eq!(tid, "ns/sub");
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn merge_autopilot_drive_followup_refreshes_when_goal_text_changes() {
        let prev_chat = std::env::var("ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP").ok();
        std::env::set_var("ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP", "1");
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-autopilot-merge-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/mg-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"mg-task"}"#,
        )
        .expect("write pointer");
        let rr = repo.display().to_string();
        framework_autopilot_goal(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "mg-task",
            "goal": "alpha",
            "drive_until_done": true,
        }))
        .expect("start");
        let mut out = json!({});
        merge_autopilot_drive_followup(&repo, &mut out);
        let fm = out["followup_message"].as_str().expect("fm");
        assert!(fm.contains("alpha"));
        framework_autopilot_goal(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "mg-task",
            "goal": "beta",
            "drive_until_done": true,
        }))
        .expect("start2");
        merge_autopilot_drive_followup(&repo, &mut out);
        let fm2 = out["followup_message"].as_str().expect("fm2");
        assert!(
            fm2.contains("beta"),
            "expected refreshed goal in followup: {fm2}"
        );
        assert!(
            !fm2.contains("alpha"),
            "stale goal text should be stripped: {fm2}"
        );
        let _ = fs::remove_dir_all(&repo);
        match prev_chat {
            Some(v) => std::env::set_var("ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP", v),
            None => std::env::remove_var("ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP"),
        }
    }

    /// GOAL 与 RFV 同 task 互斥：autopilot start 应将活跃 RFV 标为 superseded。
    #[test]
    fn autopilot_start_supersedes_active_rfv_same_task() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-rfv-mutex-ag-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/mx-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"mx-task"}"#,
        )
        .expect("pointer");
        let rr = repo.display().to_string();

        crate::rfv_loop::framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "mx-task",
            "goal": "rfv phase",
            "max_rounds": 3u64,
        }))
        .expect("rfv start");
        assert!(crate::rfv_loop::build_rfv_loop_followup_message(&repo).is_some());

        let ag = framework_autopilot_goal(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "mx-task",
            "goal": "autopilot phase",
            "drive_until_done": true,
        }))
        .expect("goal start");
        assert_eq!(ag["rfv_loop_superseded"], json!(true));
        assert!(crate::rfv_loop::build_rfv_loop_followup_message(&repo).is_none());

        let rfv_path = crate::rfv_loop::rfv_loop_state_path(&repo, "mx-task");
        let raw = fs::read_to_string(&rfv_path).expect("read rfv");
        let v: Value = serde_json::from_str(&raw).expect("parse rfv");
        assert_eq!(v["loop_status"], json!("superseded"));
        assert_eq!(v["superseded_by"], json!("autopilot_goal"));

        let _ = fs::remove_dir_all(&repo);
    }
}
