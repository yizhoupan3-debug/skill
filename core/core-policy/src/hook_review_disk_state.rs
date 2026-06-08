//! Cross-host hook disk review gate core fields, JSON hydration, and Stop orchestration.
//! Host modules own transport projection (Cursor `followup_message`, Codex `decision:block`, Claude `stopReason`).

use crate::review_gate_engine::{review_gate_blocks_stop, ReviewGateFacts};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Schema version for a **review-only** disk file (Claude `review_gate_*.json`).
pub const HOOK_REVIEW_DISK_VERSION: u32 = 1;

/// Shared review gate fields persisted on hook state disks (all three hosts).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookReviewGateFields {
    #[serde(default)]
    pub review_required: bool,
    #[serde(default)]
    pub review_override: bool,
    #[serde(default)]
    pub independent_reviewer_seen: bool,
    #[serde(default)]
    pub reject_reason_seen: bool,
}

/// Claude `review_gate_*.json` on-disk shape (version + shared gate fields).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookReviewDiskCore {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub review_required: bool,
    #[serde(default)]
    pub review_override: bool,
    #[serde(default, alias = "independent_review_subagent_seen")]
    pub independent_reviewer_seen: bool,
    #[serde(default)]
    pub reject_reason_seen: bool,
}

/// Common trait for hook-state structs that carry a schema `version` field.
pub trait HookReviewDiskVersion {
    const STATE_VERSION: u32;
    fn disk_version(&self) -> u32;
}

impl HookReviewDiskVersion for HookReviewDiskCore {
    const STATE_VERSION: u32 = HOOK_REVIEW_DISK_VERSION;
    fn disk_version(&self) -> u32 {
        self.version
    }
}

impl From<&HookReviewDiskCore> for HookReviewGateFields {
    fn from(core: &HookReviewDiskCore) -> Self {
        core.gate_fields()
    }
}

impl HookReviewGateFields {
    pub fn facts(&self) -> ReviewGateFacts {
        ReviewGateFacts {
            review_required: self.review_required,
            review_override: self.review_override,
            independent_reviewer_seen: self.independent_reviewer_seen,
        }
    }

    /// Armed review gate would nudge on Stop (before host-specific pending/multiset rules).
    /// L4 must treat this as advisory when [`hook_common::review_gate_advisory_only`] is true.
    pub fn review_gate_stop_blocks(&self) -> bool {
        review_gate_blocks_stop(self.facts())
    }

    /// Codex/Cursor/Claude: `reject_reason_seen` bounded escape clears Stop nudge.
    pub fn review_stop_blocks(&self) -> bool {
        review_stop_blocks_with_reject_escape(self)
    }
}

impl HookReviewDiskCore {
    pub fn bump_version_for_save(&mut self) {
        self.version = HOOK_REVIEW_DISK_VERSION;
    }

    pub fn gate_fields(&self) -> HookReviewGateFields {
        hook_review_gate_fields_from_parts(
            self.review_required,
            self.review_override,
            self.independent_reviewer_seen,
            self.reject_reason_seen,
        )
    }
}

/// Shared hook-state review gate JSON basename (`review-subagent-<session_key>.json`).
pub fn hook_review_subagent_state_basename(session_key: &str) -> String {
    format!("review-subagent-{session_key}.json")
}

/// Legacy Claude hook-state basename (`review_gate_<session_key>.json`) — read/migrate only.
pub fn hook_review_gate_legacy_state_basename(session_key: &str) -> String {
    format!("review_gate_{session_key}.json")
}

/// Build shared gate fields from the four persisted booleans.
pub fn hook_review_gate_fields_from_parts(
    review_required: bool,
    review_override: bool,
    independent_reviewer_seen: bool,
    reject_reason_seen: bool,
) -> HookReviewGateFields {
    HookReviewGateFields {
        review_required,
        review_override,
        independent_reviewer_seen,
        reject_reason_seen,
    }
}

