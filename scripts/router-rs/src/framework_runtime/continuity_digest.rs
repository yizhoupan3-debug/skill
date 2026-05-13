//! Continuity digest text for Codex SessionStart and other hook surfaces（无独立 continuity JSON CLI）。

use serde_json::{Map, Value};
use std::path::Path;

use super::{
    classify_runtime_continuity, compact_contract_text, join_lines, load_framework_runtime_view,
    stable_line_items, supervisor_contract, value_string_list, value_text,
};

/// 生成连续性摘要正文（含 depth 提示与可选 GOAL 段落）；不含剪贴板或 envelope。
pub fn build_framework_continuity_digest_prompt(
    repo_root: &Path,
    max_lines: usize,
) -> Result<String, String> {
    build_framework_continuity_digest_prompt_impl(repo_root, max_lines, false)
}

/// Like [`build_framework_continuity_digest_prompt`], but can omit the active/focus GOAL mismatch
/// ZH line so a caller (e.g. Cursor SessionStart) may prepend it for prefix-truncation survival.
pub fn build_framework_continuity_digest_prompt_ex(
    repo_root: &Path,
    max_lines: usize,
    omit_active_focus_mismatch_line: bool,
) -> Result<String, String> {
    build_framework_continuity_digest_prompt_impl(
        repo_root,
        max_lines,
        omit_active_focus_mismatch_line,
    )
}

fn build_framework_continuity_digest_prompt_impl(
    repo_root: &Path,
    max_lines: usize,
    omit_active_focus_mismatch_line: bool,
) -> Result<String, String> {
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    let continuity = classify_runtime_continuity(&snapshot);
    let contract = supervisor_contract(&snapshot.supervisor_state);
    let task_view = crate::task_state::resolve_task_view(repo_root, None);
    let mut prompt = render_continuity_digest_prompt_base(&continuity, &contract, max_lines);
    if !omit_active_focus_mismatch_line
        && crate::task_state::task_view_has_active_goal_focus_mismatch_note(&task_view)
    {
        prompt.push_str("\n\n");
        prompt.push_str(crate::task_state::CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH);
    }
    if let Some(hint) = crate::task_state::depth_compliance_refresh_hint(&task_view) {
        prompt.push_str("\n\n");
        prompt.push_str(&hint);
    }
    let goal_state = crate::autopilot_goal::read_goal_state_for_hydration(repo_root)
        .ok()
        .flatten()
        .map(|(value, _task_id)| value);
    if let Some(ref g) = goal_state {
        prompt.push_str("\n\n");
        prompt.push_str(&format_goal_state_digest_section(repo_root, g));
    }
    Ok(prompt)
}

/// 把 `GOAL_STATE.json` 嵌进 digest，使 SessionStart 等可见「可执行目标」而非仅有连续性摘要。
/// GOAL 段落统一使用紧凑模板。
///
/// GOAL 段落只保留状态、验收和证据锚点；长 nudge 留在文档/schema。
/// digest 主线在 `build_framework_continuity_digest_prompt` 中追加 `task_state::depth_compliance_refresh_hint`；
/// 当 [`crate::task_state::task_view_has_active_goal_focus_mismatch_note`] 为真时再追加单行连续性运维提示（文案见 [`crate::task_state::CONTINUITY_ACTIVE_FOCUS_GOAL_MISMATCH_HINT_ZH`]）。
fn format_goal_state_digest_section(repo_root: &Path, goal: &Value) -> String {
    format_goal_state_digest_section_compact(repo_root, goal)
}

