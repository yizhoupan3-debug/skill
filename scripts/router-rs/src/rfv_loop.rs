//! Review-Fix-Verify 多轮闭环：Rust 真源 `RFV_LOOP_STATE.json` + stdio，支撑长任务轮次账本与宿主并行 lane 之后的 supervisor 合并落盘。

use crate::atomic_write::write_atomic_json;
use crate::autopilot_goal::read_active_task_id;
use crate::framework_runtime::resolve_repo_root_arg;
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const RFV_LOOP_STATE_FILENAME: &str = "RFV_LOOP_STATE.json";
pub const RFV_LOOP_SCHEMA_VERSION: &str = "router-rs-rfv-loop-v1";
/// Repo-relative path; keep in sync with `cursor_hooks` merge logic that surfaces this substring.
pub const RFV_EXTERNAL_RESEARCH_SCHEMA_REL_PATH: &str =
    "configs/framework/RFV_EXTERNAL_RESEARCH.schema.json";
/// `retrieval_trace` prose fields must be at least this many **trimmed** chars under strict mode.
pub const EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN: usize = 40;
/// Allowed `verify_result` enum (uppercase); see `reasoning-depth-contract.md`.
/// `append_round` rejects values outside this set so PASS/FAIL is auditable, not free-form.
pub const ALLOWED_VERIFY_RESULTS: &[&str] = &["PASS", "FAIL", "SKIPPED", "UNKNOWN"];

fn nonempty_trimmed_string_at(value: &Value, ctx: &str, key: &str) -> Result<(), String> {
    let Some(t) = value.as_str() else {
        return Err(format!("{ctx}: `{key}` must be string"));
    };
    if t.trim().is_empty() {
        return Err(format!("{ctx}: `{key}` must be non-empty"));
    }
    Ok(())
}

fn validate_nonempty_string_items(arr: &[Value], ctx: &str, arr_name: &str) -> Result<(), String> {
    if arr.is_empty() {
        return Err(format!("{ctx}: `{arr_name}` must be non-empty"));
    }
    for (idx, elem) in arr.iter().enumerate() {
        let label = format!("{ctx}.{arr_name}[{idx}]");
        nonempty_trimmed_string_at(elem, &label, "item")?;
    }
    Ok(())
}

/// Heuristic: source string looks like a machine-checkable external pointer (URL, DOI, arXiv, …).
pub fn source_traceable_heuristic(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return true;
    }
    if lower.starts_with("doi:10.") {
        return true;
    }
    if lower.starts_with("10.") && lower.contains('/') {
        return true;
    }
    for prefix in [
        "arxiv:",
        "pmid:",
        "isbn:",
        "dataset:",
        "official_doc:",
        "huggingface:",
        "hf:",
        "github:",
        "kaggle:",
        "geojson:",
    ] {
        if lower.starts_with(prefix) {
            return true;
        }
    }
    false
}

fn validate_source_list_traceable(
    sources: &[Value],
    ctx: &str,
    min_len: usize,
    err_label: &str,
) -> Result<(), String> {
    if sources.len() < min_len {
        return Err(format!(
            "external_research strict: {ctx} `{err_label}` must have at least {min_len} entries, got {}",
            sources.len()
        ));
    }
    for (j, sv) in sources.iter().enumerate() {
        let Some(s) = sv.as_str() else {
            return Err(format!(
                "external_research strict: {ctx} `{err_label}[{j}]` must be string"
            ));
        };
        if !source_traceable_heuristic(s) {
            return Err(format!(
                "external_research strict: {ctx} `{err_label}[{j}]` not traceable: {s:?}"
            ));
        }
    }
    Ok(())
}

/// Stricter checks when `RFV_LOOP_STATE.external_research_strict` is true; run only after
/// [`validate_external_research_structured`] succeeds.
pub fn validate_external_research_strict(v: &Value) -> Result<(), String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "external_research strict: root must be object".to_string())?;

    let Some(unk) = obj.get("unknowns") else {
        return Err(
            "external_research strict: missing `unknowns` key (use [] or null)".to_string(),
        );
    };
    if !unk.is_null() && !unk.is_array() {
        return Err("external_research strict: `unknowns` must be array or null".to_string());
    }

    let claims = obj
        .get("claims")
        .and_then(Value::as_array)
        .ok_or_else(|| "external_research strict: claims must be array".to_string())?;
    let claims_len = claims.len();

    let sweep = obj
        .get("contradiction_sweep")
        .and_then(Value::as_array)
        .ok_or_else(|| "external_research strict: contradiction_sweep must be array".to_string())?;
    let min_sweep = std::cmp::max(2, claims_len / 2);
    if sweep.len() < min_sweep {
        return Err(format!(
            "external_research strict: contradiction_sweep must have at least {min_sweep} entries, got {}",
            sweep.len()
        ));
    }
    for (i, item) in sweep.iter().enumerate() {
        let ctx = format!("contradiction_sweep[{i}]");
        let row = item
            .as_object()
            .ok_or_else(|| format!("external_research strict: {ctx} entry must be object"))?;
        let sources = row
            .get("sources")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("external_research strict: {ctx} sources must be array"))?;
        validate_source_list_traceable(sources, &ctx, 1, "sources")?;
    }

    for (i, c) in claims.iter().enumerate() {
        let ctx = format!("claims[{i}]");
        let row = c
            .as_object()
            .ok_or_else(|| format!("external_research strict: {ctx} must be object"))?;
        let sources = row
            .get("sources")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("external_research strict: {ctx} sources must be array"))?;
        validate_source_list_traceable(sources, &ctx, 2, "sources")?;
    }

    let trace = obj
        .get("retrieval_trace")
        .and_then(Value::as_object)
        .ok_or_else(|| "external_research strict: retrieval_trace must be object".to_string())?;
    let queries = trace
        .get("queries_used")
        .and_then(Value::as_array)
        .ok_or_else(|| "external_research strict: queries_used must be array".to_string())?;
    if queries.len() < 3 {
        return Err(format!(
            "external_research strict: queries_used must have at least 3 entries, got {}",
            queries.len()
        ));
    }

    for key in ["inclusion_rules", "exclusions", "exclusion_rationale"] {
        let field = trace.get(key).and_then(Value::as_str).ok_or_else(|| {
            format!("external_research strict: retrieval_trace `{key}` must be string")
        })?;
        if field.trim().len() < EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN {
            return Err(format!(
                "external_research strict: retrieval_trace `{key}` must be at least {} non-whitespace chars (trimmed len={})",
                EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN,
                field.trim().len()
            ));
        }
    }

    Ok(())
}

