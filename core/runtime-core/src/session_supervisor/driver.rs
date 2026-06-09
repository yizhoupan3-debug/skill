use super::runtime::shell_join;
use super::types::DriverCommandSpec;
use serde_json::{json, Value};

pub(super) fn is_safe_worktree_slug(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub(super) fn resolve_worktree_cwd(cwd: &str, worktree_name: Option<&str>, worktree_path: Option<&str>) -> String {
    if let Some(path) = worktree_path {
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            // Reject paths with traversal components
            if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return cwd.to_string();
            }
            return path.to_string();
        }
        // Relative path: resolve against cwd, reject traversal
        let resolved = std::path::Path::new(cwd).join(p);
        if resolved.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return cwd.to_string();
        }
        return resolved.to_string_lossy().to_string();
    }
    if let Some(name) = worktree_name {
        if !is_safe_worktree_slug(name) {
            return cwd.to_string();
        }
        let wt_path = std::path::Path::new(cwd).join(".claude/worktrees").join(name);
        return wt_path.to_string_lossy().to_string();
    }
    cwd.to_string()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_driver_command(
    host: &str,
    cwd: &str,
    prompt: Option<String>,
    resume_target: Option<String>,
    resume_mode: &str,
    resume_only: bool,
    _native_tmux_requested: bool,
    worktree_name: Option<String>,
    worktree_path: Option<String>,
) -> Result<DriverCommandSpec, String> {
    let effective_cwd = resolve_worktree_cwd(cwd, worktree_name.as_deref(), worktree_path.as_deref());
    let lowered = host.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "codex" => {
            let mut args = vec!["-C".to_string(), effective_cwd.clone()];
            if resume_only {
                args.push("resume".to_string());
                if let Some(target) = resume_target {
                    if target == "last" || resume_mode == "last" {
                        args.push("--last".to_string());
                    } else {
                        args.push(target);
                    }
                } else {
                    args.push("--last".to_string());
                }
            } else if let Some(prompt) = prompt {
                args.push(prompt);
            }
            Ok(DriverCommandSpec {
                driver_id: "codex_driver".to_string(),
                binary: "codex".to_string(),
                shell_command: shell_join("codex", &args),
                args,
                supports_resume: true,
                supports_native_tmux: false,
                supports_external_tmux: true,
            })
        }
        "claude" | "claude-code" => {
            let mut args = vec!["--print".to_string()];
            if resume_only {
                if let Some(target) = resume_target {
                    args.push("--resume".to_string());
                    args.push(target);
                }
            } else if let Some(ref p) = prompt {
                args.push("-p".to_string());
                args.push(p.clone());
            }
            Ok(DriverCommandSpec {
                driver_id: "claude_code_driver".to_string(),
                binary: "claude".to_string(),
                shell_command: shell_join("claude", &args),
                args,
                supports_resume: true,
                supports_native_tmux: false,
                supports_external_tmux: true,
            })
        }
        other => Err(format!("Unsupported session supervisor host: {other}")),
    }
}

pub(super) fn driver_id_for_host(host: &str) -> &'static str {
    match host.trim().to_ascii_lowercase().as_str() {
        "codex" => "codex_driver",
        "claude" | "claude-code" => "claude_code_driver",
        _ => "unknown_driver",
    }
}

pub(super) fn ensure_lane_contract_metadata(
    metadata: Value,
    worker_id: &str,
    host: &str,
    cwd: &str,
    prompt: Option<&str>,
    lane_contract: Option<Value>,
) -> Value {
    let mut object = metadata.as_object().cloned().unwrap_or_default();
    let existing_lane_contract = object.remove("lane_contract").or(lane_contract);
    object.insert(
        "lane_contract".to_string(),
        merge_lane_contract_defaults(
            existing_lane_contract,
            worker_id,
            host,
            cwd,
            prompt.unwrap_or("bounded worker lane"),
        ),
    );
    Value::Object(object)
}

pub(super) fn merge_lane_contract_defaults(
    lane_contract: Option<Value>,
    worker_id: &str,
    host: &str,
    cwd: &str,
    lane_goal: &str,
) -> Value {
    let defaults = json!({
        "lane_id": worker_id,
        "lane_owner": host,
        "lane_goal": lane_goal,
        "goal": lane_goal,
        "bounded_scope": cwd,
        "forbidden_scope": "outside assigned lane-local scope",
        "verification_required": true,
        "expected_output": {
            "changed_files": [],
            "evidence": [],
            "verification": [],
            "risk": null,
            "next_action": null
        },
        "final_digest": null,
        "evidence_ref": null,
        "integration_status": "planned",
        "verification_status": "not-started",
        "recovery_anchor": worker_id
    });
    let mut merged = defaults.as_object().cloned().unwrap_or_default();
    if let Some(Value::Object(provided)) = lane_contract {
        for (key, value) in provided {
            if key == "expected_output" {
                let mut expected = merged
                    .get("expected_output")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if let Value::Object(provided_expected) = value {
                    for (nested_key, nested_value) in provided_expected {
                        expected.insert(nested_key, nested_value);
                    }
                    merged.insert(key, Value::Object(expected));
                } else {
                    merged.insert(key, value);
                }
            } else {
                merged.insert(key, value);
            }
        }
    }
    Value::Object(merged)
}

pub(super) fn default_resume_mode(_host: &str) -> &'static str {
    "last"
}

