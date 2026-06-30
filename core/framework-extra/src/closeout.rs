//! Closeout evaluation hooks and task-registry cache (Wave 3c-i successor).
//!
//! The original closeout_enforcement module was deleted from fr-contracts in
//! Wave 3c-i Phase C. Validation logic migrated to `core-state/closeout_validation.rs`.
//! This module provides the active wrapper: closeout record evaluation, environment-level
//! enforcement gating (env var `ROUTER_RS_CLOSEOUT_ENFORCEMENT`), and task-registry lookups
//! consumed by the `framework_hook_closeout` MCP tool path.

use core_errors::FrameworkError;
use core_state::closeout_validation::{
    CloseoutEvidenceContext, evaluate_closeout_record_value_with_context,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// Whether programmatic closeout enforcement is enabled in the current process.
///
/// - **Always enabled** by default in relaxed (advisory) mode.
/// - Explicitly **disable** with `ROUTER_RS_CLOSEOUT_ENFORCEMENT=skip`.
pub(crate) fn closeout_programmatic_enforcement_enabled() -> bool {
    if std::env::var("ROUTER_RS_CLOSEOUT_ENFORCEMENT").as_deref() == Ok("skip") {
        return false;
    }
    // CI: always enforce
    if std::env::var("GITHUB_ACTIONS").is_ok() || std::env::var("CI").is_ok() {
        return true;
    }
    // Local development: enable by default in relaxed (advisory) mode
    true
}

/// Default location for a task's closeout record.
pub fn closeout_record_path_for_task(
    repo_root: &Path,
    task_id: &str,
) -> Result<PathBuf, FrameworkError> {
    // SECURITY: Validate task_id to prevent path traversal attacks.
    // Only allow alphanumeric characters, hyphens, and underscores.
    let sanitized = task_id.trim();
    if sanitized.is_empty() {
        return Err(FrameworkError::validation("task_id cannot be empty"));
    }
    if !sanitized
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(FrameworkError::validation(format!(
            "task_id contains invalid characters (only alphanumeric, hyphen, underscore allowed): {:?}",
            sanitized
        )));
    }

    let path = repo_root
        .join("artifacts")
        .join("closeout")
        .join(format!("{}.json", sanitized));

    // SECURITY: Verify the resolved path is still within the expected directory.
    // This prevents any remaining path traversal attempts (e.g., via symlinks).
    let closeout_dir = repo_root.join("artifacts").join("closeout");
    let canonical_path = std::fs::canonicalize(&path).or_else(|_| {
        std::fs::canonicalize(&closeout_dir).map(|p| p.join(format!("{}.json", sanitized)))
    });
    if let Ok(canonical) = canonical_path {
        let canonical_dir = std::fs::canonicalize(&closeout_dir)?;
        if !canonical.starts_with(&canonical_dir) {
            return Err(FrameworkError::validation("path traversal detected"));
        }
    }

    Ok(path)
}

struct CachedTaskRegistry {
    content: Value,
    mtime: Option<SystemTime>,
}

static TASK_REGISTRY_CACHE: Mutex<Option<CachedTaskRegistry>> = Mutex::new(None);

/// 从 task_registry.json 中读取 task_id（pointer 机制移除后的回退）。
/// 优先返回 focus_task_id，再返回 tasks 数组中第一个。
pub fn first_task_id_from_registry(repo_root: &Path) -> Option<String> {
    let registry_path = repo_root.join("artifacts/current/task_registry.json");
    let mtime = fs::metadata(&registry_path)
        .ok()
        .and_then(|m| m.modified().ok());
    {
        let guard = TASK_REGISTRY_CACHE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ref cached) = *guard
            && cached.mtime == mtime
        {
            return extract_first_task_id_from_value(&cached.content);
        }
    }
    let raw = fs::read_to_string(&registry_path).ok()?;
    let data: Value = serde_json::from_str(&raw).ok()?;
    {
        let mut guard = TASK_REGISTRY_CACHE
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *guard = Some(CachedTaskRegistry {
            content: data.clone(),
            mtime,
        });
    }
    extract_first_task_id_from_value(&data)
}

fn extract_first_task_id_from_value(data: &Value) -> Option<String> {
    if let Some(focus) = data.get("focus_task_id").and_then(Value::as_str) {
        let focus = focus.trim();
        if !focus.is_empty() {
            return Some(focus.to_string());
        }
    }
    let tasks = data.get("tasks").and_then(Value::as_array)?;
    for row in tasks {
        if let Some(tid) = row.get("task_id").and_then(Value::as_str) {
            let tid = tid.trim();
            if !tid.is_empty() {
                return Some(tid.to_string());
            }
        }
    }
    None
}