fn external_research_strict_from_loaded_state(obj: &Map<String, Value>) -> bool {
    match obj.get("external_research_strict") {
        Some(Value::Bool(b)) => *b,
        _ => false,
    }
}

/// Validates optional structured external research blob for `append_round`.
/// Aligns with lane-templates **deep mode** YAML (`claims`, `contradiction_sweep`, `retrieval_trace`, optional `unknowns` / `quantitative_replays`).
pub fn validate_external_research_structured(v: &Value) -> Result<(), String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "external_research must be a JSON object".to_string())?;

    let claims = obj
        .get("claims")
        .ok_or_else(|| "external_research missing `claims`".to_string())?;
    let claims = claims
        .as_array()
        .ok_or_else(|| "external_research.claims must be array".to_string())?;
    if claims.is_empty() {
        return Err("external_research.claims must be non-empty".to_string());
    }
    for (i, c) in claims.iter().enumerate() {
        let ctx = format!("external_research.claims[{i}]");
        let row = c
            .as_object()
            .ok_or_else(|| format!("{ctx}: claim entry must be object"))?;
        let claim_v = row
            .get("claim")
            .ok_or_else(|| format!("{ctx}: missing `claim`"))?;
        nonempty_trimmed_string_at(claim_v, &ctx, "claim")?;
        let sources = row
            .get("sources")
            .ok_or_else(|| format!("{ctx}: missing `sources`"))?;
        let sources = sources
            .as_array()
            .ok_or_else(|| format!("{ctx}: sources must be array"))?;
        validate_nonempty_string_items(sources, &ctx, "sources")?;
    }

    let sweep_key = obj
        .get("contradiction_sweep")
        .ok_or_else(|| "external_research missing `contradiction_sweep`".to_string())?;
    let sweep = sweep_key
        .as_array()
        .ok_or_else(|| "external_research.contradiction_sweep must be array".to_string())?;
    if sweep.is_empty() {
        return Err("external_research.contradiction_sweep must be non-empty".to_string());
    }
    for (i, item) in sweep.iter().enumerate() {
        let ctx = format!("external_research.contradiction_sweep[{i}]");
        let row = item
            .as_object()
            .ok_or_else(|| format!("{ctx}: entry must be object"))?;
        let rk = row
            .get("related_claim_or_topic")
            .ok_or_else(|| format!("{ctx}: missing `related_claim_or_topic`"))?;
        nonempty_trimmed_string_at(rk, &ctx, "related_claim_or_topic")?;
        let contradict = row
            .get("contradicting_or_limiting_evidence")
            .ok_or_else(|| format!("{ctx}: missing `contradicting_or_limiting_evidence`"))?;
        nonempty_trimmed_string_at(contradict, &ctx, "contradicting_or_limiting_evidence")?;
        let sources = row
            .get("sources")
            .ok_or_else(|| format!("{ctx}: missing `sources`"))?;
        let sources = sources
            .as_array()
            .ok_or_else(|| format!("{ctx}: sources must be array"))?;
        validate_nonempty_string_items(sources, &ctx, "sources")?;
    }

    if let Some(u) = obj.get("unknowns") {
        if u.is_null() {
            // skip unknowns
        } else {
            let arr = u
                .as_array()
                .ok_or_else(|| "external_research.unknowns must be array or null".to_string())?;
            for (i, rowv) in arr.iter().enumerate() {
                let ctx = format!("external_research.unknowns[{i}]");
                let row = rowv
                    .as_object()
                    .ok_or_else(|| format!("{ctx}: entry must be object"))?;
                let q = row
                    .get("question")
                    .ok_or_else(|| format!("{ctx}: missing `question`"))?;
                nonempty_trimmed_string_at(q, &ctx, "question")?;
                let why = row
                    .get("why_insufficient")
                    .ok_or_else(|| format!("{ctx}: missing `why_insufficient`"))?;
                nonempty_trimmed_string_at(why, &ctx, "why_insufficient")?;
            }
        }
    }

    if let Some(qr) = obj.get("quantitative_replays") {
        if qr.is_null()
            || (qr
                .as_str()
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("none")))
        {
            // optional / explicit N/A sentinel
        } else if let Some(entries) = qr.as_array() {
            for (i, rowv) in entries.iter().enumerate() {
                let ctx = format!("external_research.quantitative_replays[{i}]");
                let row = rowv
                    .as_object()
                    .ok_or_else(|| format!("{ctx}: entry must be object"))?;
                for key in [
                    "dataset_or_source_id",
                    "version_or_snapshot",
                    "window",
                    "replay_command",
                ] {
                    let f = row
                        .get(key)
                        .ok_or_else(|| format!("{ctx}: missing `{key}`"))?;
                    nonempty_trimmed_string_at(f, &ctx, key)?;
                }
            }
        } else {
            return Err(
                "external_research.quantitative_replays must be array, null, \"none\", or absent"
                    .to_string(),
            );
        }
    }

    let trace = obj
        .get("retrieval_trace")
        .ok_or_else(|| "external_research missing `retrieval_trace`".to_string())?;
    let tr = trace
        .as_object()
        .ok_or_else(|| "external_research.retrieval_trace must be object".to_string())?;
    let queries = tr
        .get("queries_used")
        .ok_or_else(|| "retrieval_trace missing `queries_used`".to_string())?;
    let queries = queries
        .as_array()
        .ok_or_else(|| "retrieval_trace.queries_used must be array".to_string())?;
    validate_nonempty_string_items(queries, "external_research.retrieval_trace", "queries_used")?;
    for key in ["inclusion_rules", "exclusions", "exclusion_rationale"] {
        let field = tr
            .get(key)
            .ok_or_else(|| format!("retrieval_trace missing `{key}`"))?;
        nonempty_trimmed_string_at(field, "external_research.retrieval_trace", key)?;
    }

    Ok(())
}

fn normalize_verify_result(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok("UNKNOWN".to_string());
    }
    let upper = trimmed.to_ascii_uppercase();
    if ALLOWED_VERIFY_RESULTS.iter().any(|s| *s == upper) {
        return Ok(upper);
    }
    Err(format!(
        "verify_result must be one of {ALLOWED_VERIFY_RESULTS:?} (case-insensitive), got {raw:?}"
    ))
}

/// Optional hard gates on RFV **收口轮**预览（`append_round`）：supervisor 显式 **`close`/`closed`**，
/// 或 **`max_rounds` 耗尽**（`round_n >= max_rounds` 且非 block）自动记 `closed` 时同样校验。
#[derive(Debug, Clone)]
struct RfvCloseGates {
    enabled: bool,
    require_last_round_verify_pass: bool,
    min_depth_score: Option<u8>,
    block_on_rfv_pass_without_evidence: bool,
    require_external_research_object_when_strict_on_close: bool,
}

