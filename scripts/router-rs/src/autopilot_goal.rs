//! Autopilot 宏目标：Rust 真源 `GOAL_STATE.json` + stdio 控制面 + Cursor hook 续跑提示。
//! 不替代 LLM 执行，但把「未完成不得停」写成可校验文件态并由 hook 注入跟进。

use crate::framework_runtime::resolve_repo_root_arg;
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const GOAL_STATE_FILENAME: &str = "GOAL_STATE.json";
pub const GOAL_STATE_SCHEMA_VERSION: &str = "router-rs-autopilot-goal-v1";
const AUTOPILOT_DRIVE_HOOK_ENV: &str = "ROUTER_RS_AUTOPILOT_DRIVE_HOOK";

fn autopilot_drive_hook_enabled() -> bool {
    match env::var(AUTOPILOT_DRIVE_HOOK_ENV) {
        Ok(value) => {
            let token = value.trim().to_ascii_lowercase();
            !(token == "0" || token == "false" || token == "off" || token == "no")
        }
        Err(_) => true,
    }
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

pub fn goal_state_path_for_task(repo_root: &Path, task_id: &str) -> PathBuf {
    repo_root
        .join("artifacts/current")
        .join(task_id)
        .join(GOAL_STATE_FILENAME)
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
            Ok(json!({
                "ok": true,
                "operation": "start",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
                "goal_state": value,
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
            Ok(json!({
                "ok": true,
                "operation": "checkpoint",
                "task_id": task_id,
                "goal_state_path": path.display().to_string(),
                "goal_state": state,
            }))
        }
        "pause" => set_terminal_flags(&repo_root, task_id_override, "paused", false, None),
        "resume" => set_terminal_flags(&repo_root, task_id_override, "running", true, None),
        "complete" => set_terminal_flags(&repo_root, task_id_override, "completed", false, None),
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
                false,
                Some(blocker.to_string()),
            )
        }
        _ => Err(format!(
            "framework_autopilot_goal: unknown operation '{operation}'"
        )),
    }
}

fn set_terminal_flags(
    repo_root: &Path,
    task_id_override: Option<&str>,
    status: &str,
    drive_until_done: bool,
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
    obj.insert("drive_until_done".to_string(), json!(drive_until_done));
    match blocker {
        Some(b) => obj.insert("blocker".to_string(), json!(b)),
        None if status == "blocked" => None,
        None => obj.insert("blocker".to_string(), Value::Null),
    };
    obj.insert("updated_at".to_string(), json!(now_iso()));
    write_atomic_json(&path, &state)?;
    Ok(json!({
        "ok": true,
        "operation": status,
        "task_id": task_id,
        "goal_state_path": path.display().to_string(),
        "goal_state": state,
    }))
}

fn goal_state_requests_continuation(state: &Value) -> bool {
    let drive = state
        .get("drive_until_done")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = state.get("status").and_then(Value::as_str).unwrap_or("");
    drive && status == "running"
}

/// Cursor stop / beforeSubmit：若 goal 仍在 drive 且 running，生成续跑提示。
pub fn build_autopilot_drive_followup_message(repo_root: &Path) -> Option<String> {
    if !autopilot_drive_hook_enabled() {
        return None;
    }
    let state = read_goal_state(repo_root, None).ok()??;
    if !goal_state_requests_continuation(&state) {
        return None;
    }
    let task_id = read_active_task_id(repo_root)?;
    let path = goal_state_path_for_task(repo_root, &task_id);
    let goal = state
        .get("goal")
        .and_then(Value::as_str)
        .unwrap_or("(no goal text in GOAL_STATE)");
    let horizon = state
        .get("current_horizon")
        .and_then(Value::as_str)
        .unwrap_or("");
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
    Some(lines.join("\n"))
}

/// 合并进 hook JSON；已有 followup 时追加一段。
pub fn merge_autopilot_drive_followup(repo_root: &Path, output: &mut Value) {
    let Some(msg) = build_autopilot_drive_followup_message(repo_root) else {
        return;
    };
    if msg.is_empty() {
        return;
    }
    match output.get_mut("followup_message") {
        Some(Value::String(existing)) => {
            if existing.contains("AUTOPILOT_DRIVE") {
                return;
            }
            existing.push_str("\n\n");
            existing.push_str(&msg);
        }
        _ => {
            output["followup_message"] = Value::String(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn goal_start_writes_and_status_reads() {
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

        framework_autopilot_goal(json!({
            "repo_root": rr,
            "operation": "complete",
        }))
        .expect("complete");
        assert!(build_autopilot_drive_followup_message(&repo).is_none());
        let _ = fs::remove_dir_all(&repo);
    }
}