fn format_goal_state_digest_section_compact(_repo_root: &Path, goal: &Value) -> String {
    let g = value_text(goal.get("goal"));
    let st = value_text(goal.get("status"));
    let drive = goal
        .get("drive_until_done")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let done = value_string_list(goal.get("done_when"));
    let val = value_string_list(goal.get("validation_commands"));
    let horizon = value_text(goal.get("current_horizon"));
    let goal_line = if g.is_empty() {
        "（未填写）".to_string()
    } else {
        compact_contract_text(&g, 200)
    };
    let mut lines: Vec<String> = Vec::new();
    lines.push("## Active goal（全文见磁盘 `GOAL_STATE.json`）".to_string());
    lines.push(format!(
        "- {} · drive={} · {}",
        if st.is_empty() {
            "（状态未填）".to_string()
        } else {
            st
        },
        drive,
        goal_line
    ));
    let mut bits: Vec<String> = Vec::new();
    if !horizon.is_empty() {
        bits.push(format!("horizon: {}", compact_contract_text(&horizon, 100)));
    }
    if !done.is_empty() {
        bits.push(format!(
            "done: {}",
            compact_contract_text(&done.join(" · "), 160)
        ));
    }
    if !val.is_empty() {
        let head = val.first().map(|s| s.as_str()).unwrap_or("").to_string();
        let extra = val.len().saturating_sub(1);
        let cmd = if extra > 0 {
            format!("`{}` (+{extra})", compact_contract_text(&head, 72))
        } else {
            format!("`{}`", compact_contract_text(&head, 90))
        };
        bits.push(format!("verify: {cmd}"));
    }
    if !bits.is_empty() {
        lines.push(format!("- {}", bits.join(" · ")));
    }
    lines.push(
        "- 收口: `framework_autopilot_goal` operation=complete（或 pause/block）。".to_string(),
    );
    lines.push("- 证据见 `EVIDENCE_INDEX.json` / `RFV_LOOP_STATE.json`。".to_string());
    lines.join("\n")
}

fn append_capped_bullet_section(
    lines: &mut Vec<String>,
    section_title: &str,
    items: Vec<String>,
    cap: usize,
) {
    if items.is_empty() {
        return;
    }
    let total = items.len();
    let take_n = cap.min(total);
    lines.push(String::new());
    lines.push(section_title.to_string());
    lines.extend(
        items
            .into_iter()
            .take(take_n)
            .map(|item| format!("- {item}")),
    );
    if total > take_n {
        lines.push(format!(
            "- …（共 {total} 条，此处展示前 {take_n} 条；完整列表见 `router-rs framework snapshot`）"
        ));
    }
}