/// Evaluate a materialized closeout record JSON file, attaching an EvidenceContext (R8) when possible.
/// Shared Stop/closeout guard when assistant or user text claims completion (Cursor/Codex parity).
pub fn closeout_stop_followup_for_completion_text(repo_root: &Path, text: &str) -> Option<String> {
    if text.trim().is_empty() || !framework_core::hook_common::contains_completion_claim_token(text) {
        return None;
    }
    // Pointer 机制已移除：先尝试 resolve_task_view，再回退到 task_registry.json
    let tid = core_state::task_state::resolve_task_view(repo_root, None)
        .task_id
        .filter(|s| !s.is_empty())
        .or_else(|| first_task_id_from_registry(repo_root));
    let tid = tid?;
    if !closeout_programmatic_enforcement_enabled() {
        tracing::debug!("closeout enforcement disabled via ROUTER_RS_CLOSEOUT_ENFORCEMENT=skip");
        return None;
    }
    let record_path = closeout_record_path_for_task(repo_root, &tid).ok()?;
    if !record_path.is_file() {
        return Some(format!(
            "CLOSEOUT_FOLLOWUP task_id={tid} reason=missing_record path={}\n\
            请在完成态宣称前写入 closeout record 并通过评估。",
            record_path.display()
        ));
    }
    let eval = evaluate_closeout_record_file_for_task(repo_root, &tid, &record_path).ok()?;
    if eval
        .get("closeout_allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    Some(format!(
        "CLOSEOUT_FOLLOWUP task_id={tid} reason=evaluation_failed path={}",
        record_path.display()
    ))
}

pub fn evaluate_closeout_record_file_for_task(
    repo_root: &Path,
    task_id: &str,
    record_path: &Path,
) -> Result<Value, FrameworkError> {
    let tid = task_id.trim();
    if tid.is_empty() {
        return Err(FrameworkError::validation("task_id is empty"));
    }
    let text = std::fs::read_to_string(record_path).map_err(|err| {
        FrameworkError::validation(format!(
            "read closeout record failed ({}): {err}",
            record_path.display()
        ))
    })?;
    let record: Value = serde_json::from_str(&text).map_err(|err| {
        FrameworkError::validation(format!(
            "parse closeout record JSON failed ({}): {err}",
            record_path.display()
        ))
    })?;
    let ctx = CloseoutEvidenceContext::new(Some(tid.to_string()), repo_root);
    evaluate_closeout_record_value_with_context(record, &ctx).map_err(|err| {
        FrameworkError::validation(format!("closeout record evaluation failed: {err}"))
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::fs;

    #[test]
    fn closeout_record_path_valid_task_id() {
        let root = std::path::Path::new("/tmp/test-repo");
        let path = closeout_record_path_for_task(root, "task-123").unwrap();
        assert_eq!(
            path,
            root.join("artifacts/closeout/task-123.json")
        );
    }

    #[test]
    fn closeout_record_path_empty_task_id_errors() {
        let root = std::path::Path::new("/tmp/test-repo");
        let result = closeout_record_path_for_task(root, "");
        assert!(result.is_err());
    }

    #[test]
    fn closeout_record_path_traversal_task_id_errors() {
        let root = std::path::Path::new("/tmp/test-repo");
        let result = closeout_record_path_for_task(root, "../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn closeout_record_path_special_chars_errors() {
        let root = std::path::Path::new("/tmp/test-repo");
        let result = closeout_record_path_for_task(root, "task; rm -rf /");
        assert!(result.is_err());
    }

    #[test]
    fn first_task_id_from_registry_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let result = first_task_id_from_registry(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn first_task_id_from_registry_with_focus() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("artifacts/current");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("task_registry.json"),
            r#"{"focus_task_id": "focused-task", "tasks": []}"#,
        )
        .unwrap();
        let result = first_task_id_from_registry(tmp.path());
        assert_eq!(result.as_deref(), Some("focused-task"));
    }

    #[test]
    fn first_task_id_from_registry_from_tasks_array() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("artifacts/current");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("task_registry.json"),
            r#"{"tasks": [{"task_id": "array-task"}, {"task_id": "second"}]}"#,
        )
        .unwrap();
        let result = first_task_id_from_registry(tmp.path());
        assert_eq!(result.as_deref(), Some("array-task"));
    }

    #[test]
    fn first_task_id_from_registry_invalid_json() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("artifacts/current");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("task_registry.json"), "not json").unwrap();
        let result = first_task_id_from_registry(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn closeout_stop_followup_empty_text() {
        let tmp = tempfile::tempdir().unwrap();
        let result = closeout_stop_followup_for_completion_text(tmp.path(), "");
        assert!(result.is_none());
    }

    #[test]
    fn closeout_stop_followup_no_claim_token() {
        let tmp = tempfile::tempdir().unwrap();
        let result = closeout_stop_followup_for_completion_text(tmp.path(), "hello world");
        assert!(result.is_none());
    }
}
