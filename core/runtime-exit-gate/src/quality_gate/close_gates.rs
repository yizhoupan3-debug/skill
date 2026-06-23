//! RFV 收口闸门：supervisor 显式 close 或 max_rounds 耗尽时执行 close_gates 校验。

use super::*;

/// Optional hard gates on RFV **收口轮**预览（`append_round`）：supervisor 显式 **`close`/`closed`**，
/// 或 **`max_rounds` 耗尽**（`round_n >= max_rounds` 且非 block）自动记 `closed` 时同样校验。
#[derive(Debug, Clone)]
pub struct RfvCloseGates {
    pub enabled: bool,
    pub require_last_round_verify_pass: bool,
    pub min_depth_score: Option<u8>,
    pub block_on_rfv_pass_without_evidence: bool,
    pub require_external_research_object_when_strict_on_close: bool,
}

pub fn parse_close_gates(state: &Map<String, Value>) -> Option<RfvCloseGates> {
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

pub fn enforce_rfv_close_gates(
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
    if gates.require_external_research_object_when_strict_on_close && allow_external && strict_task {
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
        core_state::state_manager::task_evidence_artifacts_summary_for_task(repo_root, task_id);
    let goal_opt = core_state::state_manager::read_goal_state(repo_root, Some(task_id))
        .ok()
        .flatten();
    let preview_val = Value::Object(preview_rfv.clone());
    let dc = core_state::task_state::depth_compliance_aggregate(
        goal_opt.as_ref(),
        Some(&preview_val),
        evidence_ok,
    );
    if let Some(min) = gates.min_depth_score
        && dc.depth_score < min {
            return Err(format!(
                "RFV close_gates: depth_score={} < min_depth_score={}",
                dc.depth_score, min
            ));
        }
    if gates.block_on_rfv_pass_without_evidence && dc.qg_pass_without_evidence_count > 0 {
        return Err(format!(
            "RFV close_gates: block_on_rfv_pass_without_evidence but qg_pass_without_evidence_count={}",
            dc.qg_pass_without_evidence_count
        ));
    }
    Ok(())
}
