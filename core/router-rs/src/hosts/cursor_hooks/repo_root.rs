use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn push_path_candidate(out: &mut Vec<PathBuf>, raw: &str) {
    let t = raw.trim();
    if !t.is_empty() {
        out.push(PathBuf::from(t));
    }
}

/// 自 `start`（文件或目录）向上查找包含 `.cursor/hooks.json` 的目录。
pub(super) fn first_ancestor_with_hooks_json(start: &Path) -> Option<PathBuf> {
    let start_meta = fs::metadata(start).ok();
    let mut cur = if start_meta.as_ref().is_some_and(|m| m.is_file()) {
        start.parent()?.to_path_buf()
    } else if start_meta.as_ref().is_some_and(|m| m.is_dir()) {
        start.to_path_buf()
    } else {
        start
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| start.to_path_buf())
    };

    for _ in 0..64 {
        if cur.join(".cursor").join("hooks.json").is_file() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

/// 合并 CLI `--repo-root`、环境变量与 stdin JSON 中的路径字段，解析含 `.cursor/hooks.json` 的策略根目录。
///
/// 优先使用载荷中的 `cwd` / `workspaceFolder` 等（Cursor 侧通常比 hook 进程 pwd 更可靠），避免子目录会话时状态写到错误根路径。
pub fn resolve_cursor_hook_repo_root(
    cli_root: Option<&Path>,
    payload: &Value,
) -> Result<PathBuf, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(v) = std::env::var("ROUTER_RS_CURSOR_WORKSPACE_ROOT") {
        push_path_candidate(&mut candidates, &v);
    }
    if let Ok(v) = std::env::var("CURSOR_WORKSPACE_ROOT") {
        push_path_candidate(&mut candidates, &v);
    }

    for key in [
        "workspaceFolder",
        "workspace_folder",
        "workspaceRoot",
        "workspace_root",
        "cwd",
        "root",
    ] {
        if let Some(s) = payload.get(key).and_then(Value::as_str) {
            push_path_candidate(&mut candidates, s);
        }
    }

    if let Some(p) = payload
        .get("tool_input")
        .and_then(|t| t.get("path"))
        .and_then(Value::as_str)
    {
        push_path_candidate(&mut candidates, p);
    }
    if let Some(p) = payload.get("file_path").and_then(Value::as_str) {
        push_path_candidate(&mut candidates, p);
    }

    if let Some(p) = cli_root {
        candidates.push(p.to_path_buf());
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }

    for c in candidates {
        if let Some(found) = first_ancestor_with_hooks_json(&c) {
            let canon = fs::canonicalize(&found).unwrap_or(found);
            return Ok(canon);
        }
    }

    let base = cli_root
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            "cursor hook: cannot resolve repo root (no .cursor/hooks.json marker and no fallback cwd)"
                .to_string()
        })?;
    Ok(fs::canonicalize(&base).unwrap_or(base))
}
