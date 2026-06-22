//! Verification Gate — independent cross-validation of closeout claims.
//!
//! R19: commands_run ↔ EVIDENCE_INDEX cross-check
//! R20: fs::metadata independent file existence check on changed_files
//! R21: source=model_manual warning
//! Completion keyword detection on summary/claims
//!
//! This gate operates on the EVIDENCE_INDEX and closeout record artifacts
//! to provide an independent verification layer before ship/closeout.

use serde_json::{Value, json};
use std::fs;
use std::path::Path;

// ────────────────────────────────────────────────────────────────
// Result types
// ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct VerificationGateFinding {
    pub rule: String,
    pub severity: String, // "warn" | "info"
    pub detail: String,
}

impl VerificationGateFinding {
    pub fn warn(rule: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            severity: "warn".into(),
            detail: detail.into(),
        }
    }
    pub fn info(rule: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            severity: "info".into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationGateResult {
    pub passed: bool,
    pub findings: Vec<VerificationGateFinding>,
    pub evidence_rows_count: usize,
    pub has_completion_keyword: bool,
}

// ────────────────────────────────────────────────────────────────
// EVIDENCE_INDEX reading
// ────────────────────────────────────────────────────────────────

fn read_evidence_index(repo_root: &Path, task_id: &str) -> Vec<Value> {
    let tid = match core_state::utils::path_guard::validate_task_id_component(task_id) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let path = repo_root
        .join("artifacts/current")
        .join(tid)
        .join("EVIDENCE_INDEX.json");
    if !path.is_file() {
        return Vec::new();
    }
    let raw = match fs::read_to_string(&path) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let val: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    val.get("artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

// ────────────────────────────────────────────────────────────────
// R19: commands_run ↔ EVIDENCE_INDEX cross-check
// ────────────────────────────────────────────────────────────────

/// Check that each command in `commands_run` has a corresponding entry
/// in EVIDENCE_INDEX. Matching is prefix-based on `command_preview`.
fn check_r19_cross_validation(
    commands_run: &[Value],
    evidence_rows: &[Value],
) -> Vec<VerificationGateFinding> {
    if commands_run.is_empty() {
        return Vec::new();
    }
    let mut findings = Vec::new();
    let evidence_cmds: Vec<&str> = evidence_rows
        .iter()
        .filter_map(|row| {
            row.get("command_preview")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .collect();

    for cmd_val in commands_run {
        let cmd_text = cmd_val
            .get("command")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(cmd) = cmd_text else {
            continue;
        };
        // Check if any evidence row's command_preview contains or is contained by this command
        let has_match = evidence_cmds.iter().any(|ev_cmd| {
            ev_cmd.contains(cmd)
                || cmd.contains(ev_cmd)
                || prefix_match_60pct(cmd, ev_cmd)
        });
        if !has_match {
            findings.push(VerificationGateFinding::warn(
                "r19_command_no_evidence",
                format!("commands_run entry `{}` has no matching EVIDENCE_INDEX row", truncate(cmd, 120)),
            ));
        }
    }
    findings
}

/// Simple prefix match: at least 60% of the shorter string matches the start of the longer.
fn prefix_match_60pct(a: &str, b: &str) -> bool {
    let min_len = a.len().min(b.len());
    if min_len == 0 {
        return false;
    }
    let threshold = (min_len * 60) / 100;
    if threshold == 0 {
        return false;
    }
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut matched = 0usize;
    for i in 0..min_len {
        if a_bytes[i] == b_bytes[i] {
            matched += 1;
        } else {
            break;
        }
    }
    matched > threshold // strictly greater than
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

// ────────────────────────────────────────────────────────────────
// R20: fs::metadata independent file existence check
// ────────────────────────────────────────────────────────────────

fn check_r20_file_existence(
    repo_root: &Path,
    changed_files: &[String],
) -> Vec<VerificationGateFinding> {
    if changed_files.is_empty() {
        return Vec::new();
    }
    let mut findings = Vec::new();
    for file_path in changed_files {
        let p = if Path::new(file_path).is_absolute() {
            std::path::PathBuf::from(file_path)
        } else {
            repo_root.join(file_path)
        };
        match fs::metadata(&p) {
            Ok(_) => {}
            Err(e) => {
                findings.push(VerificationGateFinding::warn(
                    "r20_changed_file_missing",
                    format!("changed_file `{}` metadata check failed: {e}", truncate(file_path, 200)),
                ));
            }
        }
    }
    findings
}

// ────────────────────────────────────────────────────────────────
// R21: source=model_manual warning
// ────────────────────────────────────────────────────────────────

fn check_r21_model_manual_source(evidence_rows: &[Value]) -> Vec<VerificationGateFinding> {
    let mut findings = Vec::new();
    for row in evidence_rows {
        let source = row.get("source").and_then(Value::as_str).unwrap_or("");
        if source == "model_manual" {
            let cmd = row
                .get("command_preview")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            findings.push(VerificationGateFinding::warn(
                "r21_model_manual_evidence",
                format!("evidence row source=model_manual: `{}`", truncate(cmd, 120)),
            ));
        }
    }
    findings
}

// ────────────────────────────────────────────────────────────────
// Completion keyword detection
// ────────────────────────────────────────────────────────────────

const COMPLETION_KEYWORDS_EN: &[&str] = &[
    "done",
    "finished",
    "completed",
    "succeeded",
    "passed",
    "all done",
    "task complete",
    "work complete",
    "implementation complete",
    "fix applied",
    "changes applied",
    "ready to ship",
    "all tests pass",
    "verified",
    "done with",
];

const COMPLETION_KEYWORDS_ZH: &[&str] = &[
    "已完成",
    "已经完成",
    "全部完成",
    "完成了",
    "搞定",
    "全部搞定",
    "修好了",
    "改完了",
    "做完了",
    "测试通过",
    "验证通过",
];

/// Detect completion keywords in the given text. Returns true if any keyword
/// is found as a whole-word/phrase match (case-insensitive for English).
fn has_completion_keyword(text: &str) -> bool {
    let lower = text.to_lowercase();
    for kw in COMPLETION_KEYWORDS_EN {
        if lower.contains(kw) {
            return true;
        }
    }
    for kw in COMPLETION_KEYWORDS_ZH {
        if text.contains(kw) {
            return true;
        }
    }
    false
}

// ────────────────────────────────────────────────────────────────
// Public API
// ────────────────────────────────────────────────────────────────

/// Evaluate the verification gate for a given task.
///
/// Reads EVIDENCE_INDEX.json from disk and applies R19/R20/R21 checks.
/// Optionally cross-checks against a closeout record if `closeout_record` is provided.
pub fn evaluate_verification_gate(
    repo_root: &Path,
    task_id: &str,
) -> VerificationGateResult {
    evaluate_verification_gate_with_closeout(repo_root, task_id, None)
}

/// Evaluate the verification gate with an optional closeout record for R19 cross-checking.
pub fn evaluate_verification_gate_with_closeout(
    repo_root: &Path,
    task_id: &str,
    closeout_record: Option<&Value>,
) -> VerificationGateResult {
    let evidence_rows = read_evidence_index(repo_root, task_id);
    let mut findings: Vec<VerificationGateFinding> = Vec::new();

    // R19: commands_run ↔ EVIDENCE_INDEX cross-check
    if let Some(record) = closeout_record {
        let commands_run = record
            .get("commands_run")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        findings.extend(check_r19_cross_validation(&commands_run, &evidence_rows));
    }

    // R20: fs::metadata check on changed_files
    let changed_files: Vec<String> = if let Some(record) = closeout_record {
        record
            .get("changed_files")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    findings.extend(check_r20_file_existence(repo_root, &changed_files));

    // R21: source=model_manual warning
    findings.extend(check_r21_model_manual_source(&evidence_rows));

    // Completion keyword detection
    let mut keyword_text = String::new();
    if let Some(record) = closeout_record {
        if let Some(summary) = record.get("summary").and_then(Value::as_str) {
            keyword_text.push_str(summary);
            keyword_text.push(' ');
        }
        if let Some(claims) = record.get("notes").and_then(Value::as_str) {
            keyword_text.push_str(claims);
        }
    }
    let has_completion_keyword = has_completion_keyword(&keyword_text);

    let has_warn = findings.iter().any(|f| f.severity == "warn");

    VerificationGateResult {
        passed: !has_warn,
        findings,
        evidence_rows_count: evidence_rows.len(),
        has_completion_keyword,
    }
}

// ────────────────────────────────────────────────────────────────
// EVIDENCE_INDEX bridge: write quality_gate_closed entry
// ────────────────────────────────────────────────────────────────

/// Append a quality_gate_closed entry to EVIDENCE_INDEX.json.
/// Called when a Quality Gate transitions to closed state.
///
/// This function directly writes to EVIDENCE_INDEX.json (matching the schema
/// used by `append_evidence_index_merged_row` in runtime-core).
pub fn append_quality_gate_evidence(
    repo_root: &Path,
    task_id: &str,
    goal_text: &str,
    rounds_count: usize,
) -> Result<(), String> {
    let tid = core_state::utils::path_guard::validate_task_id_component(task_id)
        .map_err(|e| format!("append_quality_gate_evidence: invalid task_id: {e}"))?;
    let evidence_path = repo_root
        .join("artifacts/current")
        .join(&tid)
        .join("EVIDENCE_INDEX.json");

    if let Some(parent) = evidence_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create evidence dir: {err}"))?;
    }

    let mut entry = serde_json::Map::new();
    entry.insert(
        "tool_name".to_string(),
        json!("framework_quality_gate_close"),
    );
    entry.insert(
        "command_preview".to_string(),
        json!(format!("quality_gate_closed: {goal_text}")),
    );
    entry.insert("source".to_string(), json!("quality_gate"));
    entry.insert("success".to_string(), json!(true));
    entry.insert("exit_code".to_string(), json!(0));
    entry.insert(
        "output_preview".to_string(),
        json!(format!(
            "Quality gate closed after {rounds_count} round(s). Goal: {goal_text}"
        )),
    );
    entry.insert(
        "recorded_at".to_string(),
        json!(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
    );
    entry.insert("evidence_type".to_string(), json!("quality_gate_closed"));

    // Read existing evidence index or create new one
    let mut val: Value = if evidence_path.is_file() {
        let raw = fs::read_to_string(&evidence_path)
            .map_err(|err| format!("read EVIDENCE_INDEX: {err}"))?;
        serde_json::from_str(&raw).unwrap_or_else(|_| json!({"schema_version": "router-rs-evidence-index-v1", "artifacts": []}))
    } else {
        json!({"schema_version": "router-rs-evidence-index-v1", "artifacts": []})
    };

    let artifacts = val
        .as_object_mut()
        .ok_or_else(|| "EVIDENCE_INDEX root not object".to_string())?
        .entry("artifacts")
        .or_insert_with(|| json!([]));
    let arr = artifacts
        .as_array_mut()
        .ok_or_else(|| "artifacts not array".to_string())?;

    // Dedup: skip if same source + evidence_type already present
    let already = arr.iter().any(|row| {
        row.get("source").and_then(Value::as_str) == Some("quality_gate")
            && row.get("evidence_type").and_then(Value::as_str) == Some("quality_gate_closed")
            && row.get("command_preview").and_then(Value::as_str).map(|s| s.contains(goal_text)).unwrap_or(false)
    });
    if !already {
        arr.push(Value::Object(entry));
    }

    let serialized = serde_json::to_string_pretty(&val)
        .map_err(|err| format!("serialize EVIDENCE_INDEX: {err}"))?;
    fs::write(&evidence_path, serialized)
        .map_err(|err| format!("write EVIDENCE_INDEX: {err}"))?;
    Ok(())
}

// ────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_has_completion_keyword_en() {
        assert!(has_completion_keyword("Task is done"));
        assert!(has_completion_keyword("All tests passed"));
        assert!(has_completion_keyword("Implementation complete"));
        assert!(has_completion_keyword("ready to ship"));
        assert!(!has_completion_keyword("working on it"));
        assert!(!has_completion_keyword("in progress"));
    }

    #[test]
    fn test_has_completion_keyword_zh() {
        assert!(has_completion_keyword("任务已完成"));
        assert!(has_completion_keyword("全部搞定"));
        assert!(has_completion_keyword("测试通过"));
        assert!(has_completion_keyword("验证通过"));
        assert!(!has_completion_keyword("进行中"));
        assert!(!has_completion_keyword("尚未完成"));
    }

    #[test]
    fn test_r21_model_manual_warning() {
        let rows = vec![
            json!({"tool_name": "bash", "command_preview": "cargo test", "source": "model_manual"}),
            json!({"tool_name": "bash", "command_preview": "cargo build", "source": "auto"}),
        ];
        let findings = check_r21_model_manual_source(&rows);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "r21_model_manual_evidence");
        assert_eq!(findings[0].severity, "warn");
    }

    #[test]
    fn test_r19_cross_validation_match() {
        let commands = vec![json!({"command": "cargo test -p core-state"})];
        let evidence = vec![json!({"command_preview": "cargo test -p core-state -q"})];
        let findings = check_r19_cross_validation(&commands, &evidence);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_r19_cross_validation_no_match() {
        let commands = vec![json!({"command": "cargo clippy"})];
        let evidence = vec![json!({"command_preview": "cargo test"})];
        let findings = check_r19_cross_validation(&commands, &evidence);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "r19_command_no_evidence");
    }

    #[test]
    fn test_r19_empty_commands_no_findings() {
        let commands: Vec<Value> = vec![];
        let evidence = vec![json!({"command_preview": "cargo test"})];
        let findings = check_r19_cross_validation(&commands, &evidence);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_r20_existing_file_no_findings() {
        let tmp = std::env::temp_dir().join("vg_test_r20_exist");
        let _ = fs::create_dir_all(&tmp);
        let test_file = tmp.join("test.txt");
        let _ = fs::write(&test_file, "hello");
        let findings = check_r20_file_existence(&tmp, &["test.txt".to_string()]);
        assert!(findings.is_empty());
        let _ = fs::remove_file(&test_file);
        let _ = fs::remove_dir(&tmp);
    }

    #[test]
    fn test_r20_missing_file_warns() {
        let tmp = std::env::temp_dir().join("vg_test_r20_missing");
        let findings = check_r20_file_existence(&tmp, &["nonexistent_file.txt".to_string()]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "r20_changed_file_missing");
    }

    #[test]
    fn test_evaluate_verification_gate_no_closeout() {
        let tmp = std::env::temp_dir().join("vg_test_no_closeout");
        let _ = fs::create_dir_all(tmp.join("artifacts/current/test-task"));
        let result = evaluate_verification_gate(&tmp, "test-task");
        // No closeout = no R19/R20 findings; R21 depends on evidence which is absent
        assert!(result.findings.is_empty());
        assert!(!result.has_completion_keyword);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_evaluate_verification_gate_with_closeout() {
        let tmp = std::env::temp_dir().join("vg_test_with_closeout");
        let _ = fs::create_dir_all(tmp.join("artifacts/current/test-task"));

        let record = json!({
            "commands_run": [{"command": "cargo test"}],
            "changed_files": ["nonexistent.txt"],
            "summary": "Implementation complete",
            "notes": null
        });
        let result = evaluate_verification_gate_with_closeout(&tmp, "test-task", Some(&record));
        // Should have r19 (no evidence match) and r20 (missing file) findings
        assert!(result.has_completion_keyword);
        let has_r19 = result.findings.iter().any(|f| f.rule == "r19_command_no_evidence");
        let has_r20 = result.findings.iter().any(|f| f.rule == "r20_changed_file_missing");
        assert!(has_r19);
        assert!(has_r20);
        assert!(!result.passed);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_prefix_match_60pct() {
        assert!(prefix_match_60pct("cargo test -p core-state", "cargo test -p core-state -q"));
        assert!(!prefix_match_60pct("cargo clippy", "cargo test"));
    }
}