/// Stop path when hook-state is absent: prompt facts + `reject_reason_seen`.
pub fn hook_review_gate_fields_from_facts(
    facts: &ReviewGateFacts,
    reject_reason_seen: bool,
) -> HookReviewGateFields {
    hook_review_gate_fields_from_parts(
        facts.review_required,
        facts.review_override,
        facts.independent_reviewer_seen,
        reject_reason_seen,
    )
}

/// Write shared gate fields into mutable host-state bool slots.
pub fn apply_hook_review_gate_fields(
    fields: &HookReviewGateFields,
    review_required: &mut bool,
    review_override: &mut bool,
    independent_reviewer_seen: &mut bool,
    reject_reason_seen: &mut bool,
) {
    *review_required = fields.review_required;
    *review_override = fields.review_override;
    *independent_reviewer_seen = fields.independent_reviewer_seen;
    *reject_reason_seen = fields.reject_reason_seen;
}

/// Hydrate shared gate fields from JSON into mutable host-state bool slots.
pub fn hydrate_hook_review_gate_fields_from_value(
    value: &Value,
    review_required: &mut bool,
    review_override: &mut bool,
    independent_reviewer_seen: &mut bool,
    reject_reason_seen: &mut bool,
) {
    apply_hook_review_gate_fields(
        &hook_review_gate_fields_from_value(value),
        review_required,
        review_override,
        independent_reviewer_seen,
        reject_reason_seen,
    );
}

/// Cross-host JSON key for independent reviewer evidence (canonical + legacy alias).
pub fn hook_review_independent_reviewer_seen_from_value(value: &Value) -> bool {
    value
        .get("independent_reviewer_seen")
        .and_then(Value::as_bool)
        .or_else(|| {
            value
                .get("independent_review_subagent_seen")
                .and_then(Value::as_bool)
        })
        .unwrap_or(false)
}

/// Hydrate shared gate fields from arbitrary hook-state JSON (v0 missing `version` → defaults).
pub fn hook_review_gate_fields_from_value(value: &Value) -> HookReviewGateFields {
    hook_review_gate_fields_from_parts(
        value
            .get("review_required")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        value
            .get("review_override")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        hook_review_independent_reviewer_seen_from_value(value),
        value
            .get("reject_reason_seen")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    )
}

/// Hydrate Claude review_gate JSON (v0/v1).
pub fn hook_review_disk_core_from_value(value: &Value) -> HookReviewDiskCore {
    let gate = hook_review_gate_fields_from_value(value);
    HookReviewDiskCore {
        version: value
            .get("version")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
        review_required: gate.review_required,
        review_override: gate.review_override,
        independent_reviewer_seen: gate.independent_reviewer_seen,
        reject_reason_seen: gate.reject_reason_seen,
    }
}

/// Migrate legacy v0 review disk (no version) to current in-memory shape; bumps version on next save.
pub fn migrate_hook_review_disk_core(raw: &Value) -> HookReviewDiskCore {
    hook_review_disk_core_from_value(raw)
}

/// Armed gate Stop nudge unless `reject_reason_seen` cleared (advisory on all hosts at L4).
pub fn review_stop_blocks_with_reject_escape(fields: &HookReviewGateFields) -> bool {
    if fields.reject_reason_seen {
        return false;
    }
    fields.review_gate_stop_blocks()
}

/// Claude-canonical Stop review advisory: `Some(nudge_line)` when armed gate unsatisfied.
///
/// Host transport must check suppression (`*_review_gate_suppressed`: env disable + my-light)
/// before calling. Pending multiset / phase are **not** inputs (telemetry only on Cursor).
pub fn hook_review_stop_advisory_needed(
    fields: &HookReviewGateFields,
    review_gate_tag: &str,
) -> Option<String> {
    if fields.review_stop_blocks() {
        Some(hook_review_stop_advisory_line(review_gate_tag))
    } else {
        None
    }
}