fn render_continuity_digest_prompt_base(
    continuity: &Value,
    contract: &Map<String, Value>,
    max_lines: usize,
) -> String {
    // Per-section bullet list cap; at least 2. Callers may pass values above 4 (tests use 8).
    let list_cap = max_lines.max(2);
    let state = value_text(continuity.get("state"));
    let task = value_text(continuity.get("task"));
    let phase = value_text(continuity.get("phase"));
    let status = {
        let raw = value_text(continuity.get("status"));
        if raw.is_empty() {
            state.clone()
        } else {
            raw
        }
    };
    let can_resume = continuity
        .get("can_resume")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let route = value_string_list(continuity.get("route"));
    let paths_map = continuity
        .get("paths")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let current = continuity
        .get("current_execution")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let completed = continuity
        .get("recent_completed_execution")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let recovery_hints = value_string_list(continuity.get("recovery_hints"));
    let continuity_next_actions = value_string_list(continuity.get("next_actions"));
    let continuity_blockers = value_string_list(continuity.get("blockers"));
    let verification_status = value_text(continuity.get("verification_status"));
    let effect_line = if state == "completed" {
        if verification_status == "completed" {
            "结果已经稳定，可以直接按已完成上下文来看。".to_string()
        } else {
            "这一轮已经收住，不用再把它当当前任务。".to_string()
        }
    } else {
        String::new()
    };
    let remaining_tasks = if state == "active" && !current.is_empty() {
        stable_line_items(
            contract
                .get("scope")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .chain(
                    contract
                        .get("acceptance_criteria")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten(),
                )
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect(),
        )
    } else if state == "completed" && !completed.is_empty() {
        stable_line_items(vec!["最近一轮已经收尾".to_string()])
    } else if state == "inconsistent" {
        value_string_list(continuity.get("inconsistency_reasons"))
    } else {
        recovery_hints.clone()
    };
    let next_steps = if state == "active" && !current.is_empty() {
        let mut items = vec!["先核对恢复锚点和当前代码".to_string()];
        items.extend(continuity_next_actions.clone());
        stable_line_items(items)
    } else if state == "completed" && !completed.is_empty() {
        stable_line_items(vec![
            "如果还要继续相关工作，先新开一个 standalone task".to_string()
        ])
    } else if state == "stale" {
        let mut items = vec!["先重读锚点并重建上下文".to_string()];
        if continuity_next_actions.is_empty() {
            items.extend(recovery_hints.clone());
        } else {
            items.extend(continuity_next_actions.clone());
        }
        stable_line_items(items)
    } else if state == "inconsistent" {
        let mut items = vec!["先对齐摘要、轨迹和 supervisor".to_string()];
        items.extend(recovery_hints.clone());
        stable_line_items(items)
    } else {
        let mut items = vec!["先补齐缺失锚点并确认状态".to_string()];
        if continuity_next_actions.is_empty() {
            items.extend(recovery_hints.clone());
        } else {
            items.extend(continuity_next_actions.clone());
        }
        stable_line_items(items)
    };
    let blockers = if state == "completed" {
        Vec::new()
    } else {
        continuity_blockers.clone()
    };
    let anchors = stable_line_items(vec![
        value_text(paths_map.get("session_summary"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "SESSION_SUMMARY: {}",
                    compact_artifact_anchor(&value_text(paths_map.get("session_summary")))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("next_actions"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "NEXT_ACTIONS: {}",
                    compact_artifact_anchor(&value_text(paths_map.get("next_actions")))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("trace_metadata"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "TRACE_METADATA: {}",
                    compact_artifact_anchor(&value_text(paths_map.get("trace_metadata")))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("supervisor_state"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "SUPERVISOR_STATE: {}",
                    compact_artifact_anchor(&value_text(paths_map.get("supervisor_state")))
                )
            })
            .unwrap_or_default(),
    ]);

    if state == "completed" && !completed.is_empty() {
        let mut lines = vec!["最近一轮已经收尾：".to_string()];
        lines.push(format!(
            "- {}",
            if task.is_empty() {
                "上一轮任务已完成"
            } else {
                &task
            }
        ));
        if !effect_line.is_empty() {
            lines.push(format!("- {effect_line}"));
        }
        let total_ns = next_steps.len();
        let take_ns = list_cap.min(total_ns);
        lines.extend(
            next_steps
                .into_iter()
                .take(take_ns)
                .map(|item| format!("- {item}")),
        );
        if total_ns > take_ns {
            lines.push(format!(
                "- …（共 {total_ns} 条，此处展示前 {take_ns} 条；完整列表见 `router-rs framework snapshot`）"
            ));
        }
        return lines.join("\n") + "\n";
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "任务：{}",
        if task.is_empty() { "未记录" } else { &task }
    ));
    lines.push(format!(
        "状态：{}",
        join_lines(&stable_line_items(vec![
            if phase.is_empty() {
                String::new()
            } else {
                phase.clone()
            },
            if status.is_empty() {
                if state.is_empty() {
                    "missing".to_string()
                } else {
                    state.clone()
                }
            } else {
                status.clone()
            },
            if state.is_empty() {
                String::new()
            } else {
                state.clone()
            },
        ]))
    ));
    if !route.is_empty() {
        lines.push(format!("路由：{}", join_lines(&route)));
    }
    if !remaining_tasks.is_empty() {
        append_capped_bullet_section(&mut lines, "剩余：", remaining_tasks, list_cap);
    }
    if !next_steps.is_empty() {
        append_capped_bullet_section(&mut lines, "先做：", next_steps, list_cap);
    }
    if !blockers.is_empty() {
        append_capped_bullet_section(&mut lines, "阻塞：", blockers, list_cap);
    }
    if can_resume && !anchors.is_empty() {
        append_capped_bullet_section(&mut lines, "恢复锚点：", anchors, list_cap);
    }
    lines.join("\n") + "\n"
}

fn compact_artifact_anchor(path: &str) -> String {
    for marker in ["artifacts/current/", ".supervisor_state.json"] {
        if marker == ".supervisor_state.json" && path.ends_with(marker) {
            return marker.to_string();
        }
        if let Some(idx) = path.find(marker) {
            return path[idx..].to_string();
        }
    }
    path.to_string()
}

#[cfg(test)]
mod digest_depth_self_check_tests {
    use super::format_goal_state_digest_section;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn compact_goal_section_keeps_single_line_self_check() {
        let joined = format_goal_state_digest_section(
            Path::new("."),
            &json!({
                "goal": "ship",
                "status": "running",
                "drive_until_done": true,
                "done_when": ["tests green"],
                "validation_commands": ["cargo test -q"]
            }),
        );
        assert!(
            !joined.contains("深度自检"),
            "long self-check removed; got {joined:?}"
        );
        assert!(
            joined.contains("EVIDENCE") || joined.contains("RFV"),
            "expected evidence anchor; got {joined:?}"
        );
        assert!(
            !joined.contains("1)"),
            "compact template should not emit numbered verbose checklist; got {joined:?}"
        );
    }
}

#[cfg(test)]
mod digest_active_focus_hint_tests {
    use super::build_framework_continuity_digest_prompt;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_repo(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("router-rs-digest-hint-{label}-{nonce}"))
    }

    #[test]
    fn digest_appends_focus_goal_hint_when_task_view_notes_match() {
        let tmp = unique_repo("pos");
        let active_tid = "t-empty";
        let focus_tid = "t-filled";
        let cur = tmp.join("artifacts/current");
        fs::create_dir_all(cur.join(active_tid)).unwrap();
        fs::write(
            cur.join("active_task.json"),
            format!(r#"{{"task_id":"{active_tid}"}}"#),
        )
        .unwrap();
        fs::write(
            cur.join("focus_task.json"),
            format!(r#"{{"task_id":"{focus_tid}"}}"#),
        )
        .unwrap();
        let focus_dir = cur.join(focus_tid);
        fs::create_dir_all(&focus_dir).unwrap();
        fs::write(
            focus_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-autopilot-goal-v1",
                "drive_until_done": true,
                "status": "running",
                "goal": "g",
                "non_goals": [],
                "done_when": [],
                "validation_commands": [],
                "current_horizon": "",
                "checkpoints": [],
                "blocker": null,
                "updated_at": "2026-01-01T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();

        let prompt = build_framework_continuity_digest_prompt(&tmp, 8).expect("digest");
        assert!(
            prompt.contains("连续性提示:"),
            "expected zh continuity hint line in prompt:\n{prompt}"
        );
        assert!(
            prompt.contains("hydration"),
            "expected hydration wording:\n{prompt}"
        );
        assert!(
            !prompt.contains("## Active goal"),
            "must not present focus GOAL as Active goal block:\n{prompt}"
        );
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn digest_omits_focus_goal_hint_when_active_has_goal() {
        let tmp = unique_repo("neg");
        let tid = "t1";
        let cur = tmp.join("artifacts/current");
        let task_dir = cur.join(tid);
        fs::create_dir_all(&task_dir).unwrap();
        fs::write(
            cur.join("active_task.json"),
            format!(r#"{{"task_id":"{tid}"}}"#),
        )
        .unwrap();
        fs::write(
            cur.join("focus_task.json"),
            format!(r#"{{"task_id":"other"}}"#),
        )
        .unwrap();
        fs::write(
            task_dir.join("GOAL_STATE.json"),
            serde_json::to_string_pretty(&json!({
                "schema_version": "router-rs-autopilot-goal-v1",
                "drive_until_done": true,
                "status": "running",
                "goal": "ok",
                "non_goals": [],
                "done_when": [],
                "validation_commands": [],
                "current_horizon": "",
                "checkpoints": [],
                "blocker": null,
                "updated_at": "2026-01-01T00:00:00Z"
            }))
            .unwrap(),
        )
        .unwrap();

        let prompt = build_framework_continuity_digest_prompt(&tmp, 8).expect("digest");
        assert!(
            !prompt.contains("连续性提示:"),
            "unexpected zh continuity hint when active GOAL ok:\n{prompt}"
        );
        let _ = fs::remove_dir_all(&tmp);
    }
}