fn parse_close_gates(state: &Map<String, Value>) -> Option<RfvCloseGates> {
    let raw = state.get("close_gates")?;
    if raw.is_null() {
        return None;
    }
    let o = raw.as_object()?;
    Some(RfvCloseGates {
        enabled: o.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        require_last_round_verify_pass: o
            .get("require_last_round_verify_pass")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        min_depth_score: o
            .get("min_depth_score")
            .and_then(Value::as_u64)
            .map(|u| u.min(3) as u8),
        block_on_rfv_pass_without_evidence: o
            .get("block_on_rfv_pass_without_evidence")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        require_external_research_object_when_strict_on_close: o
            .get("require_external_research_object_when_strict_on_close")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn enforce_rfv_close_gates(
    repo_root: &Path,
    task_id: &str,
    preview_rfv: &Map<String, Value>,
    closing_round: &Map<String, Value>,
    gates: &RfvCloseGates,
) -> Result<(), String> {
    if !gates.enabled {
        return Ok(());
    }
    if gates.require_last_round_verify_pass {
        let vr = closing_round
            .get("verify_result")
            .and_then(Value::as_str)
            .unwrap_or("");
        if vr != "PASS" {
            return Err(format!(
                "RFV close_gates: require_last_round_verify_pass but verify_result={vr:?}"
            ));
        }
    }
    let allow_external = preview_rfv
        .get("allow_external_research")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let strict_task = external_research_strict_from_loaded_state(preview_rfv);
    if gates.require_external_research_object_when_strict_on_close && allow_external && strict_task
    {
        let has_obj = closing_round
            .get("external_research")
            .is_some_and(|v| !v.is_null() && v.is_object());
        if !has_obj {
            return Err(
                "RFV close_gates: require_external_research_object_when_strict_on_close but closing round has no structured external_research object"
                    .to_string(),
            );
        }
    }
    let (_, evidence_ok) =
        crate::autopilot_goal::task_evidence_artifacts_summary_for_task(repo_root, task_id);
    let goal_opt = crate::autopilot_goal::read_goal_state(repo_root, Some(task_id))
        .ok()
        .flatten();
    let preview_val = Value::Object(preview_rfv.clone());
    let dc = crate::task_state::depth_compliance_aggregate(
        goal_opt.as_ref(),
        Some(&preview_val),
        evidence_ok,
    );
    if let Some(min) = gates.min_depth_score {
        if dc.depth_score < min {
            return Err(format!(
                "RFV close_gates: depth_score={} < min_depth_score={}",
                dc.depth_score, min
            ));
        }
    }
    if gates.block_on_rfv_pass_without_evidence && dc.rfv_pass_without_evidence_count > 0 {
        return Err(format!(
            "RFV close_gates: block_on_rfv_pass_without_evidence but rfv_pass_without_evidence_count={}",
            dc.rfv_pass_without_evidence_count
        ));
    }
    Ok(())
}

/// EVIDENCE_INDEX 行视为「成功验证」：`success==true` 或 `exit_code==0`。
/// 实际规则下沉到 [`crate::hook_common::evidence_index_entry_implies_success`]，与 `autopilot_goal`
/// 共用一份口径（避免历史上的两套独立判定函数）。
fn evidence_row_is_success(row: &Value) -> bool {
    crate::hook_common::evidence_index_entry_implies_success(row)
}

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
) -> Result<Vec<Value>, EvidenceReadError> {
    let tid = crate::path_guard::validate_task_id_component(task_id)
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
fn read_evidence_index_artifacts(repo_root: &Path, task_id: &str) -> Vec<Value> {
    read_evidence_index_artifacts_impl(repo_root, task_id).unwrap_or_default()
}

/// 取上一轮 `at`；若无上一轮则取 RFV state 的 `updated_at`；都无则返回 None。
fn previous_round_window_start(state_obj: &Map<String, Value>) -> Option<String> {
    let rounds = state_obj.get("rounds").and_then(Value::as_array)?;
    if let Some(last) = rounds.last() {
        if let Some(at) = last.get("at").and_then(Value::as_str) {
            return Some(at.to_string());
        }
    }
    state_obj
        .get("updated_at")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Cross-link 本轮 verify 与 EVIDENCE_INDEX 成功行：返回 `(refs, cross_check_label)`。
/// `refs` 为 EVIDENCE artifacts 数组中的索引（u64）；`cross_check_label` 为可选标签：
/// - `"no_evidence_window"`：claimed PASS 但窗口内无成功 evidence（**审计警告**，不阻断写入）
/// - `"evidence_after_fail"`：claimed FAIL 但仍有成功 evidence（信息性，便于人工核对）
/// - `None`：未声明 PASS/FAIL，或一致。
fn cross_link_evidence(
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

/// 解析 RFC 3339 时间字符串为 DateTime<Utc>。
/// 只接受标准 RFC 3339 格式，拒绝非标准格式以避免歧义。
fn parse_iso_datetime(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .ok()
}

/// 判断一个 evidence 行的 recorded_at 是否在 window 内。
/// 保守处理：解析失败视为不在窗口内（避免 false positive）。
fn is_timestamp_in_window(row_at: Option<&str>, window_start: Option<&str>) -> bool {
    match (window_start, row_at) {
        // 两个时间都存在：使用真正的时间比较
        (Some(start), Some(at)) => {
            let start_dt = match parse_iso_datetime(start) {
                Some(dt) => dt,
                None => return false, // 解析失败：保守处理
            };
            let at_dt = match parse_iso_datetime(at) {
                Some(dt) => dt,
                None => return false, // 解析失败：保守处理
            };
            at_dt > start_dt
        }
        // 有 window_start 但 row 没有时间：不在任何窗口内
        (Some(_), None) => false,
        // 无 window_start：表示从头开始，所有有效时间的记录都在窗口内
        (None, Some(_)) => true,
        // 两者都没有：不在窗口内
        (None, None) => false,
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn rfv_loop_state_path(repo_root: &Path, task_id: &str) -> Result<PathBuf, String> {
    let tid = crate::path_guard::validate_task_id_component(task_id)?;
    Ok(repo_root
        .join("artifacts/current")
        .join(tid)
        .join(RFV_LOOP_STATE_FILENAME))
}

/// Autopilot 在同 task 上 `start`/`upsert`/`resume` 时结束 RFV 的 `loop_status=active`（与 GOAL 互斥；保留文件并标记 `superseded`）。
pub(crate) fn deactivate_rfv_for_conflict_with_autopilot(
    repo_root: &Path,
    task_id: &str,
) -> Result<bool, String> {
    if task_id.trim().is_empty() {
        return Ok(false);
    }
    if crate::path_guard::safe_task_id_component(task_id).is_none() {
        return Ok(false);
    }
    let path = rfv_loop_state_path(repo_root, task_id)?;
    if !path.is_file() {
        return Ok(false);
    }
    let mut state = read_rfv_loop_state(repo_root, Some(task_id))?
        .ok_or_else(|| format!("RFV_LOOP_STATE missing at {}", path.display()))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "RFV_LOOP_STATE root must be object".to_string())?;
    let active = obj
        .get("loop_status")
        .and_then(Value::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("active"));
    if !active {
        return Ok(false);
    }
    obj.insert("loop_status".to_string(), json!("superseded"));
    obj.insert("superseded_by".to_string(), json!("autopilot_goal"));
    obj.insert("updated_at".to_string(), json!(now_iso()));
    write_atomic_json(&path, &state)?;
    let tx = crate::task_ledger::LedgerTransaction {
        ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        tx_type: "rfv_loop_state".to_string(),
        payload: state.clone(),
        idempotency_key: None,
        seq: None,
        schema_version: Some(1),
    };
    if let Err(e) = crate::task_ledger::append_transaction_assuming_l1_held(repo_root, task_id, tx) {
        eprintln!("[router-rs] failed to append rfv transaction to TASK_LEDGER: {e}");
    }
    crate::task_state_aggregate::sync_task_state_aggregate_best_effort(repo_root, task_id);
    Ok(true)
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
    crate::path_guard::validate_task_id_component(&task_id)
        .map_err(|e| format!("framework_rfv_loop: invalid task_id for RFV_LOOP_STATE path: {e}"))?;
    let path = rfv_loop_state_path(repo_root, &task_id)?;
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

fn value_array_or_empty(payload: &Value, key: &str) -> Result<Vec<Value>, String> {
    let Some(v) = payload.get(key) else {
        return Ok(Vec::new());
    };
    if v.is_null() {
        return Ok(Vec::new());
    }
    let Some(arr) = v.as_array() else {
        return Err(format!("{key} must be array (or null), got {v:?}"));
    };
    Ok(arr.clone())
}

fn clamp_max_rounds(raw: u64) -> (u64, bool) {
    let cap = crate::router_env_flags::router_rs_rfv_max_rounds_cap();
    if raw > cap {
        (cap, true)
    } else {
        (raw, false)
    }
}

fn resolve_framework_rfv_loop_repo(payload: &Value) -> Result<PathBuf, String> {
    let repo_root = payload
        .get("repo_root")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "framework_rfv_loop requires repo_root".to_string())?;
    if !repo_root.is_dir() {
        return Err(format!(
            "framework_rfv_loop: repo_root is not a directory: {}",
            repo_root.display()
        ));
    }
    resolve_repo_root_arg(Some(repo_root.as_path()))
}

/// stdio：`framework_rfv_loop`
pub fn framework_rfv_loop(payload: Value) -> Result<Value, String> {
    let operation = payload
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    if operation == "status" {
        framework_rfv_loop_impl(payload)
    } else {
        let resolved = resolve_framework_rfv_loop_repo(&payload)?;
        crate::task_write_lock::apply_task_ledger_mutation(&resolved, || {
            framework_rfv_loop_impl(payload)
        })
    }
}

fn framework_rfv_loop_impl(payload: Value) -> Result<Value, String> {
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
                rfv_loop_state_path(&repo_root, &tid).unwrap_or_else(|_| PathBuf::new())
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
            crate::path_guard::validate_task_id_component(&task_id)?;
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
            // When external research is allowed, default structured blob preference to true so
            // strict-mode validators and struct-hint nudges align; explicit `false` still wins.
            // When `allow_external_research` is false, keep legacy default `false` unless the
            // caller explicitly sets a bool (tests / forward-compat).
            let prefer_structured_external = if allow_external {
                match payload.get("prefer_structured_external_research") {
                    None => true,
                    Some(v) if v.is_null() => true,
                    Some(v) => v.as_bool().unwrap_or(false),
                }
            } else {
                payload
                    .get("prefer_structured_external_research")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            };
            // NOTE: `external_research_strict` defaults to true (enforce structured ER quality),
            // while `prefer_structured_external_research` defaults to false (loose recommendation).
            // These have different design intents despite similar naming.
            // See: docs/references/rfv-loop/reasoning-depth-contract.md
            let external_research_strict = payload
                .get("external_research_strict")
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
                "prefer_structured_external_research".to_string(),
                json!(prefer_structured_external),
            );
            obj.insert(
                "external_research_strict".to_string(),
                json!(external_research_strict),
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
            if let Some(cg) = payload.get("close_gates") {
                if !cg.is_null() {
                    obj.insert("close_gates".to_string(), cg.clone());
                }
            }

            let path = rfv_loop_state_path(&repo_root, &task_id)?;
            let value = Value::Object(obj);
            write_atomic_json(&path, &value)?;
            let tx = crate::task_ledger::LedgerTransaction {
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                tx_type: "rfv_loop_state".to_string(),
                payload: value.clone(),
                idempotency_key: None,
                seq: None,
                schema_version: Some(1),
            };
            if let Err(e) = crate::task_ledger::append_transaction_assuming_l1_held(&repo_root, &task_id, tx) {
                eprintln!("[router-rs] failed to append rfv transaction to TASK_LEDGER: {e}");
            }
            let goal_state_cleared =
                crate::autopilot_goal::deactivate_goal_for_conflict_with_rfv(&repo_root, &task_id)?;
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
            Ok(json!({
                "ok": true,
                "operation": "start",
                "task_id": task_id,
                "rfv_loop_state_path": path.display().to_string(),
                "rfv_loop_state": value,
                "goal_state_cleared": goal_state_cleared,
                "warning": if capped {
                    Some(format!(
                        "max_rounds requested {requested_max} exceeds hard cap {}; stored max_rounds={max_rounds}",
                        crate::router_env_flags::router_rs_rfv_max_rounds_cap()
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
            crate::path_guard::validate_task_id_component(&task_id)?;
            let path = rfv_loop_state_path(&repo_root, &task_id)?;
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
                .unwrap_or(crate::router_env_flags::router_rs_rfv_max_rounds_cap());
            if round_n > max_rounds {
                return Err(format!("round {round_n} exceeds max_rounds {max_rounds}"));
            }

            let close_gates_cfg = parse_close_gates(obj);

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
            let raw_verify = payload
                .get("verify_result")
                .and_then(Value::as_str)
                .unwrap_or("UNKNOWN");
            let verify_result = normalize_verify_result(raw_verify)?;
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

            // Optional "adversarial depth" fields: stored as-is (array) for audit; no new state machine.
            // Shapes are minimally validated here so later rollups can trust arrays.
            let adversarial_findings = value_array_or_empty(&payload, "adversarial_findings")?;
            let falsification_tests = value_array_or_empty(&payload, "falsification_tests")?;

            let external_research_strict = external_research_strict_from_loaded_state(obj);
            if let Some(er) = payload.get("external_research") {
                if !er.is_null() {
                    validate_external_research_structured(er)?;
                    if external_research_strict {
                        validate_external_research_strict(er)?;
                    }
                }
            }

            // Cross-link this round's verify claim against EVIDENCE_INDEX successful rows
            // recorded since the previous round (audit trail; not a hard block — supervisor
            // still owns the call, but the discrepancy lands in `cross_check`).
            let (evidence_refs, cross_check_label) =
                cross_link_evidence(&repo_root, &task_id, obj, &verify_result);

            let mut entry_map = serde_json::Map::new();
            entry_map.insert("round".to_string(), json!(round_n));
            entry_map.insert("review_summary".to_string(), json!(review_summary));
            entry_map.insert(
                "external_research_summary".to_string(),
                json!(external_research_summary),
            );
            entry_map.insert("fix_summary".to_string(), json!(fix_summary));
            entry_map.insert("verify_result".to_string(), json!(verify_result));
            entry_map.insert(
                "supervisor_decision".to_string(),
                json!(supervisor_decision),
            );
            entry_map.insert("reason".to_string(), json!(reason));
            entry_map.insert("at".to_string(), json!(now_iso()));
            entry_map.insert("evidence_refs".to_string(), Value::Array(evidence_refs));
            if !adversarial_findings.is_empty() {
                entry_map.insert(
                    "adversarial_findings".to_string(),
                    Value::Array(adversarial_findings),
                );
            }
            if !falsification_tests.is_empty() {
                entry_map.insert(
                    "falsification_tests".to_string(),
                    Value::Array(falsification_tests),
                );
            }
            if let Some(label) = cross_check_label {
                entry_map.insert("cross_check".to_string(), json!(label));
            }
            if let Some(er) = payload.get("external_research") {
                if !er.is_null() {
                    entry_map.insert("external_research".to_string(), er.clone());
                }
            }
            let entry = Value::Object(entry_map);

            let supervisor_closes = matches!(supervisor_decision.as_str(), "close" | "closed");
            if supervisor_closes {
                if let Some(ref g) = close_gates_cfg {
                    let mut preview_map = obj.clone();
                    {
                        let pr = preview_map
                            .get_mut("rounds")
                            .and_then(|r| r.as_array_mut())
                            .ok_or_else(|| "RFV_LOOP_STATE.rounds missing".to_string())?;
                        pr.push(entry.clone());
                    }
                    let closing = preview_map
                        .get("rounds")
                        .and_then(|r| r.as_array())
                        .and_then(|a| a.last())
                        .and_then(|v| v.as_object())
                        .ok_or_else(|| {
                            "RFV close_gates: internal error resolving closing round".to_string()
                        })?;
                    enforce_rfv_close_gates(&repo_root, &task_id, &preview_map, closing, g)?;
                }
            }

            let closes_due_to_round_cap = !supervisor_closes
                && !matches!(supervisor_decision.as_str(), "block" | "blocked")
                && round_n >= max_rounds;
            if closes_due_to_round_cap {
                if let Some(ref g) = close_gates_cfg {
                    let mut preview_map = obj.clone();
                    {
                        let pr = preview_map
                            .get_mut("rounds")
                            .and_then(|r| r.as_array_mut())
                            .ok_or_else(|| "RFV_LOOP_STATE.rounds missing".to_string())?;
                        pr.push(entry.clone());
                    }
                    let closing = preview_map
                        .get("rounds")
                        .and_then(|r| r.as_array())
                        .and_then(|a| a.last())
                        .and_then(|v| v.as_object())
                        .ok_or_else(|| {
                            "RFV close_gates: internal error resolving closing round (max_rounds)"
                                .to_string()
                        })?;
                    enforce_rfv_close_gates(&repo_root, &task_id, &preview_map, closing, g)?;
                }
            }

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
            let tx = crate::task_ledger::LedgerTransaction {
                ts: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                tx_type: "rfv_loop_state".to_string(),
                payload: state.clone(),
                idempotency_key: None,
                seq: None,
                schema_version: Some(1),
            };
            if let Err(e) = crate::task_ledger::append_transaction_assuming_l1_held(&repo_root, &task_id, tx) {
                eprintln!("[router-rs] failed to append rfv transaction to TASK_LEDGER: {e}");
            }
            crate::task_state_aggregate::sync_task_state_aggregate_best_effort(
                &repo_root, &task_id,
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rfv_start_append_roundtrip() {
        let _nudge_env = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rfv-task")).expect("mkdir");
        let skill_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let nudge_src = skill_root.join("configs/framework/HARNESS_OPERATOR_NUDGES.json");
        fs::create_dir_all(repo.join("configs/framework")).expect("nudge dir");
        fs::copy(
            &nudge_src,
            repo.join("configs/framework/HARNESS_OPERATOR_NUDGES.json"),
        )
        .expect("copy harness nudges fixture");
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
            "adversarial_findings": [
                {"id":"A1","hypothesis":"panic on empty input","severity":"high"}
            ],
            "falsification_tests": [
                {"id":"T1","command":"cargo test -q","expect":"pass"}
            ],
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
        assert_eq!(gs["external_research_strict"], json!(true));
        assert_eq!(gs["current_round"], json!(1));
        assert_eq!(gs["loop_status"], json!("active"));
        let rounds = gs["rounds"].as_array().expect("rounds");
        let r1 = rounds[0].as_object().expect("round1 obj");
        assert!(r1.get("adversarial_findings").is_some());
        assert!(r1.get("falsification_tests").is_some());
        let _via_api = read_rfv_loop_state(&repo, None)
            .expect("read api")
            .expect("state");

        let _ = fs::remove_dir_all(&repo);
    }

    /// P0-A: invalid `verify_result` is rejected (not silently coerced).
    #[test]
    fn append_round_rejects_unknown_verify_result() {
        let _nudge_env = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-vr-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/t-vr")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-vr"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-vr",
            "goal": "verify enum",
            "max_rounds": 5u64,
        }))
        .expect("start");
        let err = framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "append_round",
            "round": 1u64,
            "verify_result": "kinda passed",
        }))
        .expect_err("invalid verify_result must error");
        assert!(
            err.contains("verify_result must be one of"),
            "unexpected error: {err}"
        );
        let _ = fs::remove_dir_all(&repo);
    }

    /// P1-B: PASS round with no successful EVIDENCE_INDEX rows surfaces `cross_check=no_evidence_window`.
    #[test]
    fn append_round_marks_pass_without_evidence() {
        let _nudge_env = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-cl-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        let task_dir = repo.join("artifacts/current/t-cl");
        fs::create_dir_all(&task_dir).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-cl"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-cl",
            "goal": "cross-link",
            "max_rounds": 3u64,
        }))
        .expect("start");
        // No EVIDENCE_INDEX yet → PASS should land with no_evidence_window.
        let out = framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "verify_result": "PASS",
        }))
        .expect("append");
        let rounds = out["rfv_loop_state"]["rounds"]
            .as_array()
            .expect("rounds array");
        let r1 = &rounds[0];
        assert_eq!(r1["cross_check"], json!("no_evidence_window"));
        assert!(r1["evidence_refs"].as_array().expect("refs").is_empty());

        // Now write a successful EVIDENCE row newer than the round timestamp and append round 2.
        // Use a timestamp far in the future so it deterministically beats round 1's `at`.
        fs::write(
            task_dir.join("EVIDENCE_INDEX.json"),
            r#"{"schema_version":"evidence-index-v2","artifacts":[{"recorded_at":"2099-12-31T23:59:59Z","exit_code":0,"success":true}]}"#,
        )
        .expect("evidence");
        let out2 = framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "append_round",
            "round": 2u64,
            "verify_result": "PASS",
        }))
        .expect("append 2");
        let rounds2 = out2["rfv_loop_state"]["rounds"]
            .as_array()
            .expect("rounds 2");
        let r2 = &rounds2[1];
        assert!(
            r2.get("cross_check").is_none(),
            "expected cross_check absent on PASS-with-evidence; round={r2}"
        );
        assert!(
            !r2["evidence_refs"].as_array().expect("refs2").is_empty(),
            "expected non-empty evidence_refs; round={r2}"
        );
        let _ = fs::remove_dir_all(&repo);
    }

    /// RFV 与 GOAL 同 task 互斥：RFV start 应删除已存在的 GOAL_STATE。
    #[test]
    fn rfv_start_clears_goal_same_task() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-goal-rfv-mutex-rfv-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/rfv-mx")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"rfv-mx"}"#,
        )
        .expect("pointer");
        let rr = repo.display().to_string();

        crate::autopilot_goal::framework_goal_drive(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "rfv-mx",
            "goal": "macro first",
            "non_goals": ["n"],
            "done_when": ["d1", "d2"],
            "validation_commands": ["cargo test -q"],
            "drive_until_done": true,
        }))
        .expect("goal start");
        let gpath =
            crate::autopilot_goal::goal_state_path_for_task(&repo, "rfv-mx").expect("gpath");
        assert!(gpath.is_file());

        let out = framework_rfv_loop(json!({
            "repo_root": rr,
            "operation": "start",
            "task_id": "rfv-mx",
            "goal": "rfv mode",
            "max_rounds": 2u64,
        }))
        .expect("rfv start");
        assert_eq!(out["goal_state_cleared"], json!(true));
        assert!(!gpath.is_file());

        let _ = fs::remove_dir_all(&repo);
    }

    fn minimal_external_research_loose_only() -> Value {
        json!({
            "claims": [{"claim": "c1", "sources": ["https://a.example/foo"]}],
            "contradiction_sweep": [{
                "related_claim_or_topic": "t1",
                "contradicting_or_limiting_evidence": "e1",
                "sources": ["https://contradicts.example/bar"],
            }],
            "retrieval_trace": {
                "queries_used": ["duckdb reproducibility"],
                "inclusion_rules": "official docs first",
                "exclusions": "forum posts without primary cites",
                "exclusion_rationale": "noise",
            }
        })
    }

    /// Satisfies both [`validate_external_research_structured`] and [`validate_external_research_strict`].
    fn minimal_external_research() -> Value {
        let t40 = "0123456789012345678901234567890123456789";
        json!({
            "claims": [{
                "claim": "c1",
                "sources": [
                    "https://a.example/foo",
                    "doi:10.1000/182"
                ]
            }],
            "contradiction_sweep": [
                {
                    "related_claim_or_topic": "t1",
                    "contradicting_or_limiting_evidence": "e1",
                    "sources": ["https://contradicts.example/bar"]
                },
                {
                    "related_claim_or_topic": "t2",
                    "contradicting_or_limiting_evidence": "e2",
                    "sources": ["arxiv:2301.00001v1"]
                }
            ],
            "unknowns": [],
            "retrieval_trace": {
                "queries_used": ["q1 scope literature", "q2 methods survey", "q3 risk edge cases"],
                "inclusion_rules": t40,
                "exclusions": t40,
                "exclusion_rationale": t40
            }
        })
    }

    #[test]
    fn source_traceable_heuristic_matrix() {
        assert!(source_traceable_heuristic("  https://x/y "));
        assert!(source_traceable_heuristic("HTTP://LOCALHOST/z"));
        assert!(source_traceable_heuristic("doi:10.1000/182"));
        assert!(source_traceable_heuristic("DOI:10.9999/zenodo.123"));
        assert!(source_traceable_heuristic("10.5281/zenodo.12345"));
        assert!(source_traceable_heuristic("ArXiv:2301.00001"));
        assert!(source_traceable_heuristic("PMID:12345678"));
        assert!(source_traceable_heuristic("ISBN:978-3-16-148410-0"));
        assert!(source_traceable_heuristic("dataset:gov.example/series/v1"));
        assert!(source_traceable_heuristic("official_doc:eu-reg-2024/001"));
        // New prefixes
        assert!(source_traceable_heuristic("huggingface:transformers"));
        assert!(source_traceable_heuristic("HF:bert-base"));
        assert!(source_traceable_heuristic("github:anthropic/claude-code"));
        assert!(source_traceable_heuristic("kaggle:datasets/imagenet"));
        assert!(source_traceable_heuristic(
            "geojson:https://example.com/data.geojson"
        ));

        assert!(!source_traceable_heuristic(""));
        assert!(!source_traceable_heuristic("random blog title"));
        assert!(!source_traceable_heuristic("ftp://files.example/a"));
        assert!(!source_traceable_heuristic("doi:9.1234/nope"));
        assert!(!source_traceable_heuristic("10.1234"));
    }

    #[test]
    fn validate_external_research_strict_matrix() {
        validate_external_research_strict(&minimal_external_research()).expect("strict minimal");

        let err = validate_external_research_strict(&minimal_external_research_loose_only())
            .expect_err("loose missing unknowns");
        assert!(err.contains("missing `unknowns` key"), "unexpected: {err}");

        let mut one_src = minimal_external_research();
        one_src.as_object_mut().unwrap().insert(
            "claims".to_string(),
            json!([{"claim":"x","sources":["https://a.example/only-one"]}]),
        );
        let err = validate_external_research_strict(&one_src).expect_err("single-source claim");
        assert!(err.contains("`sources` must have at least 2"), "{err}");

        let mut bad_sweep = minimal_external_research();
        bad_sweep.as_object_mut().unwrap().insert(
            "contradiction_sweep".to_string(),
            json!([{
                "related_claim_or_topic":"t",
                "contradicting_or_limiting_evidence":"e",
                "sources":["https://x"]
            }]),
        );
        let err = validate_external_research_strict(&bad_sweep).expect_err("short sweep");
        assert!(
            err.contains("contradiction_sweep must have at least"),
            "{err}"
        );

        let mut q2 = minimal_external_research();
        let tr = q2
            .as_object_mut()
            .unwrap()
            .get_mut("retrieval_trace")
            .unwrap()
            .as_object_mut()
            .unwrap();
        tr.insert("queries_used".to_string(), json!(["a", "b"]));
        let err = validate_external_research_strict(&q2).expect_err("queries");
        assert!(err.contains("queries_used must have at least 3"), "{err}");

        let mut short_trace = minimal_external_research();
        let tr = short_trace
            .as_object_mut()
            .unwrap()
            .get_mut("retrieval_trace")
            .unwrap()
            .as_object_mut()
            .unwrap();
        tr.insert("inclusion_rules".to_string(), json!("short"));
        let err = validate_external_research_strict(&short_trace).expect_err("trace len");
        assert!(
            err.contains("inclusion_rules") && err.contains("at least 40"),
            "{err}"
        );

        let mut bad_unk = minimal_external_research();
        bad_unk
            .as_object_mut()
            .unwrap()
            .insert("unknowns".to_string(), json!("nope"));
        let err = validate_external_research_strict(&bad_unk).expect_err("unknowns type");
        assert!(err.contains("unknowns` must be array or null"), "{err}");
    }

    #[test]
    fn append_round_strict_rejects_without_round_write() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-er-strict-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/t-er-st")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-er-st"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-er-st",
            "goal": "strict ext",
            "max_rounds": 3u64,
        }))
        .expect("start");

        let err = framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "external_research": minimal_external_research_loose_only(),
            "verify_result": "PASS",
        }))
        .expect_err("strict rejects loose blob");
        assert!(
            err.contains("external_research strict"),
            "unexpected err: {err}"
        );
        let st = framework_rfv_loop(json!({"repo_root": rr.clone(), "operation": "status"}))
            .expect("st");
        assert!(st["rfv_loop_state"]["rounds"]
            .as_array()
            .expect("rounds")
            .is_empty());

        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "external_research": minimal_external_research(),
            "verify_result": "PASS",
        }))
        .expect("strict ok append");

        let st2 = framework_rfv_loop(json!({"repo_root": rr, "operation": "status"})).expect("st2");
        assert_eq!(st2["rfv_loop_state"]["rounds"].as_array().unwrap().len(), 1);
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn append_round_legacy_missing_strict_flag_accepts_loose_blob() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-legacy-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/t-leg")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-leg"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-leg",
            "goal": "legacy",
            "max_rounds": 3u64,
        }))
        .expect("start");

        let path = rfv_loop_state_path(&repo, "t-leg").expect("path");
        let mut v: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
        v.as_object_mut()
            .expect("obj")
            .remove("external_research_strict");
        write_atomic_json(&path, &v).expect("rewrite");

        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "external_research": minimal_external_research_loose_only(),
            "verify_result": "PASS",
        }))
        .expect("legacy append with loose blob");

        let st = framework_rfv_loop(json!({"repo_root": rr, "operation": "status"})).expect("st");
        assert_eq!(st["rfv_loop_state"]["rounds"].as_array().unwrap().len(), 1);
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn append_round_respects_explicit_external_research_strict_false() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-loose-flag-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/t-loose")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-loose"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-loose",
            "goal": "loose",
            "max_rounds": 3u64,
            "external_research_strict": false,
        }))
        .expect("start");

        let st = framework_rfv_loop(json!({"repo_root": rr.clone(), "operation": "status"}))
            .expect("st");
        assert_eq!(
            st["rfv_loop_state"]["external_research_strict"],
            json!(false)
        );

        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "external_research": minimal_external_research_loose_only(),
            "verify_result": "PASS",
        }))
        .expect("append loose");

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn validate_external_research_struct_matrix() {
        validate_external_research_structured(&minimal_external_research()).expect("minimal ok");

        validate_external_research_structured(&json!({})).expect_err("empty root must reject");
        validate_external_research_structured(&json!({"claims": [], "contradiction_sweep": [], "retrieval_trace": {"queries_used":[], "inclusion_rules":"a","exclusions":"b","exclusion_rationale":"c"}}))
            .expect_err("empty arrays");

        validate_external_research_structured(&json!({
            "claims": [{"claim":"","sources":["u"]}],
            "contradiction_sweep": [{"related_claim_or_topic":"a","contradicting_or_limiting_evidence":"b","sources":["s"]}],
            "retrieval_trace": {"queries_used":["q"],"inclusion_rules":"i","exclusions":"x","exclusion_rationale":"r"}
        })).expect_err("empty claim trim");

        let mut with_unknown = minimal_external_research();
        let uo = with_unknown.as_object_mut().unwrap();
        uo.insert(
            "unknowns".to_string(),
            json!([
                {"question": "pq", "why_insufficient": "no data"}
            ]),
        );
        validate_external_research_structured(&with_unknown).expect("unknowns optional");

        let mut qr_none = minimal_external_research();
        qr_none
            .as_object_mut()
            .unwrap()
            .insert("quantitative_replays".to_string(), json!("NONE"));
        validate_external_research_structured(&qr_none).expect("uppercase NONE");

        let mut qr_arr = minimal_external_research();
        qr_arr.as_object_mut().unwrap().insert(
            "quantitative_replays".to_string(),
            json!([{
                "dataset_or_source_id": "d",
                "version_or_snapshot": "v",
                "window": "2020-2025",
                "replay_command": "python - <<'PY'\nPY",
            }]),
        );
        validate_external_research_structured(&qr_arr).expect("quant array");
    }

    #[test]
    fn append_round_rejects_bad_external_research_without_write() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-er-bad-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/t-er-bad")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-er-bad"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-er-bad",
            "goal": "structured ext",
            "max_rounds": 3u64,
        }))
        .expect("start");

        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "external_research": {"claims":[],"contradiction_sweep":[],"retrieval_trace":{}},
            "verify_result": "PASS",
        }))
        .expect_err("invalid external payload");

        let st = framework_rfv_loop(json!({"repo_root": rr, "operation": "status"})).expect("st");
        let rounds = st["rfv_loop_state"]["rounds"].as_array().expect("arr");
        assert!(rounds.is_empty(), "rounds unchanged on validation failure");
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn append_round_persists_valid_external_research() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-er-good-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/t-er-good")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"t-er-good"}"#,
        )
        .expect("active");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "t-er-good",
            "goal": "x",
            "max_rounds": 3u64,
        }))
        .expect("start");

        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "external_research": minimal_external_research(),
            "verify_result": "PASS",
        }))
        .expect("append ok");

        let rounds = framework_rfv_loop(json!({"repo_root": rr, "operation": "status"}))
            .expect("st")["rfv_loop_state"]["rounds"]
            .as_array()
            .expect("rounds")
            .clone();
        assert_eq!(rounds.len(), 1);
        let er = rounds[0]
            .get("external_research")
            .expect("external_research");
        assert!(
            validate_external_research_structured(er).is_ok(),
            "stored blob should re-validate",
        );
        assert!(
            validate_external_research_strict(er).is_ok(),
            "stored blob should satisfy strict when task default strict",
        );

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn rfv_start_writes_prefer_structured_flag() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-pref-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/pref-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"pref-task"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();

        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "pref-task",
            "goal": "g",
            "max_rounds": 2u64,
            "prefer_structured_external_research": true,
        }))
        .expect("start");

        let st =
            framework_rfv_loop(json!({"repo_root": rr, "operation": "status"})).expect("status");
        assert_eq!(
            st["rfv_loop_state"]["prefer_structured_external_research"],
            json!(true)
        );
        assert_eq!(
            st["rfv_loop_state"]["external_research_strict"],
            json!(true)
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn rfv_start_defaults_prefer_structured_when_allow_external() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-prefdef-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/extdef")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"extdef"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "extdef",
            "goal": "g",
            "max_rounds": 2u64,
            "allow_external_research": true,
        }))
        .expect("start");
        let st =
            framework_rfv_loop(json!({"repo_root": rr, "operation": "status"})).expect("status");
        assert_eq!(
            st["rfv_loop_state"]["prefer_structured_external_research"],
            json!(true)
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn append_round_close_gates_reject_skipped_when_require_pass() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-closegate-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/cg-task")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"cg-task"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "cg-task",
            "goal": "g",
            "max_rounds": 3u64,
            "close_gates": {
                "enabled": true,
                "require_last_round_verify_pass": true
            }
        }))
        .expect("start");
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "review_summary": "r",
            "fix_summary": "f",
            "verify_result": "PASS",
            "supervisor_decision": "continue",
        }))
        .expect("r1");
        let err = framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 2u64,
            "review_summary": "r",
            "fix_summary": "f",
            "verify_result": "SKIPPED",
            "supervisor_decision": "close",
        }))
        .expect_err("close with SKIPPED should fail gates");
        assert!(
            err.contains("close_gates") && err.contains("verify_result"),
            "err={err}"
        );
        let st =
            framework_rfv_loop(json!({"repo_root": rr, "operation": "status"})).expect("status");
        assert_eq!(
            st["rfv_loop_state"]["rounds"].as_array().map(|a| a.len()),
            Some(1)
        );
        let _ = fs::remove_dir_all(&repo);
    }

    /// `close_gates` 在 **`max_rounds` 耗尽**（非显式 close）路径与显式 close 一致：仍校验收口轮。
    #[test]
    fn append_round_close_gates_enforced_on_max_rounds_cap() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-capgate-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/cap-g")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"cap-g"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "cap-g",
            "goal": "g",
            "max_rounds": 2u64,
            "close_gates": {
                "enabled": true,
                "require_last_round_verify_pass": true
            }
        }))
        .expect("start");
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "verify_result": "PASS",
            "supervisor_decision": "continue",
        }))
        .expect("r1");
        let err = framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 2u64,
            "verify_result": "SKIPPED",
            "supervisor_decision": "continue",
        }))
        .expect_err("max_rounds cap close must still enforce verify_pass gate");
        assert!(
            err.contains("close_gates") && err.contains("verify_result"),
            "unexpected err: {err}"
        );
        let st =
            framework_rfv_loop(json!({"repo_root": rr, "operation": "status"})).expect("status");
        assert_eq!(
            st["rfv_loop_state"]["rounds"].as_array().map(|a| a.len()),
            Some(1)
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn append_round_max_rounds_cap_passes_close_gates_when_verify_pass() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("router-rs-rfv-capgate-ok-{suffix}"));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(repo.join("artifacts/current/cap-ok")).expect("mkdir");
        fs::write(
            repo.join("artifacts/current/active_task.json"),
            r#"{"task_id":"cap-ok"}"#,
        )
        .expect("ptr");
        let rr = repo.display().to_string();
        framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "start",
            "task_id": "cap-ok",
            "goal": "g",
            "max_rounds": 1u64,
            "close_gates": {
                "enabled": true,
                "require_last_round_verify_pass": true
            }
        }))
        .expect("start");
        let out = framework_rfv_loop(json!({
            "repo_root": rr.clone(),
            "operation": "append_round",
            "round": 1u64,
            "verify_result": "PASS",
            "supervisor_decision": "continue",
        }))
        .expect("single round hits cap");
        let gs = out["rfv_loop_state"].as_object().expect("obj");
        assert_eq!(gs.get("loop_status"), Some(&json!("closed")));
        assert_eq!(gs["rounds"].as_array().map(|a| a.len()), Some(1));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn parse_iso_datetime_accepts_rfc3339() {
        assert!(parse_iso_datetime("2024-01-01T12:00:00Z").is_some());
        assert!(parse_iso_datetime("2024-01-01T12:00:00+00:00").is_some());
        assert!(parse_iso_datetime("2024-01-01T12:00:00-05:00").is_some());
    }

    #[test]
    fn parse_iso_datetime_rejects_non_rfc3339() {
        // Non-RFC3339 formats are now rejected
        assert!(parse_iso_datetime("2024-01-01 12:00:00").is_none());
        assert!(parse_iso_datetime("2024-01-01T12:00:00").is_none());
        assert!(parse_iso_datetime("2024/01/01 12:00:00").is_none());
    }
}
