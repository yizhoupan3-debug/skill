//! Core types for the Loop Architecture.
//!
//! Includes deserialization types for LOOP_REGISTRY.json, runtime phase enums,
//! safety levels, LoopActionRecord, LoopCloseoutAggregate, and related types.

use serde::{Deserialize, Serialize};

// ── Phase ──

/// Phase of the Loop Runner state machine.
///
/// ```text
/// PENDING → DISCOVERING → PREFLIGHT → DISPATCHING → RUNNING → VERIFYING → COMPLETED
///                                                              ↘ ESCALATED
/// 任意阶段 → INTERRUPTED（kill/超时）
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopPhase {
    Pending,
    Discovering,
    Preflight,
    Dispatching,
    Running,
    Verifying,
    Completed,
    Escalated,
    Interrupted,
}

impl LoopPhase {
    pub fn is_terminal(&self) -> bool {
        matches!(self, LoopPhase::Completed | LoopPhase::Interrupted)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LoopPhase::Pending => "pending",
            LoopPhase::Discovering => "discovering",
            LoopPhase::Preflight => "preflight",
            LoopPhase::Dispatching => "dispatching",
            LoopPhase::Running => "running",
            LoopPhase::Verifying => "verifying",
            LoopPhase::Completed => "completed",
            LoopPhase::Escalated => "escalated",
            LoopPhase::Interrupted => "interrupted",
        }
    }
}

// ── Safety Level ──

/// Scope-based safety level (§6.1). Use to control whether a loop action reports only,
/// assists with fixes, or runs unattended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyLevel {
    /// L1 report-only：发现 + 报告，不改文件。
    L1ReportOnly,
    /// L2 assisted-fix：修改 + 验证 + commit（不 merge）。
    L2AssistedFix,
    /// L3 unattended：修改 + 验证 + commit（不 merge）。
    L3Unattended,
}

impl SafetyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            SafetyLevel::L1ReportOnly => "L1",
            SafetyLevel::L2AssistedFix => "L2",
            SafetyLevel::L3Unattended => "L3",
        }
    }
}

// ── Profile Config ──

/// Profile configuration snapshot loaded from RUNTIME_REGISTRY.json during PREFLIGHT.
/// Determines loop behaviour (scheduling, closeout enforcement, review gating, budgets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopProfileConfig {
    /// profile 标识符（"loop-auto" / "interactive"）
    pub profile: String,
    /// 是否可被循环调度器调度
    pub loop_capable: bool,
    /// closeout 强制模式
    pub closeout_enforcement: String,
    /// review_gate 模式
    pub review_gate: String,
    /// 是否强制 spawn-first nudge
    pub spawn_first_nudge: bool,

    /// 预算配置（§5.2 cost_budget）
    #[serde(default)]
    pub cost_budget: Option<CostBudgetConfig>,

    /// 升级策略（§8 escalation）
    #[serde(default)]
    pub escalation: Option<EscalationConfig>,
}

impl LoopProfileConfig {
    pub fn from_runtime_registry(repo_root: &std::path::Path, profile_name: &str) -> Option<Self> {
        let path = repo_root
            .join("configs")
            .join("framework")
            .join("RUNTIME_REGISTRY.json");
        let raw = std::fs::read_to_string(&path).ok()?;
        let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let profiles = val.get("lifecycle_profiles")?;
        let profile_val = profiles.get(profile_name)?;

        let loop_capable = profile_val
            .get("loop_capable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let closeout_enforcement = profile_val
            .get("closeout_enforcement")
            .and_then(|v| v.as_str())
            .unwrap_or("advisory")
            .to_string();
        let review_gate = profile_val
            .get("review_gate")
            .and_then(|v| v.as_str())
            .unwrap_or("suppressed")
            .to_string();
        let spawn_first_nudge = profile_val
            .get("spawn_first_nudge")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let cost_budget = profile_val
            .get("cost_budget")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let escalation = profile_val
            .get("escalation")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        Some(LoopProfileConfig {
            profile: profile_name.to_string(),
            loop_capable,
            closeout_enforcement,
            review_gate,
            spawn_first_nudge,
            cost_budget,
            escalation,
        })
    }

    pub fn is_hard_block(&self) -> bool {
        self.closeout_enforcement == "hard-block"
    }
}

/// Soft token budget constraints per run or per day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBudgetConfig {
    /// 单次运行的 token 软上限
    #[serde(default)]
    pub tokens_per_run: Option<u64>,
    /// 每日 token 软上限
    #[serde(default)]
    pub daily_tokens: Option<u64>,
}

