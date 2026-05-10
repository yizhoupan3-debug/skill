use serde_json::Value;
use std::path::Path;
use std::process::Command;

use super::{
    classify_runtime_continuity, load_framework_runtime_view, value_string_list, value_text,
};

pub fn build_framework_statusline(repo_root: &Path) -> Result<String, String> {
    let snapshot = load_framework_runtime_view(repo_root, None, None);
    let continuity = classify_runtime_continuity(&snapshot);
    let task_view = crate::task_state::resolve_task_view(repo_root, None);
    let depth_status = task_view
        .depth_compliance
        .as_ref()
        .and_then(|dc| {
            let tid_ok = task_view
                .task_id
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            tid_ok.then_some(dc)
        })
        .map(|dc| {
            let bang = if dc.rfv_pass_without_evidence_count > 0 {
                "!"
            } else {
                ""
            };
            format!("depth=d{}{} | ", dc.depth_score, bang)
        })
        .unwrap_or_default();
    let supervisor_state = &snapshot.supervisor_state;
    let task = first_nonempty_text(&[
        continuity.get("task"),
        supervisor_state.get("task_summary"),
        Some(&Value::String("none".to_string())),
    ]);
    let phase = first_nonempty_text(&[
        continuity.get("phase"),
        supervisor_state.get("active_phase"),
        Some(&Value::String("idle".to_string())),
    ]);
    let status = first_nonempty_text(&[
        continuity.get("status"),
        Some(&Value::String("unknown".to_string())),
    ]);
    let route = statusline_route(&continuity);
    let (git_state, branch) = git_statusline_state(repo_root);
    let blockers = value_string_list(continuity.get("blockers"));
    let next_actions = value_string_list(continuity.get("next_actions"));
    let focus_task_id = snapshot.focus_task_id.clone().unwrap_or_default();
    let other_known_count = snapshot
        .known_task_ids
        .iter()
        .filter(|task_id| !task_id.is_empty() && **task_id != focus_task_id)
        .count();
    let other_recoverable_count = snapshot
        .recoverable_task_ids
        .iter()
        .filter(|task_id| !task_id.is_empty() && **task_id != focus_task_id)
        .count();
    Ok(format!(
        "{} | {} | {}/{} | task={} | route={} | nexts={} | blockers={} | others={} | resumable={} | {}git={}",
        branch,
        statusline_decision_hint(&blockers, &next_actions, &git_state, &status),
        phase,
        status,
        short_statusline_text(&task, 24),
        route,
        next_actions.len(),
        blockers.len(),
        other_known_count,
        other_recoverable_count,
        depth_status,
        git_state,
    ))
}

fn first_nonempty_text(values: &[Option<&Value>]) -> String {
    values
        .iter()
        .map(|value| value_text(*value))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

fn statusline_route(continuity: &Value) -> String {
    let skills = continuity
        .get("route")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| value_text(Some(item)))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match skills.len() {
        0 => "none".to_string(),
        1 => skills[0].clone(),
        count => format!("{}+{}", skills[0], count - 1),
    }
}

fn git_statusline_state(repo_root: &Path) -> (String, String) {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .arg("--branch")
        .arg("--untracked-files=no")
        .current_dir(repo_root)
        .output();
    let Ok(output) = output else {
        return ("nogit".to_string(), "nogit".to_string());
    };
    if !output.status.success() {
        return ("nogit".to_string(), "nogit".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let branch = lines
        .next()
        .and_then(|line| line.strip_prefix("## "))
        .map(|line| line.split("...").next().unwrap_or(line).trim().to_string())
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let changed = lines.any(|line| !line.trim().is_empty());
    (if changed { "dirty" } else { "clean" }.to_string(), branch)
}

fn statusline_decision_hint(
    blockers: &[String],
    next_actions: &[String],
    git_state: &str,
    status: &str,
) -> String {
    if let Some(blocker) = blockers.iter().find(|item| !item.trim().is_empty()) {
        return format!("blocked={}", short_statusline_text(blocker, 36));
    }
    if status == "completed" {
        if let Some(action) = next_actions.iter().find(|item| !item.trim().is_empty()) {
            return format!("next={}", short_statusline_text(action, 36));
        }
        if git_state == "dirty" {
            return "next=review local changes".to_string();
        }
        return "next=pick task".to_string();
    }
    if next_actions.iter().any(|item| !item.trim().is_empty()) {
        return "next=NEXT_ACTIONS".to_string();
    }
    "next=run verification".to_string()
}

fn short_statusline_text(value: &str, limit: usize) -> String {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.len() <= limit {
        text
    } else if limit <= 3 {
        text.chars().take(limit).collect()
    } else {
        format!("{}...", text.chars().take(limit - 3).collect::<String>())
    }
}
