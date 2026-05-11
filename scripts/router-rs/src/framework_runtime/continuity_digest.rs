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
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    let continuity = classify_runtime_continuity(&snapshot);
    let contract = supervisor_contract(&snapshot.supervisor_state);
    let task_view = crate::task_state::resolve_task_view(repo_root, None);
    let mut prompt = render_continuity_digest_prompt_base(&continuity, &contract, max_lines);
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
/// digest 主线在 `build_framework_continuity_digest_prompt` 中追加 `task_state::depth_compliance_refresh_hint`。
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

fn render_continuity_digest_prompt_base(
    continuity: &Value,
    contract: &Map<String, Value>,
    max_lines: usize,
) -> String {
    let capped_max_lines = max_lines.clamp(2, 4);
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
        lines.extend(
            next_steps
                .into_iter()
                .take(capped_max_lines)
                .map(|item| format!("- {item}")),
        );
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
        lines.push(String::new());
        lines.push("剩余：".to_string());
        lines.extend(
            remaining_tasks
                .into_iter()
                .take(capped_max_lines)
                .map(|item| format!("- {item}")),
        );
    }
    if !next_steps.is_empty() {
        lines.push(String::new());
        lines.push("先做：".to_string());
        lines.extend(
            next_steps
                .into_iter()
                .take(capped_max_lines)
                .map(|item| format!("- {item}")),
        );
    }
    if !blockers.is_empty() {
        lines.push(String::new());
        lines.push("阻塞：".to_string());
        lines.extend(
            blockers
                .into_iter()
                .take(capped_max_lines)
                .map(|item| format!("- {item}")),
        );
    }
    if can_resume && !anchors.is_empty() {
        lines.push(String::new());
        lines.push("恢复锚点：".to_string());
        lines.extend(
            anchors
                .into_iter()
                .take(capped_max_lines)
                .map(|anchor| format!("- {anchor}")),
        );
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
