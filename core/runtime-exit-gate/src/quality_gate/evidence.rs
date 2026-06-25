//! Evidence 交叉校验：读取 EVIDENCE_INDEX.json 并 cross-link 到 RFV round。

use super::*;

/// 读取同任务目录下的 `EVIDENCE_INDEX.json`。
/// 返回 `Result` 以区分「文件不存在」（正常）和其他错误（可能需要诊断）。
#[derive(Debug)]
pub enum EvidenceReadError {
    InvalidTaskId(String),
    FileNotFound,
    ParseError(String),
    IoError(String),
}

impl std::fmt::Display for EvidenceReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidenceReadError::InvalidTaskId(id) => write!(f, "invalid task_id: {}", id),
            EvidenceReadError::FileNotFound => write!(f, "EVIDENCE_INDEX.json not found"),
            EvidenceReadError::ParseError(e) => write!(f, "parse error: {}", e),
            EvidenceReadError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for EvidenceReadError {}

fn read_evidence_index_artifacts_impl(
    repo_root: &Path,
    task_id: &str,
) -> std::result::Result<Vec<Value>, EvidenceReadError> {
    let tid = core_state_utils::path_guard::validate_task_id_component(task_id)
        .map_err(EvidenceReadError::InvalidTaskId)?;
    let path = repo_root
        .join("artifacts/current")
        .join(tid)
        .join("EVIDENCE_INDEX.json");
    if !path.is_file() {
        return Err(EvidenceReadError::FileNotFound);
    }
    let raw = fs::read_to_string(&path).map_err(|e| EvidenceReadError::IoError(e.to_string()))?;
    let val: Value =
        serde_json::from_str(&raw).map_err(|e| EvidenceReadError::ParseError(e.to_string()))?;
    val.get("artifacts")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| EvidenceReadError::ParseError("missing or non-array artifacts".to_string()))
}

/// 读取同任务目录下的 `EVIDENCE_INDEX.json`；非法 / 缺失视为空。
/// 注意：此函数保持向后兼容，返回空 Vec 而非 Result。
pub fn read_evidence_index_artifacts(repo_root: &Path, task_id: &str) -> Vec<Value> {
    read_evidence_index_artifacts_impl(repo_root, task_id).unwrap_or_default()
}

/// Cross-link 本轮 verify 与 EVIDENCE_INDEX 成功行：返回 `(refs, cross_check_label)`。
/// `refs` 为 EVIDENCE artifacts 数组中的索引（u64）；`cross_check_label` 为可选标签：
/// - `"no_evidence_window"`：claimed PASS 但窗口内无成功 evidence（**审计警告**，不阻断写入）
/// - `"evidence_after_fail"`：claimed FAIL 但仍有成功 evidence（信息性，便于人工核对）
/// - `None`：未声明 PASS/FAIL，或一致。
pub fn cross_link_evidence(
    repo_root: &Path,
    task_id: &str,
    state_obj: &Map<String, Value>,
    verify_result: &str,
) -> (Vec<Value>, Option<String>) {
    let artifacts = read_evidence_index_artifacts(repo_root, task_id);
    if artifacts.is_empty() {
        let label = if verify_result == "PASS" {
            Some("no_evidence_window".to_string())
        } else {
            None
        };
        return (Vec::new(), label);
    }
    let window_start = previous_round_window_start(state_obj);
    let mut refs: Vec<Value> = Vec::new();
    for (idx, row) in artifacts.iter().enumerate() {
        if !evidence_row_is_success(row) {
            continue;
        }
        let row_at = row
            .get("recorded_at")
            .or_else(|| row.get("at"))
            .and_then(Value::as_str);
        if is_timestamp_in_window(row_at, window_start.as_deref()) {
            refs.push(json!(idx as u64));
        }
    }
    let label = match verify_result {
        "PASS" if refs.is_empty() => Some("no_evidence_window".to_string()),
        "FAIL" if !refs.is_empty() => Some("evidence_after_fail".to_string()),
        _ => None,
    };
    (refs, label)
}
