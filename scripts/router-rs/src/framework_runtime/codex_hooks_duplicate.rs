//! Detect duplicate `router-rs codex hook` registrations (user + project hooks.json).

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Count nested `command` strings that invoke `router-rs` + `codex hook`.
pub fn count_codex_hook_router_rs_commands(hooks_json: &Value) -> usize {
    let mut n = 0usize;
    let Some(root) = hooks_json.get("hooks").and_then(Value::as_object) else {
        return 0;
    };
    for entries in root.values() {
        let Some(arr) = entries.as_array() else {
            continue;
        };
        for entry in arr {
            let commands: Vec<&str> = entry
                .get("hooks")
                .and_then(Value::as_array)
                .map(|inner| {
                    inner
                        .iter()
                        .filter_map(|h| h.get("command").and_then(Value::as_str))
                        .collect()
                })
                .unwrap_or_else(|| {
                    entry
                        .get("command")
                        .and_then(Value::as_str)
                        .into_iter()
                        .collect()
                });
            for cmd in commands {
                if cmd.contains("router-rs") && cmd.contains("codex hook") {
                    n += 1;
                }
            }
        }
    }
    n
}

fn codex_hooks_json_paths(repo_root: &Path) -> Vec<(String, PathBuf)> {
    let mut paths = Vec::new();
    let project = repo_root.join(".codex/hooks.json");
    if project.is_file() {
        paths.push(("project .codex/hooks.json".to_string(), project));
    }
    if let Some(home) = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))
    {
        let user = home.join("hooks.json");
        if user.is_file() {
            paths.push(("user Codex hooks.json".to_string(), user));
        }
    }
    paths
}

/// Human-readable WARN lines (empty if no issue).
pub fn collect_codex_hooks_duplicate_warnings(repo_root: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut total = 0usize;
    for (label, path) in codex_hooks_json_paths(repo_root) {
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&raw) else {
            warnings.push(format!("WARN: {label}: invalid JSON ({})", path.display()));
            continue;
        };
        let c = count_codex_hook_router_rs_commands(&v);
        total += c;
        if c > 1 {
            warnings.push(format!(
                "WARN: {label} registers {c} commands targeting `router-rs codex hook` — one event may run the hook multiple times ({})",
                path.display()
            ));
        }
    }
    if total > 2 {
        warnings.push(format!(
            "WARN: combined Codex hook registrations ({total}) suggest stacked user+project hooks; trim duplicate entries if Stop/PostToolUse fire repeatedly."
        ));
    }
    warnings
}

pub fn eprint_codex_hooks_duplicate_warnings(repo_root: &Path) {
    for line in collect_codex_hooks_duplicate_warnings(repo_root) {
        eprintln!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn count_duplicate_commands_in_one_file() {
        let hooks = json!({
            "hooks": {
                "Stop": [{
                    "hooks": [
                        {"command": "router-rs host cursor hook --event=Stop"},
                        {"command": "router-rs codex hook --event=Stop"}
                    ]
                }]
            }
        });
        assert_eq!(count_codex_hook_router_rs_commands(&hooks), 1);
        let hooks_dup = json!({
            "hooks": {
                "Stop": [{
                    "hooks": [
                        {"command": "router-rs codex hook --event=Stop"},
                        {"command": "router-rs codex hook --event=Stop"}
                    ]
                }]
            }
        });
        assert_eq!(count_codex_hook_router_rs_commands(&hooks_dup), 2);
    }

    #[test]
    fn collect_warnings_for_duplicate_fixture() {
        let dir = std::env::temp_dir().join(format!("codex-dup-warn-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let hooks_path = dir.join(".codex/hooks.json");
        if let Some(parent) = hooks_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let fixture = json!({
            "hooks": {
                "PostToolUse": [{
                    "hooks": [
                        {"command": "router-rs codex hook --event=PostToolUse"},
                        {"command": "router-rs codex hook --event=PostToolUse"}
                    ]
                }]
            }
        });
        fs::write(&hooks_path, serde_json::to_string(&fixture).unwrap()).unwrap();
        let warnings = collect_codex_hooks_duplicate_warnings(&dir);
        assert!(
            warnings.iter().any(|w| w.contains("registers 2 commands")),
            "warnings={warnings:?}"
        );
    }
}