/// Escalation strategy defining fallback actions on closeout / verify / budget / error failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationConfig {
    #[serde(default)]
    pub on_closeout_fail: Option<String>,
    #[serde(default)]
    pub on_verify_fail: Option<String>,
    #[serde(default)]
    pub on_budget_exceeded: Option<String>,
    #[serde(default)]
    pub on_unexpected_error: Option<String>,
}

// ── Loop Registry Entry ──

/// A single loop registration entry from LOOP_REGISTRY.json (§4.1).
/// Defines the loop ID, profile, trigger schedule, safety rules, and optional research config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopRegistryEntry {
    pub loop_id: String,
    pub profile: String,
    pub trigger: LoopTriggerConfig,
    pub skill: Option<String>,
    #[serde(default)]
    pub scope_based_safety: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub default_safety: Option<String>,
    #[serde(default)]
    pub scope_conflict_resolution: Option<String>,
    #[serde(default)]
    pub cost_budget: Option<CostBudgetConfig>,
    #[serde(default)]
    pub notification: Option<serde_json::Value>,
    #[serde(default)]
    pub research_enabled: bool,
    #[serde(default)]
    pub research: Option<ResearchConfig>,
    /// When true, the runner verifies RFV convergence state after a "pass" aggregate.
    /// If RFV hasn't converged, the aggregate is downgraded to "fail".
    /// Only meaningful for loops that use the RFV loop protocol.
    #[serde(default)]
    pub verify_rfv_convergence: Option<bool>,
    /// Pre-defined static action list. When present, the runner uses these actions
    /// directly instead of spawning a subagent for discovery.
    #[serde(default)]
    pub static_actions: Option<Vec<LoopAction>>,
}

/// Research configuration for barrier escalation (§19.9).
/// Used by research-aware loops to define escalation thresholds, auto-resume behaviour, and time limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchConfig {
    #[serde(default = "default_barrier_threshold")]
    pub barrier_threshold: u32,
    #[serde(default = "default_escalation_target")]
    pub escalation_target: String,
    #[serde(default = "default_max_research_time_min")]
    pub max_research_time_min: u32,
    #[serde(default = "default_auto_resume")]
    pub auto_resume: bool,
    #[serde(default)]
    pub require_human_approval: bool,
}

fn default_barrier_threshold() -> u32 {
    3
}
fn default_escalation_target() -> String {
    "autoresearch".to_string()
}
fn default_max_research_time_min() -> u32 {
    30
}
fn default_auto_resume() -> bool {
    true
}

/// Loop trigger configuration specifying the trigger type (e.g. cron / manual) and optional schedule parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopTriggerConfig {
    #[serde(rename = "type")]
    pub trigger_type: String,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
}

/// Root structure of LOOP_REGISTRY.json.
/// Contains the schema version and the full list of registered loops.
///
/// # Design note: String-typed enums vs Rust enums (M8)
/// Several fields use `String` rather than a Rust `enum` (e.g. `profile`, `safety`,
/// `execution`, `overall_status`). This is intentional: the JSON schema is consumed
/// by external tools and may evolve independently. Using `String` provides forward
/// compatibility — unknown values deserialize without error. The trade-off is that
/// invalid values are caught at runtime rather than compile time. If stricter
/// validation is needed, introduce a serde `deserialize_with` custom deserializer
/// that falls back gracefully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopRegistryRoot {
    pub schema_version: String,
    pub loops: Vec<LoopRegistryEntry>,
}

// ── Loop Action ──

/// A single action allocated during the DISCOVERING phase of a loop run.
/// Each action carries a type, scope paths, safety level, and optional description for the subagent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopAction {
    pub action_id: String,
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(default)]
    pub scope_paths: Vec<String>,
    pub safety: String,
    #[serde(default)]
    pub description: Option<String>,
}

// ── Loop Action Record ──

/// Closeout record for a single action (§5.3).
/// Embeds the existing CloseoutRecord from framework-runtime.
///
/// Written to `artifacts/closeout/<action-id>.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopActionRecord {
    pub schema_version: String,
    pub loop_id: String,
    pub run_id: String,
    pub action_id: String,
    pub safety_level: String,
    /// 嵌入现有 CloseoutRecord（来自 framework-runtime）
    pub closeout: serde_json::Value,
}