/// Single cross-host Stop review nudge line (Claude canonical wording; tag per host).
pub fn hook_review_stop_advisory_line(review_gate_tag: &str) -> String {
    if crate::env_flags::router_rs_review_fork_context_missing_infer_false_enabled() {
        format!(
            "router-rs {review_gate_tag} incomplete: run an observed independent reviewer lane \
             (fork_context=false, or omit fork_context when \
             ROUTER_RS_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE=1) before closing this review turn."
        )
    } else {
        format!(
            "router-rs {review_gate_tag} incomplete: run an observed independent reviewer lane \
             with explicit fork_context=false before closing this review turn."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hook_review_gate_fields_from_value_defaults_v0() {
        let fields = hook_review_gate_fields_from_value(&json!({}));
        assert!(!fields.review_required);
        assert!(!fields.independent_reviewer_seen);
    }

    #[test]
    fn hook_review_independent_reviewer_seen_reads_legacy_alias() {
        let raw = json!({
            "independent_review_subagent_seen": true,
        });
        assert!(hook_review_independent_reviewer_seen_from_value(&raw));
        let fields = hook_review_gate_fields_from_value(&raw);
        assert!(fields.independent_reviewer_seen);
    }

    #[test]
    fn migrate_hook_review_disk_core_preserves_v1_fields() {
        let raw = json!({
            "version": 1,
            "review_required": true,
            "review_override": false,
            "independent_reviewer_seen": false,
        });
        let core = migrate_hook_review_disk_core(&raw);
        assert_eq!(core.version, 1);
        assert!(core.review_required);
        assert!(core.gate_fields().review_gate_stop_blocks());
        assert!(core.gate_fields().review_stop_blocks());
    }

    #[test]
    fn reject_reason_seen_clears_stop_block() {
        let fields = HookReviewGateFields {
            review_required: true,
            review_override: false,
            independent_reviewer_seen: false,
            reject_reason_seen: true,
        };
        assert!(!fields.review_stop_blocks());
        assert!(fields.review_gate_stop_blocks());
    }

    #[test]
    fn independent_reviewer_seen_satisfies_gate() {
        let fields = HookReviewGateFields {
            review_required: true,
            review_override: false,
            independent_reviewer_seen: true,
            reject_reason_seen: false,
        };
        assert!(!fields.review_stop_blocks());
    }

    #[test]
    fn hydrate_hook_review_gate_fields_from_value_writes_slots() {
        let raw = json!({
            "review_required": true,
            "review_override": true,
            "independent_reviewer_seen": true,
            "reject_reason_seen": true,
        });
        let mut review_required = false;
        let mut review_override = false;
        let mut independent_reviewer_seen = false;
        let mut reject_reason_seen = false;
        hydrate_hook_review_gate_fields_from_value(
            &raw,
            &mut review_required,
            &mut review_override,
            &mut independent_reviewer_seen,
            &mut reject_reason_seen,
        );
        assert!(review_required);
        assert!(review_override);
        assert!(independent_reviewer_seen);
        assert!(reject_reason_seen);
    }

    #[test]
    fn hook_review_stop_advisory_needed_matches_stop_blocks() {
        let armed = HookReviewGateFields {
            review_required: true,
            review_override: false,
            independent_reviewer_seen: false,
            reject_reason_seen: false,
        };
        let line = hook_review_stop_advisory_needed(&armed, "CLAUDE_REVIEW_GATE")
            .expect("armed gate must nudge");
        assert!(line.contains("CLAUDE_REVIEW_GATE"));
        assert!(line.contains("fork_context=false"));

        let cleared = HookReviewGateFields {
            independent_reviewer_seen: true,
            ..armed
        };
        assert!(hook_review_stop_advisory_needed(&cleared, "CODEX_REVIEW_GATE").is_none());
    }

    #[test]
    fn hook_review_subagent_state_basename_matches_cursor_codex() {
        assert_eq!(
            hook_review_subagent_state_basename("abc123"),
            "review-subagent-abc123.json"
        );
    }

    #[test]
    fn hook_review_gate_legacy_state_basename_matches_claude_hook_state() {
        assert_eq!(
            hook_review_gate_legacy_state_basename("abc123"),
            "review_gate_abc123.json"
        );
    }
}
