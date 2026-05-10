//! Continuity digest text for Codex SessionStart and other hook surfaces（无独立 continuity JSON CLI）。

use serde_json::{Map, Value};
use std::path::Path;

use super::{
    classify_runtime_continuity, compact_contract_text, join_lines, load_framework_runtime_view,
    stable_line_items, supervisor_contract, value_string_list, value_text,
};
use crate::router_env_flags::router_rs_goal_prompt_verbose;

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
    let goal_state = crate::autopilot_goal::read_goal_state(repo_root, None)
        .ok()
        .flatten();
    if let Some(ref g) = goal_state {
        prompt.push_str("\n\n");
        prompt.push_str(&format_goal_state_digest_section(repo_root, g));
    }
    Ok(prompt)
}

/// 把 `GOAL_STATE.json` 嵌进 digest，使 SessionStart 等可见「可执行目标」而非仅有连续性摘要。
/// 默认紧凑；`ROUTER_RS_GOAL_PROMPT_VERBOSE=1` 使用冗长 checklist。
///
/// GOAL 段落走 `HARNESS_OPERATOR_NUDGES.json` 真源（与 RFV/AUTOPILOT 续跑共用），含可选 **`math_reasoning_harness_line`**，并附带「深度自检」行。
/// `ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` 仅去掉 JSON 配置的 operator 文案；**深度自检行仍在**。
/// 另：digest 主线在 `build_framework_continuity_digest_prompt` 中追加 `task_state::depth_compliance_refresh_hint`。
fn format_goal_state_digest_section(repo_root: &Path, goal: &Value) -> String {
    if router_rs_goal_prompt_verbose() {
        format_goal_state_digest_section_verbose(repo_root, goal)
    } else {
        format_goal_state_digest_section_compact(repo_root, goal)
    }
}

/// Verbose 版本（仅 GOAL_PROMPT_VERBOSE=1 时启用）：使用 RFV reasoning-depth contract 的完整三问。
/// Compact 版本：单行；保留 SessionStart 640 字符上限友好。
fn digest_depth_self_check_lines(verbose: bool) -> Vec<String> {
    if verbose {
        vec![
            "- 深度自检（reasoning-depth-contract）：".to_string(),
            "  1) 是否先定分工与汇合点（谁查证、谁实现、谁验证；写入范围不打架）？".to_string(),
            "  2) verify 是否有明确命令/证据（PASS/FAIL 有日志/exit_code，而不是“感觉”）？"
                .to_string(),
            "  3) 本轮是否落证据（EVIDENCE_INDEX 或 RFV append_round）便于回放？".to_string(),
        ]
    } else {
        vec![
            "- 深度自检：先定分工/汇合点 → 并行查证 → 收敛改动 → 跑验证并留证据（EVIDENCE/RFV）。"
                .to_string(),
        ]
    }
}

fn format_goal_state_digest_section_verbose(repo_root: &Path, goal: &Value) -> String {
    let g = value_text(goal.get("goal"));
    let st = value_text(goal.get("status"));
    let drive = goal
        .get("drive_until_done")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let done = value_string_list(goal.get("done_when"));
    let val = value_string_list(goal.get("validation_commands"));
    let non = value_string_list(goal.get("non_goals"));
    let horizon = value_text(goal.get("current_horizon"));
    let mut lines: Vec<String> = Vec::new();
    lines.push(
        "## GOAL_STATE（router-rs 目标机；须据此推进直至 complete 或显式 pause/block）".to_string(),
    );
    lines.push(format!(
        "- 目标: {}",
        if g.is_empty() {
            "（未填写）".to_string()
        } else {
            g
        }
    ));
    lines.push(format!("- 状态: {} | drive_until_done: {}", st, drive));
    if !horizon.is_empty() {
        lines.push(format!("- 当前地平线: {}", horizon));
    }
    if !non.is_empty() {
        lines.push(format!("- 非目标: {}", non.join("；")));
    }
    if !done.is_empty() {
        lines.push(format!("- 验收 done_when: {}", done.join("；")));
    }
    if !val.is_empty() {
        lines.push(format!("- 验证命令: {}", val.join("；")));
    }
    lines.push(
        "- 下一跳: 实现 → 跑验证命令 → 更新 SESSION_SUMMARY/NEXT_ACTIONS；满足验收后 `stdio` op `framework_autopilot_goal` operation=complete。"
            .to_string(),
    );
    let nudges = crate::harness_operator_nudges::resolve_harness_operator_nudges(repo_root);
    if !nudges.autopilot_drive_verbose_reasoning_depth.is_empty() {
        lines.push(format!(
            "- {}",
            nudges.autopilot_drive_verbose_reasoning_depth
        ));
    }
    if !nudges.math_reasoning_harness_line.trim().is_empty() {
        lines.push(format!("- {}", nudges.math_reasoning_harness_line.trim()));
    }
    lines.extend(digest_depth_self_check_lines(true));
    lines.join("\n")
}

fn format_goal_state_digest_section_compact(repo_root: &Path, goal: &Value) -> String {
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
    let nudges = crate::harness_operator_nudges::resolve_harness_operator_nudges(repo_root);
    if !nudges.autopilot_drive_compact_reasoning_depth.is_empty() {
        lines.push(format!(
            "- {}",
            nudges.autopilot_drive_compact_reasoning_depth
        ));
    }
    if !nudges.math_reasoning_harness_line.trim().is_empty() {
        lines.push(format!("- {}", nudges.math_reasoning_harness_line.trim()));
    }
    lines.extend(digest_depth_self_check_lines(false));
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
                    value_text(paths_map.get("session_summary"))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("next_actions"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "NEXT_ACTIONS: {}",
                    value_text(paths_map.get("next_actions"))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("trace_metadata"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "TRACE_METADATA: {}",
                    value_text(paths_map.get("trace_metadata"))
                )
            })
            .unwrap_or_default(),
        value_text(paths_map.get("supervisor_state"))
            .chars()
            .next()
            .map(|_| {
                format!(
                    "SUPERVISOR_STATE: {}",
                    value_text(paths_map.get("supervisor_state"))
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
        lines.push(String::new());
        lines.push("先看这些恢复锚点：".to_string());
        lines.extend(
            anchors
                .into_iter()
                .take(capped_max_lines)
                .map(|anchor| format!("- {anchor}")),
        );
        return lines.join("\n") + "\n";
    }

    let mut lines = vec!["继续当前仓库，先看这些恢复锚点：".to_string()];
    lines.extend(
        anchors
            .into_iter()
            .take(capped_max_lines)
            .map(|anchor| format!("- {anchor}")),
    );
    lines.push(String::new());
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
    lines.push(String::new());
    lines.push("按既定串并行分工直接开始执行。".to_string());
    lines.join("\n") + "\n"
}