// ── Loop Closeout Aggregate ──

/// Aggregated closeout result for a single loop run (§5.4).
/// Contains the overall status, per-action entries, and escalation/partial flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopCloseoutAggregate {
    pub schema_version: String,
    pub run_id: String,
    pub loop_id: String,
    pub overall_status: String,
    pub actions: Vec<AggregateActionEntry>,
    pub escalated: bool,
    pub partial: bool,
}

/// A single action entry within a LoopCloseoutAggregate.
/// Records the execution outcome, closeout path, verification result, and optional commit SHA.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AggregateActionEntry {
    pub action_id: String,
    pub safety_level: String,
    pub execution: String,
    #[serde(default)]
    pub closeout_path: Option<String>,
    #[serde(default)]
    pub verification: Option<String>,
    #[serde(default)]
    pub commit_sha: Option<String>,
    #[serde(default)]
    pub merged: Option<bool>,
}

/// Anti-drift check state, persisted in LoopRunState.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiDriftState {
    /// Review cycles completed since last drift check (or since loop start).
    #[serde(default)]
    pub review_cycle_count: u32,
    /// Interval at which drift checks fire (default: 3).
    #[serde(default = "default_drift_check_interval")]
    pub check_interval: u32,
    /// Original goal snapshot (text) at loop start, for comparison.
    #[serde(default)]
    pub original_goal_snapshot: Option<String>,
    /// Most recent drift check result.
    #[serde(default)]
    pub last_drift_check: Option<DriftCheckResult>,
    /// History of all drift checks performed (capped at 20 entries).
    #[serde(default)]
    pub drift_check_history: Vec<DriftCheckResult>,
}

impl Default for AntiDriftState {
    fn default() -> Self {
        Self {
            review_cycle_count: 0,
            check_interval: 3,
            original_goal_snapshot: None,
            last_drift_check: None,
            drift_check_history: Vec::new(),
        }
    }
}

fn default_drift_check_interval() -> u32 {
    3
}

/// Result of a single drift check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftCheckResult {
    pub checked_at: String,
    pub review_cycle: u32,
    pub drift_detected: bool,
    pub drift_score: f64,
    pub drift_type: String,
    pub detail: String,
}

// ── Loop Run State ──

/// Runtime persistent structure serialised as LOOP_RUN_STATE.json (§5.2).
/// Tracks the current phase, heartbeat, run history, and circuit breaker state for a loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopRunState {
    pub schema_version: String,
    pub loop_id: String,
    pub profile: String,
    pub phase: String,
    pub last_heartbeat: String,
    #[serde(default)]
    pub current_run: Option<CurrentRun>,
    #[serde(default)]
    pub history: Vec<RunHistoryEntry>,
    #[serde(default)]
    pub circuit_breaker: CircuitBreaker,
    #[serde(default)]
    pub anti_drift: AntiDriftState,
    pub last_refreshed_at: String,
}

/// Snapshot of the currently active loop run.
/// Contains discovery results, unconsumed findings, dispatch map, and the closeout aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentRun {
    pub run_id: String,
    pub started_at: String,
    #[serde(default)]
    pub discovery: Option<DiscoveryResult>,
    #[serde(default)]
    pub unconsumed_findings: Vec<UnconsumedFinding>,
    #[serde(default)]
    pub dispatch: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub closeout_aggregate: Option<LoopCloseoutAggregate>,
    #[serde(default)]
    pub report_path: Option<String>,
}

/// Result produced by the DISCOVERING phase: count of actions found and the full action list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryResult {
    pub actions_found: u32,
    pub actions: Vec<LoopAction>,
}

/// An unconsumed finding carried forward to the next DISCOVERING cycle.
/// Prevents duplicate work when a previous run's findings were not fully addressed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnconsumedFinding {
    pub finding_hash: String,
    pub source_action: String,
    pub finding: String,
}

/// A historical record of a completed loop run in the run history log.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunHistoryEntry {
    pub run_id: String,
    pub phase: String,
    pub result: String,
}

