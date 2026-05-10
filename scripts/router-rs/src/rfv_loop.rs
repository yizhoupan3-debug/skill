//! Review-Fix-Verify 多轮闭环：Rust 真源 `RFV_LOOP_STATE.json` + stdio，支撑长任务轮次账本与宿主并行 lane 之后的 supervisor 合并落盘。

use crate::autopilot_goal::read_active_task_id;
use crate::framework_runtime::resolve_repo_root_arg;
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const RFV_LOOP_STATE_FILENAME: &str = "RFV_LOOP_STATE.json";
pub const RFV_LOOP_SCHEMA_VERSION: &str = "router-rs-rfv-loop-v1";
const MAX_ROUNDS_HARD_CAP: u64 = 1000;
/// Cursor hook：`RFV_LOOP_CONTINUE` 跟进；设为 `0`/`false`/`off`/`no` 关闭。
const RFV_LOOP_HOOK_ENV: &str = "ROUTER_RS_RFV_LOOP_HOOK";

fn rfv_loop_hook_enabled() -> bool {
    match env::var(RFV_LOOP_HOOK_ENV) {
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
        .map_err(|err| format!("serialize RFV_LOOP_STATE failed: {err}"))?;
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

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn rfv_loop_state_path(repo_root: &Path, task_id: &str) -> PathBuf {
    repo_root
        .join("artifacts/current")
        .join(task_id)
        .join(RFV_LOOP_STATE_FILENAME)
}

/// 供 Cursor hook / 工具读取当前任务的 RFV 账本（无覆盖则用 `active_task.json`）。
pub fn read_rfv_loop_state(
    repo_root: &Path,
    task_id_override: Option<&str>,
) -> Result<Option<Value>, String> {
    let task_id = if let Some(t) = task_id_override {
        if t.trim().is_empty() {
            return Err("framework_rfv_loop: task_id override is empty".to_string());
        }
        t.trim().to_string()
    } else {
        let Some(t) = read_active_task_id(repo_root) else {
            return Ok(None);
        };
        t
    };
    let path = rfv_loop_state_path(repo_root, &task_id);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("read RFV_LOOP_STATE: {err}"))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|err| format!("parse RFV_LOOP_STATE: {err}"))?;
    Ok(Some(value))
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

fn clamp_max_rounds(raw: u64) -> (u64, bool) {
    if raw > MAX_ROUNDS_HARD_CAP {
        (MAX_ROUNDS_HARD_CAP, true)
    } else {
        (raw, false)
    }
}

/// stdio：`framework_rfv_loop`
pub fn framework_rfv_loop(payload: Value) -> Result<Value, String> {
    let repo_root = payload
        .get("repo_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "framework_rfv_loop requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(format!(
            "framework_rfv_loop: repo_root is not a directory: {}",
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
            let state = read_rfv_loop_state(&repo_root, task_id_override)?;
            let tid = if let Some(t) = task_id_override {
                t.to_string()
            } else {
                read_active_task_id(&repo_root).unwrap_or_default()
            };
            let path = if tid.is_empty() {
                PathBuf::new()
            } else {
                rfv_loop_state_path(&repo_root, &tid)
            };
            Ok(json!({
                "ok": true,
                "operation": "status",
                "task_id": tid,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": state,
            }))
        }
        "start" | "upsert" => {
            let task_id = task_id_override
                .map(|s| s.to_string())
                .or_else(|| read_active_task_id(&repo_root))
                .ok_or_else(|| {
                    "framework_rfv_loop start requires task_id in payload or active_task.json"
                        .to_string()
                })?;
            let goal = payload
                .get("goal")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "framework_rfv_loop start requires non-empty goal".to_string())?;
            let requested_max = payload
                .get("max_rounds")
                .and_then(Value::as_u64)
                .unwrap_or(3);
            let (max_rounds, capped) = clamp_max_rounds(requested_max);
            let allow_external = payload
                .get("allow_external_research")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let parallel_external = payload
                .get("parallel_external_with_review")
                .and_then(Value::as_bool)
                .unwrap_or(true);

            let mut obj = Map::new();
            obj.insert("schema_version".to_string(), json!(RFV_LOOP_SCHEMA_VERSION));
            obj.insert("goal".to_string(), json!(goal));
            obj.insert("max_rounds".to_string(), json!(max_rounds));
            obj.insert("max_rounds_requested".to_string(), json!(requested_max));
            obj.insert("max_rounds_capped".to_string(), json!(capped));
            obj.insert("allow_external_research".to_string(), json!(allow_external));
            obj.insert(
                "parallel_external_with_review".to_string(),
                json!(parallel_external),
            );
            obj.insert(
                "review_scope".to_string(),
                json!(payload
                    .get("review_scope")
                    .and_then(Value::as_str)
                    .unwrap_or("")),
            );
            obj.insert(
                "fix_scope".to_string(),
                json!(payload
                    .get("fix_scope")
                    .and_then(Value::as_str)
                    .unwrap_or("")),
            );
            obj.insert(
                "verify_commands".to_string(),
                Value::Array(value_string_list(&payload, "verify_commands")),
            );
            obj.insert(
                "stop_when".to_string(),
                Value::Array(value_string_list(&payload, "stop_when")),
            );
            obj.insert("loop_status".to_string(), json!("active"));
            obj.insert("current_round".to_string(), json!(0));
            obj.insert("rounds".to_string(), json!([]));
            obj.insert("updated_at".to_string(), json!(now_iso()));
            if let Some(extra) = payload.get("metadata").cloned() {
                obj.insert("metadata".to_string(), extra);
            }

            let path = rfv_loop_state_path(&repo_root, &task_id);
            let value = Value::Object(obj);
            write_atomic_json(&path, &value)?;
            Ok(json!({
                "ok": true,
                "operation": "start",
                "task_id": task_id,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": value,
                "warning": if capped {
                    Some(format!(
                        "max_rounds requested {requested_max} exceeds hard cap {MAX_ROUNDS_HARD_CAP}; stored max_rounds={max_rounds}"
                    ))
                } else {
                    None
                },
            }))
        }
        "append_round" => {
            let task_id = task_id_override
                .map(|s| s.to_string())
                .or_else(|| read_active_task_id(&repo_root))
                .ok_or_else(|| {
                    "framework_rfv_loop append_round requires task_id or active_task.json"
                        .to_string()
                })?;
            let path = rfv_loop_state_path(&repo_root, &task_id);
            let mut state = read_rfv_loop_state(&repo_root, Some(&task_id))?
                .ok_or_else(|| format!("RFV_LOOP_STATE missing at {}", path.display()))?;

            let round_n = payload
                .get("round")
                .and_then(Value::as_u64)
                .ok_or_else(|| "append_round requires round (u64)".to_string())?;

            let obj = state
                .as_object_mut()
                .ok_or_else(|| "RFV_LOOP_STATE root must be object".to_string())?;
            let max_rounds = obj
                .get("max_rounds")
                .and_then(Value::as_u64)
                .unwrap_or(MAX_ROUNDS_HARD_CAP);
            if round_n > max_rounds {
                return Err(format!("round {round_n} exceeds max_rounds {max_rounds}"));
            }

            let review_summary = payload
                .get("review_summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let external_research_summary = payload
                .get("external_research_summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let fix_summary = payload
                .get("fix_summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let verify_result = payload
                .get("verify_result")
                .and_then(Value::as_str)
                .unwrap_or("UNKNOWN")
                .to_string();
            let supervisor_decision = payload
                .get("supervisor_decision")
                .and_then(Value::as_str)
                .unwrap_or("continue")
                .to_ascii_lowercase();
            let reason = payload
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            let entry = json!({
                "round": round_n,
                "review_summary": review_summary,
                "external_research_summary": external_research_summary,
                "fix_summary": fix_summary,
                "verify_result": verify_result,
                "supervisor_decision": supervisor_decision,
                "reason": reason,
                "at": now_iso(),
            });

            let rounds = obj
                .get_mut("rounds")
                .and_then(|r| r.as_array_mut())
                .ok_or_else(|| "RFV_LOOP_STATE.rounds missing".to_string())?;
            rounds.push(entry);

            obj.insert("current_round".to_string(), json!(round_n));
            obj.insert("updated_at".to_string(), json!(now_iso()));

            let loop_status = match supervisor_decision.as_str() {
                "close" | "closed" => "closed",
                "block" | "blocked" => "blocked",
                _ => {
                    if round_n >= max_rounds {
                        "closed"
                    } else {
                        "active"
                    }
                }
            };
            obj.insert("loop_status".to_string(), json!(loop_status));

            write_atomic_json(&path, &state)?;
            Ok(json!({
                "ok": true,
                "operation": "append_round",
                "task_id": task_id,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": state,
            }))
        }
        _ => Err(format!(
            "framework_rfv_loop: unknown operation '{operation}'"
        )),
    }
}

fn rfv_loop_requests_continuation(state: &Value) -> bool {
    state
        .get("loop_status")
        .and_then(Value::as_str)
        .map(|s| s.eq_ignore_ascii_case("active"))
        .unwrap_or(false)
}

/// Cursor stop / beforeSubmit：`loop_status=active` 时提示继续下一轮 RFV。
pub fn build_rfv_loop_followup_message(repo_root: &Path) -> Option<String> {
    if !rfv_loop_hook_enabled() {
        return None;
    }
    let state = read_rfv_loop_state(repo_root, None).ok()??;
    if !rfv_loop_requests_continuation(&state) {
        return None;
    }
    let task_id = read_active_task_id(repo_root)?;
    let path = rfv_loop_state_path(repo_root, &task_id);
    let goal = state
        .get("goal")
        .and_then(Value::as_str)
        .unwrap_or("(no goal in RFV_LOOP_STATE)");
    let current = state
        .get("current_round")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let max_r = state.get("max_rounds").and_then(Value::as_u64).unwrap_or(0);
    let ext = state
        .get("allow_external_research")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut lines = vec![
        "RFV_LOOP_CONTINUE: `RFV_LOOP_STATE.json` 显示多轮 review-fix-verify 仍在进行（`loop_status=active`）。".to_string(),
        format!("Path: {}", path.display()),
        format!("Goal: {}", goal),
        format!("Progress: round {current} / max_rounds {max_r} (达到 stop_when 或 append_round close 后可结束)"),
    ];
    if ext {
        lines.push(
            "本任务允许外部调研：下一轮可并行 external research + internal review，再进入 fix / verify；每轮结束请 `framework_rfv_loop` append_round。"
                .to_string(),
        );
    } else {
        lines.push(
            "下一轮请按 review → fix → verify 顺序起独立 subagent，并在每轮末 `append_round` 落盘。"
                .to_string(),
        );
    }
    Some(lines.join("\n"))
}

pub fn merge_rfv_loop_followup(repo_root: &Path, output: &mut Value) {
    let Some(msg) = build_rfv_loop_followup_message(repo_root) else {
        return;
    };
    if msg.is_empty() {
        return;
    }
    match output.get_mut("followup_message") {
        Some(Value::String(existing)) => {
            if existing.contains("RFV_LOOP_CONTINUE") {
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

/// preCompact 用的一行摘要（不分配大段 followup）。
pub fn rfv_loop_precompact_hint(repo_root: &Path) -> Option<String> {
    let state = read_rfv_loop_state(repo_root, None).ok()??;
    if !rfv_loop_requests_continuation(&state) {
        return None;
    }
    let current = state
        .get("current_round")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let max_r = state.get("max_rounds").and_then(Value::as_u64).unwrap_or(0);
    Some(format!(
        "RFV_LOOP active: round {current}/{max_r}; see artifacts/current/<task>/RFV_LOOP_STATE.json"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rfv_start_append_roundtrip() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rfv-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"rfv-task"}"#,
        )
        .expect("pointer");

        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "rfv-task",
            "goal": "harden loop",
            "max_rounds": 100,
            "allow_external_research": true,
            "verify_commands": ["cargo test -q"],
            "stop_when": ["verifier pass", "max_rounds"],
        }))
        .expect("start");

        framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "append_round",
            "round": 1u64,
            "review_summary": "r1",
            "external_research_summary": "web: none",
            "fix_summary": "f1",
            "verify_result": "PASS",
            "supervisor_decision": "continue",
            "reason": "ok",
        }))
        .expect("append");

        let st = framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "status",
        }))
        .expect("status");
        let gs = st["rfv_loop_state"].as_object().expect("obj");
        assert_eq!(gs["current_round"], json!(1));
        assert_eq!(gs["loop_status"], json!("active"));
        let _via_api = read_rfv_loop_state(&repo, None)
            .expect("read api")
            .expect("state");

        let msg = build_rfv_loop_followup_message(&repo).expect("rfv followup");
        assert!(msg.contains("RFV_LOOP_CONTINUE"));

        let _ = fs::remove_dir_all(&repo);
    }
}
