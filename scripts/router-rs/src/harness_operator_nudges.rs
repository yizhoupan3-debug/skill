//! Operator-facing nudge lines for RFV / Autopilot hooks.
//!
//! Truth source: `configs/framework/HARNESS_OPERATOR_NUDGES.json` under repo root.
//! Disable all injected nudges: `ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` (same soft-off tokens as other `ROUTER_RS_*` defaults).

use crate::router_env_flags::{
    router_rs_env_enabled_default_true, router_rs_operator_inject_globally_enabled,
};
use serde::Deserialize;
use std::fs;
use std::path::Path;

const NUDGES_REL_PATH: &str = "configs/framework/HARNESS_OPERATOR_NUDGES.json";
const HARNESS_NUDGES_ENV: &str = "ROUTER_RS_HARNESS_OPERATOR_NUDGES";
const EXPECTED_SCHEMA_VERSION: &str = "harness-operator-nudges-v1";

/// Global enable for JSON-backed operator nudges. Default on; `0`/`false`/`off`/`no` disables injection.
///
/// P1-E: also OR-gated by `ROUTER_RS_OPERATOR_INJECT` (aggregate kill-switch). When the
/// aggregate flag is off, nudges are disabled regardless of the per-nudge env.
pub fn harness_operator_nudges_globally_enabled() -> bool {
    router_rs_operator_inject_globally_enabled()
        && router_rs_env_enabled_default_true(HARNESS_NUDGES_ENV)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HarnessOperatorNudgesFile {
    #[serde(default)]
    schema_version: String,
    #[serde(default)]
    nudges: NudgesBody,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct NudgesBody {
    #[serde(default)]
    rfv_loop_continue_reasoning_depth: String,
    #[serde(default)]
    autopilot_drive_verbose_reasoning_depth: String,
    #[serde(default)]
    autopilot_drive_compact_reasoning_depth: String,
    /// Optional second line after reasoning-depth nudges: STEM / witness + checker reminder.
    #[serde(default)]
    math_reasoning_harness_line: String,
    #[serde(default)]
    retrieval_trace_harness_line: String,
    /// Compact one-liner appended when structured external research hint applies (RFV_LOOP_CONTINUE path).
    #[serde(default)]
    rfv_loop_external_struct_hint_line: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedHarnessNudges {
    pub rfv_loop_continue_reasoning_depth: String,
    pub autopilot_drive_verbose_reasoning_depth: String,
    pub autopilot_drive_compact_reasoning_depth: String,
    pub math_reasoning_harness_line: String,
    pub retrieval_trace_harness_line: String,
    pub rfv_loop_external_struct_hint_line: String,
}

impl ResolvedHarnessNudges {
    fn disabled() -> Self {
        Self {
            rfv_loop_continue_reasoning_depth: String::new(),
            autopilot_drive_verbose_reasoning_depth: String::new(),
            autopilot_drive_compact_reasoning_depth: String::new(),
            math_reasoning_harness_line: String::new(),
            retrieval_trace_harness_line: String::new(),
            rfv_loop_external_struct_hint_line: String::new(),
        }
    }
}

fn builtin_defaults() -> ResolvedHarnessNudges {
    ResolvedHarnessNudges {
        rfv_loop_continue_reasoning_depth: "推理深度：不靠单模型拉长 CoT；靠 review∥external→fix→verify 分工 + EVIDENCE_INDEX 可审计链。"
            .to_string(),
        autopilot_drive_verbose_reasoning_depth: "推理深度：不靠单模型拉长 CoT；靠地平线切片 + 可执行验证写入 EVIDENCE_INDEX/检查点，形成可审计链。"
            .to_string(),
        autopilot_drive_compact_reasoning_depth: "深度：切片+验证证据链，非单模型堆长推理。"
            .to_string(),
        math_reasoning_harness_line: String::new(),
        retrieval_trace_harness_line: String::new(),
        rfv_loop_external_struct_hint_line:
            "外研结构化：本轮 `append_round` 需填 `external_research`，按 `RFV_EXTERNAL_RESEARCH.schema.json` 覆盖 sources、contradiction_sweep、queries_used、unknowns。"
                .to_string(),
    }
}

/// Merge repo JSON over compiled defaults. If the env disables nudges, returns empty strings.
pub fn resolve_harness_operator_nudges(repo_root: &Path) -> ResolvedHarnessNudges {
    if !harness_operator_nudges_globally_enabled() {
        return ResolvedHarnessNudges::disabled();
    }
    let mut out = builtin_defaults();
    let path = repo_root.join(NUDGES_REL_PATH);
    let Ok(text) = fs::read_to_string(&path) else {
        return out;
    };
    let Ok(file) = serde_json::from_str::<HarnessOperatorNudgesFile>(&text) else {
        eprintln!(
            "[router-rs] harness operator nudges: parse failed at {}",
            path.display()
        );
        return out;
    };
    if !file.schema_version.is_empty() && file.schema_version != EXPECTED_SCHEMA_VERSION {
        // Safety over tolerance: an unknown shape might mean v2 introduced new semantics for
        // the same key names. Falling back to compiled defaults keeps the model-facing prompt
        // predictable; an explicit upgrade of `EXPECTED_SCHEMA_VERSION` is required to merge.
        eprintln!(
            "[router-rs] harness operator nudges: expected schema_version={EXPECTED_SCHEMA_VERSION}, got {:?} — falling back to compiled defaults (no partial merge)",
            file.schema_version
        );
        return out;
    }
    merge_nonempty(
        &mut out.rfv_loop_continue_reasoning_depth,
        &file.nudges.rfv_loop_continue_reasoning_depth,
    );
    merge_nonempty(
        &mut out.autopilot_drive_verbose_reasoning_depth,
        &file.nudges.autopilot_drive_verbose_reasoning_depth,
    );
    merge_nonempty(
        &mut out.autopilot_drive_compact_reasoning_depth,
        &file.nudges.autopilot_drive_compact_reasoning_depth,
    );
    merge_nonempty(
        &mut out.math_reasoning_harness_line,
        &file.nudges.math_reasoning_harness_line,
    );
    merge_nonempty(
        &mut out.retrieval_trace_harness_line,
        &file.nudges.retrieval_trace_harness_line,
    );
    merge_nonempty(
        &mut out.rfv_loop_external_struct_hint_line,
        &file.nudges.rfv_loop_external_struct_hint_line,
    );
    out
}

fn merge_nonempty(target: &mut String, incoming: &str) {
    let t = incoming.trim();
    if !t.is_empty() {
        *target = t.to_string();
    }
}

/// Serialize tests that touch `ROUTER_RS_HARNESS_OPERATOR_NUDGES` or assume default nudge injection.
#[cfg(test)]
pub(crate) fn harness_nudges_env_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("harness nudges env test lock")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn resolve_uses_builtin_when_file_missing() {
        let _g = harness_nudges_env_test_lock();
        std::env::remove_var(HARNESS_NUDGES_ENV);
        let tmp = std::env::temp_dir().join(format!(
            "harness-nudges-missing-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let n = resolve_harness_operator_nudges(&tmp);
        assert!(n
            .rfv_loop_continue_reasoning_depth
            .contains("EVIDENCE_INDEX"));
        assert!(n.autopilot_drive_compact_reasoning_depth.contains("切片"));
    }

    #[test]
    fn resolve_overrides_from_json() {
        let _g = harness_nudges_env_test_lock();
        std::env::remove_var(HARNESS_NUDGES_ENV);
        let tmp = std::env::temp_dir().join(format!(
            "harness-nudges-override-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        let p = tmp.join(NUDGES_REL_PATH);
        let mut f = std::fs::File::create(&p).unwrap();
        write!(
            f,
            r#"{{"schema_version":"harness-operator-nudges-v1","nudges":{{"rfv_loop_continue_reasoning_depth":"CUSTOM_RFV_NUDGE"}}}}"#
        )
        .unwrap();
        drop(f);
        let n = resolve_harness_operator_nudges(&tmp);
        assert_eq!(n.rfv_loop_continue_reasoning_depth, "CUSTOM_RFV_NUDGE");
        assert!(n.autopilot_drive_verbose_reasoning_depth.contains("地平线"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_math_line_from_json() {
        let _g = harness_nudges_env_test_lock();
        std::env::remove_var(HARNESS_NUDGES_ENV);
        let tmp = std::env::temp_dir().join(format!(
            "harness-nudges-math-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        let p = tmp.join(NUDGES_REL_PATH);
        let mut f = std::fs::File::create(&p).unwrap();
        write!(
            f,
            r#"{{"schema_version":"harness-operator-nudges-v1","nudges":{{"math_reasoning_harness_line":"MATH_TEST_LINE"}}}}"#
        )
        .unwrap();
        drop(f);
        let n = resolve_harness_operator_nudges(&tmp);
        assert_eq!(n.math_reasoning_harness_line, "MATH_TEST_LINE");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_retrieval_line_from_json() {
        let _g = harness_nudges_env_test_lock();
        std::env::remove_var(HARNESS_NUDGES_ENV);
        let tmp = std::env::temp_dir().join(format!(
            "harness-nudges-retrieval-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        let p = tmp.join(NUDGES_REL_PATH);
        let mut f = std::fs::File::create(&p).unwrap();
        write!(
            f,
            r#"{{"schema_version":"harness-operator-nudges-v1","nudges":{{"retrieval_trace_harness_line":"RETR_TEST_LINE"}}}}"#
        )
        .unwrap();
        drop(f);
        let n = resolve_harness_operator_nudges(&tmp);
        assert_eq!(n.retrieval_trace_harness_line, "RETR_TEST_LINE");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// P0-F: an unknown `schema_version` must fall back to compiled defaults (no partial merge),
    /// so future v2 additions can never silently corrupt the v1 model-facing prompt.
    #[test]
    fn schema_version_mismatch_falls_back_to_builtin_defaults() {
        let _g = harness_nudges_env_test_lock();
        std::env::remove_var(HARNESS_NUDGES_ENV);
        std::env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        let tmp = std::env::temp_dir().join(format!(
            "harness-nudges-schema-mismatch-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        let p = tmp.join(NUDGES_REL_PATH);
        let mut f = std::fs::File::create(&p).unwrap();
        write!(
            f,
            r#"{{"schema_version":"harness-operator-nudges-v999","nudges":{{"rfv_loop_continue_reasoning_depth":"NEW_V999_VALUE_SHOULD_BE_IGNORED"}}}}"#
        )
        .unwrap();
        drop(f);
        let n = resolve_harness_operator_nudges(&tmp);
        // Built-in default mentions EVIDENCE_INDEX; the V999 override must not have leaked through.
        assert!(
            n.rfv_loop_continue_reasoning_depth
                .contains("EVIDENCE_INDEX"),
            "expected built-in default after schema mismatch; got {:?}",
            n.rfv_loop_continue_reasoning_depth
        );
        assert!(
            !n.rfv_loop_continue_reasoning_depth.contains("V999"),
            "schema_version mismatch must not silently merge fields"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// P1-E: `ROUTER_RS_OPERATOR_INJECT=0` aggregate kill-switch disables nudges even when
    /// the per-nudge env is unset (default-on).
    #[test]
    fn operator_inject_kill_switch_disables_nudges() {
        let _g = harness_nudges_env_test_lock();
        let prior_nudge = std::env::var(HARNESS_NUDGES_ENV).ok();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        std::env::remove_var(HARNESS_NUDGES_ENV);
        std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
        let tmp = std::env::temp_dir().join("harness-nudges-aggregate-off");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let n = resolve_harness_operator_nudges(&tmp);
        assert!(
            n.rfv_loop_continue_reasoning_depth.is_empty(),
            "aggregate kill-switch must zero out nudges"
        );
        assert!(n.autopilot_drive_compact_reasoning_depth.is_empty());
        assert!(n.math_reasoning_harness_line.is_empty());
        assert!(n.retrieval_trace_harness_line.is_empty());
        assert!(n.rfv_loop_external_struct_hint_line.is_empty());
        match prior_nudge {
            Some(v) => std::env::set_var(HARNESS_NUDGES_ENV, v),
            None => std::env::remove_var(HARNESS_NUDGES_ENV),
        }
        match prior_inject {
            Some(v) => std::env::set_var("ROUTER_RS_OPERATOR_INJECT", v),
            None => std::env::remove_var("ROUTER_RS_OPERATOR_INJECT"),
        }
    }

    #[test]
    fn env_off_yields_empty() {
        let _g = harness_nudges_env_test_lock();
        let prior = std::env::var(HARNESS_NUDGES_ENV).ok();
        std::env::set_var(HARNESS_NUDGES_ENV, "0");
        let tmp = std::env::temp_dir().join("harness-nudges-env-off");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let n = resolve_harness_operator_nudges(&tmp);
        assert!(n.rfv_loop_continue_reasoning_depth.is_empty());
        assert!(n.retrieval_trace_harness_line.is_empty());
        assert!(n.rfv_loop_external_struct_hint_line.is_empty());
        match prior {
            Some(v) => std::env::set_var(HARNESS_NUDGES_ENV, v),
            None => std::env::remove_var(HARNESS_NUDGES_ENV),
        }
    }
}