/// Circuit breaker state tracking consecutive failures and kill-switch arming (§6.3).
/// Auto-escalates when the consecutive failure threshold is reached.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(Default)]
pub struct CircuitBreaker {
    #[serde(default)]
    pub consecutive_failures: u32,
    #[serde(default)]
    pub kill_switch_armed: bool,
    #[serde(default)]
    pub kill_switch_triggered_at: Option<String>,
}

// ── Error ──

/// Error type for loop-engine operations, covering profile mismatches, kill signals,
/// timeouts, spawn failures, serialization errors, action failures, and research escalations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LoopError {
    #[error("Profile mismatch: {0}")]
    ProfileMismatch(String),

    #[error("Unknown profile: {0}")]
    UnknownProfile(String),

    #[error("Kill signal received: {0}")]
    KillSignaled(String),

    #[error("Timeout after {0}s")]
    Timeout(u64),

    #[error("Subagent spawn failed: {0}")]
    SpawnFailed(String),

    #[error("Serialization error: {0}")]
    Serde(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Action failed: {0}")]
    ActionFailed(String),

    #[error("Research escalation: {0}")]
    ResearchEscalation(String),

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),
}

impl From<serde_json::Error> for LoopError {
    fn from(e: serde_json::Error) -> Self {
        LoopError::Serde(e.to_string())
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_phase_terminal() {
        assert!(LoopPhase::Completed.is_terminal());
        assert!(
            !LoopPhase::Escalated.is_terminal(),
            "Escalated is no longer terminal: auto-resume may transition back to Dispatching"
        );
        assert!(LoopPhase::Interrupted.is_terminal());
        assert!(!LoopPhase::Pending.is_terminal());
        assert!(!LoopPhase::Running.is_terminal());
    }

    #[test]
    fn test_loop_phase_as_str() {
        assert_eq!(LoopPhase::Pending.as_str(), "pending");
        assert_eq!(LoopPhase::Verifying.as_str(), "verifying");
        assert_eq!(LoopPhase::Interrupted.as_str(), "interrupted");
    }

    #[test]
    fn test_safety_level_as_str() {
        assert_eq!(SafetyLevel::L1ReportOnly.as_str(), "L1");
        assert_eq!(SafetyLevel::L2AssistedFix.as_str(), "L2");
        assert_eq!(SafetyLevel::L3Unattended.as_str(), "L3");
    }

    #[test]
    fn test_circuit_breaker_default() {
        let cb = CircuitBreaker::default();
        assert_eq!(cb.consecutive_failures, 0);
        assert!(!cb.kill_switch_armed);
        assert!(cb.kill_switch_triggered_at.is_none());
    }

    #[test]
    fn test_loop_registry_entry_roundtrip() {
        let json = r#"{
            "loop_id": "daily-triage",
            "profile": "loop-auto",
            "trigger": {
                "type": "cron",
                "schedule": "0 */6 * * *",
                "timezone": "UTC"
            },
            "skill": "loop-daily-triage",
            "default_safety": "L1"
        }"#;
        let entry: LoopRegistryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.loop_id, "daily-triage");
        assert_eq!(entry.profile, "loop-auto");
        assert_eq!(entry.trigger.trigger_type, "cron");
    }

    #[test]
    fn test_loop_registry_root_roundtrip() {
        let json = r#"{
            "schema_version": "loop-registry-v1",
            "loops": []
        }"#;
        let root: LoopRegistryRoot = serde_json::from_str(json).unwrap();
        assert_eq!(root.schema_version, "loop-registry-v1");
        assert!(root.loops.is_empty());
    }

    #[test]
    fn test_loop_action_record_roundtrip() {
        let json = r#"{
            "schema_version": "loop-action-record-v1",
            "loop_id": "daily-triage",
            "run_id": "run-20260616-0600",
            "action_id": "a1",
            "safety_level": "L2",
            "closeout": {
                "task_id": "a1",
                "summary": "fixed clap deprecation",
                "verification_status": "passed"
            }
        }"#;
        let record: LoopActionRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.loop_id, "daily-triage");
        assert_eq!(record.action_id, "a1");
        assert_eq!(record.closeout["verification_status"], "passed");
    }

    #[test]
    fn test_loop_error_display() {
        let err = LoopError::ProfileMismatch("interactive not schedulable".into());
        assert!(err.to_string().contains("Profile mismatch"));
        let err2 = LoopError::UnknownProfile("unknown-profile".into());
        assert!(err2.to_string().contains("Unknown profile"));
    }
}
