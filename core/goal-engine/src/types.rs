//! Core types for the Loop Architecture.
//!
//! Includes deserialization types for LOOP_REGISTRY.json, runtime phase enums,
//! safety levels, LoopActionRecord, LoopCloseoutAggregate, and related types.

use core_errors::FrameworkError;
use serde::{Deserialize, Serialize};

// ── Phase ──

/// Phase of the Loop Runner state machine.
///
/// ```text
/// PENDING → DISCOVERING → PREFLIGHT → RUNNING → VERIFYING → COMPLETED
///                                                      ↘ ESCALATED → COMPLETED / INTERRUPTED
///                              ↘ PAUSED → RUNNING / INTERRUPTED
/// 任意阶段 → INTERRUPTED（kill/超时）
/// ```
/// Phase transitions:
/// - DISCOVERING: subagent discovers actions
/// - PREFLIGHT: safety checks, budget validation
/// - RUNNING: actions are dispatched and executed by subagents
/// - PAUSED: loop paused via signal, waiting for human input (resume/redirect/kill)
/// - VERIFYING: closeout verification, Quality Gate convergence, anti-drift
/// - COMPLETED: report written, lock released
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopPhase {
    Pending,
    Discovering,
    Preflight,
    Running,
    /// Loop paused via external signal. Waiting for resume/redirect/kill.
    /// Non-terminal: can transition back to Running (resume) or Interrupted (kill).
    Paused,
    Verifying,
    Completed,
    Escalated,
    Interrupted,
}

impl LoopPhase {
    pub fn is_terminal(&self) -> bool {
        matches!(self, LoopPhase::Completed | LoopPhase::Interrupted)
    }

    /// Return the set of valid next phases from this phase.
    /// When `other` is not in this set, the transition is unusual but not blocked.
    ///
    /// Note: `Escalated -> Discovering` is not listed here because it happens
    /// indirectly through the outer `run_loop` restart mechanism. When a
    /// `ResearchEscalation` error is returned, the outer loop re-reads the
    /// loop state and restarts from `Pending` through `Discovering` on the
    /// next iteration. This design keeps the phase machine simple while
    /// allowing auto-restart after research escalation completes.
    pub fn valid_transitions(&self) -> &[LoopPhase] {
        match self {
            LoopPhase::Pending => &[LoopPhase::Discovering, LoopPhase::Interrupted],
            LoopPhase::Discovering => &[LoopPhase::Preflight, LoopPhase::Interrupted],
            LoopPhase::Preflight => &[LoopPhase::Running, LoopPhase::Interrupted],
            LoopPhase::Running => &[
                LoopPhase::Verifying,
                LoopPhase::Paused,
                LoopPhase::Interrupted,
            ],
            LoopPhase::Paused => &[LoopPhase::Running, LoopPhase::Interrupted],
            LoopPhase::Verifying => &[
                LoopPhase::Completed,
                LoopPhase::Escalated,
                LoopPhase::Interrupted,
            ],
            LoopPhase::Completed => &[], // terminal
            LoopPhase::Escalated => &[LoopPhase::Completed, LoopPhase::Interrupted],
            LoopPhase::Interrupted => &[], // terminal
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LoopPhase::Pending => "pending",
            LoopPhase::Discovering => "discovering",
            LoopPhase::Preflight => "preflight",
            LoopPhase::Running => "running",
            LoopPhase::Paused => "paused",
            LoopPhase::Verifying => "verifying",
            LoopPhase::Completed => "completed",
            LoopPhase::Escalated => "escalated",
            LoopPhase::Interrupted => "interrupted",
        }
    }
}

impl std::fmt::Display for LoopPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
    /// L3 unattended：完全自动执行 — 修改 + 验证 + commit + 自动处理异常（不 merge）。
    /// Designed for scenarios where human oversight is unavailable or excessive latency
    /// is unacceptable. Unlike L2, there is no interactive review nudge — the action
    /// proceeds with full autonomy within its safety scope.
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

impl std::fmt::Display for SafetyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── KillSignalAction (v2 multi-signal protocol) ──

/// Action discriminator for the multi-signal kill-switch protocol.
///
/// The signal file format was extended from a binary (present/absent) design
/// to a multi-action JSON schema. Actions are:
/// - `Kill` (default, backward-compatible): terminate the subprocess immediately
/// - `Pause`: pause the running loop, kill the subprocess, wait for human input
/// - `PauseWithFeedback`: like Pause, but carries human feedback text to inject on resume
/// - `Resume`: continue a paused loop by re-spawning the subprocess
/// - `Redirect`: change the goal of a paused loop and re-spawn the subprocess
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KillSignalAction {
    #[serde(rename = "kill")]
    Kill,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "pause_with_feedback")]
    PauseWithFeedback {
        /// Human feedback text to inject when the action resumes.
        feedback: String,
    },
    #[serde(rename = "resume")]
    Resume,
    #[serde(rename = "redirect")]
    Redirect {
        /// New goal text description to use when re-spawning the subprocess.
        new_goal: String,
    },
}

impl Default for KillSignalAction {
    fn default() -> Self {
        KillSignalAction::Kill
    }
}

impl KillSignalAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            KillSignalAction::Kill => "kill",
            KillSignalAction::Pause => "pause",
            KillSignalAction::PauseWithFeedback { .. } => "pause_with_feedback",
            KillSignalAction::Resume => "resume",
            KillSignalAction::Redirect { .. } => "redirect",
        }
    }
}

/// Extended kill-signal payload for the multi-action protocol (v2).
///
/// Backward-compatible: when `action` is absent or the file uses the old format
/// (`{loop_id, armed_at, armed_at_iso}` only), `action` defaults to `Kill`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSignalPayload {
    #[serde(default = "default_signal_schema_v2")]
    pub schema_version: String,
    pub loop_id: String,
    /// When present, signals the action to take. Absence (default = Kill)
    /// ensures backward compatibility with old-format signal files.
    #[serde(default)]
    pub action: KillSignalAction,
    /// Optional action_id context for pause/redirect signals.
    #[serde(default)]
    pub action_id: Option<String>,
    pub armed_at: u64,
    pub armed_at_iso: String,
}

fn default_signal_schema_v2() -> String {
    "loop-signal-v2".to_string()
}

impl KillSignalPayload {
    /// Create a new Kill signal payload (backward-compatible).
    pub fn new_kill(loop_id: &str) -> Self {
        KillSignalPayload {
            schema_version: default_signal_schema_v2(),
            loop_id: loop_id.to_string(),
            action: KillSignalAction::Kill,
            action_id: None,
            armed_at: epoch_now(),
            armed_at_iso: framework_core::time::now_iso(),
        }
    }

    /// Create a new Pause signal payload.
    pub fn new_pause(loop_id: &str, action_id: impl Into<String>) -> Self {
        KillSignalPayload {
            schema_version: default_signal_schema_v2(),
            loop_id: loop_id.to_string(),
            action: KillSignalAction::Pause,
            action_id: Some(action_id.into()),
            armed_at: epoch_now(),
            armed_at_iso: framework_core::time::now_iso(),
        }
    }

    /// Create a new PauseWithFeedback signal payload.
    pub fn new_pause_with_feedback(
        loop_id: &str,
        action_id: impl Into<String>,
        feedback: impl Into<String>,
    ) -> Self {
        KillSignalPayload {
            schema_version: default_signal_schema_v2(),
            loop_id: loop_id.to_string(),
            action: KillSignalAction::PauseWithFeedback {
                feedback: feedback.into(),
            },
            action_id: Some(action_id.into()),
            armed_at: epoch_now(),
            armed_at_iso: framework_core::time::now_iso(),
        }
    }

    /// Create a new Resume signal payload.
    pub fn new_resume(loop_id: &str) -> Self {
        KillSignalPayload {
            schema_version: default_signal_schema_v2(),
            loop_id: loop_id.to_string(),
            action: KillSignalAction::Resume,
            action_id: None,
            armed_at: epoch_now(),
            armed_at_iso: framework_core::time::now_iso(),
        }
    }

    /// Create a new Redirect signal payload.
    pub fn new_redirect(loop_id: &str, new_goal: impl Into<String>) -> Self {
        KillSignalPayload {
            schema_version: default_signal_schema_v2(),
            loop_id: loop_id.to_string(),
            action: KillSignalAction::Redirect {
                new_goal: new_goal.into(),
            },
            action_id: None,
            armed_at: epoch_now(),
            armed_at_iso: framework_core::time::now_iso(),
        }
    }
}

/// Return the current epoch seconds.
fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Profile Config ──

/// Profile configuration snapshot loaded from RUNTIME_REGISTRY.json during PREFLIGHT.
/// Determines loop behaviour (scheduling, closeout enforcement, review gating, budgets).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopProfileConfig {
    /// profile 标识符（"loop-auto" / "interactive"）
    pub profile: String,
    /// 是否可被循环调度器调度
    pub loop_capable: bool,
    /// closeout 模式（hard-block / advisory）
    pub closeout_mode: String,
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

    /// Whether the profile supports interactive pause/resume/redirect.
    #[serde(default)]
    pub interactive_capable: bool,

    /// Maximum seconds a loop can stay paused before auto-timeout.
    #[serde(default)]
    pub pause_timeout_secs: Option<u64>,
}

/// Stripped-down profile for Phase 1 typed validation.
/// Used only to detect type errors; unknown fields are tolerated
/// (this struct does NOT have deny_unknown_fields). Fields from
/// the JSON that don't match the expected type will produce a warning.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Phase1Profile {
    loop_capable: Option<bool>,
    closeout_mode: Option<String>,
    review_gate: Option<String>,
    spawn_first_nudge: Option<bool>,
    interactive_capable: Option<bool>,
    pause_timeout_secs: Option<u64>,
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

        // ── Phase 1: type validation for known fields ──
        // This detects type errors (e.g. loop_capable: "true" as string instead of bool)
        // that the manual extraction path below would silently ignore.
        if let Err(e) = serde_json::from_value::<Phase1Profile>(profile_val.clone()) {
            tracing::warn!(
                "RUNTIME_REGISTRY profile '{}': field type mismatch: {}. \
                 Falling back to manual extraction.",
                profile_name, e
            );
        }

        // ── Phase 2: backward-compatible manual extraction ──
        let loop_capable = profile_val
            .get("loop_capable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let closeout_mode = profile_val
            .get("closeout_mode")
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
            closeout_mode,
            review_gate,
            spawn_first_nudge,
            cost_budget,
            escalation,
            interactive_capable: profile_val
                .get("interactive_capable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            pause_timeout_secs: profile_val
                .get("pause_timeout_secs")
                .and_then(|v| v.as_u64()),
        })
    }

    pub fn is_hard_block(&self) -> bool {
        self.closeout_mode == "hard-block"
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
    /// When true, the runner fires the two-stage quality gate (anti-fraud + checker chain)
    /// during the Verifying phase. If the QG gate blocks, the aggregate is downgraded to "fail".
    #[serde(default)]
    pub verify_quality_gate: Option<bool>,
    /// When true, the runner fires the closeout gate readiness check during the Verifying phase.
    /// Results are advisory (no blocking).
    #[serde(default)]
    pub verify_closeout_gate: Option<bool>,
    /// Pre-defined static action list. When present, the runner uses these actions
    /// directly instead of spawning a subagent for discovery.
    #[serde(default)]
    pub static_actions: Option<Vec<LoopAction>>,
    /// Subagent IPC protocol version. "v0" (natural language handoff via -p) by default.
    /// When "v1", the runner passes structured JSON input via --input <path> and
    /// expects structured JSON output via --output <path>.
    #[serde(default)]
    pub subagent_protocol: Option<String>,
}

/// Research configuration for barrier escalation (§19.9).
/// Used by research-aware loops to define escalation thresholds, auto-resume behaviour, and time limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Freshness window for barrier reports, in minutes.
    /// Reports older than this are considered stale and ignored.
    /// Default: 60 minutes (matching the previous hardcoded 3600s).
    #[serde(default = "default_freshness_window_min")]
    pub freshness_window_min: u32,
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
fn default_freshness_window_min() -> u32 {
    60
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
#[serde(deny_unknown_fields)]
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
    /// IDs of actions whose outputs should be passed as inputs to this action.
    /// Populated by the discovery phase; outputs are cached in `action_outputs/`.
    #[serde(default)]
    pub consumed_action_ids: Vec<String>,
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
    /// Quality gate blockers when the gate blocked (overall_status = "fail").
    /// Empty when the gate passed or was not evaluated.
    #[serde(default)]
    pub qg_blockers: Vec<String>,
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
    /// Timestamp of the last amend that reset the original_goal_snapshot.
    /// When GOAL_STATE's `amended_at` is newer than this value, the snapshot
    /// is refreshed to reflect user-authorized goal changes (P1-001 fix).
    #[serde(default)]
    pub last_amended_at: Option<String>,
}

impl Default for AntiDriftState {
    fn default() -> Self {
        Self {
            review_cycle_count: 0,
            check_interval: 3,
            original_goal_snapshot: None,
            last_drift_check: None,
            drift_check_history: Vec::new(),
            last_amended_at: None,
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

// ── Subagent IPC Protocol ──

/// Subagent IPC protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentProtocol {
    /// V0: natural language handoff via -p argument + file-based closeout.
    V0,
    /// V1: structured JSON input via --input <path> + structured JSON output via --output <path>.
    V1,
}

impl SubagentProtocol {
    /// Resolve the protocol from a registry entry or env var.
    /// Priority: entry field > env var > default V0.
    /// The env var is cached in a OnceLock on first access for test determinism.
    pub fn resolve(entry_protocol: Option<&str>) -> Self {
        match entry_protocol {
            Some("v1") | Some("V1") => SubagentProtocol::V1,
            _ => {
                if cached_subagent_protocol_env_is_v1() {
                    SubagentProtocol::V1
                } else {
                    SubagentProtocol::V0
                }
            }
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SubagentProtocol::V0 => "v0",
            SubagentProtocol::V1 => "v1",
        }
    }
}

impl Default for SubagentProtocol {
    fn default() -> Self {
        SubagentProtocol::V0
    }
}

/// Cached env var check: true when `ROUTER_RS_SUBAGENT_PROTOCOL=v1`.
/// Cached on first access via OnceLock for test determinism.
/// In test builds, reads the env var each time so tests can inject values.
fn cached_subagent_protocol_env_is_v1() -> bool {
    #[cfg(not(test))]
    {
        static CACHED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *CACHED.get_or_init(|| {
            std::env::var("ROUTER_RS_SUBAGENT_PROTOCOL")
                .ok()
                .as_deref()
                == Some("v1")
        })
    }
    #[cfg(test)]
    {
        std::env::var("ROUTER_RS_SUBAGENT_PROTOCOL")
            .ok()
            .as_deref()
            == Some("v1")
    }
}

/// Schema version for SubagentInput JSON files.
pub const SUBAGENT_INPUT_SCHEMA_VERSION: &str = "subagent-input-v1";

/// Schema version for SubagentOutput JSON files.
pub const SUBAGENT_OUTPUT_SCHEMA_VERSION: &str = "subagent-output-v1";

/// Structured input for a subagent action execution (V1 protocol).
///
/// Serialized to JSON and written to a temp file at `artifacts/loop/{loop_id}/input/{run_id}-{action_id}.json`.
/// The subagent reads this file and writes its output to the corresponding output path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubagentInput {
    pub schema_version: String,
    pub loop_id: String,
    pub run_id: String,
    pub action: LoopAction,
    pub repo_root: String,
    pub closeout_dir: String,
    pub evidence_dir: String,
    pub kill_signal_path: String,
    pub output_path: String,
    #[serde(default)]
    pub consumed_inputs: Vec<ConsumedInputRef>,
}

/// Reference to a prior action's output that this action consumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumedInputRef {
    pub action_id: String,
    pub path: String,
}

/// Structured output from a subagent action execution (V1 protocol).
///
/// Written by the subagent to the path specified in `SubagentInput.output_path`.
/// When the parent detects this file, it parses it for inline closeout data,
/// skipping the file-based closeout read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubagentOutput {
    pub schema_version: String,
    pub action_id: String,
    pub success: bool,
    /// Inline closeout record. When present, the parent reads it directly
    /// instead of loading closeout JSON from the closeout directory.
    #[serde(default)]
    pub closeout: Option<serde_json::Value>,
    /// Optional error message when success=false.
    #[serde(default)]
    pub error: Option<String>,
}

// ── Pause State ──

/// Pause state persisted when a loop action is paused via multi-signal protocol.
///
/// Written by `poll_subprocess` when a Pause / PauseWithFeedback signal is
/// detected, cleared on resume or redirect. The pause-wait loop reads this
/// state to determine which action to re-spawn and whether feedback was injected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PauseState {
    pub schema_version: String,
    pub loop_id: String,
    pub run_id: String,
    pub action_id: String,
    /// The full LoopAction that was executing when paused (for re-dispatch).
    pub action: LoopAction,
    /// The original handoff text (V0 protocol) for re-sending on resume.
    pub handoff: String,
    /// Injected human feedback text, if any (from PauseWithFeedback signal).
    #[serde(default)]
    pub feedback: Option<String>,
    /// ISO timestamp of pause creation.
    pub created_at: String,
    /// Cached subagent binary path for re-spawn.
    pub agent_binary: String,
    /// Remaining deadline seconds saved at pause time, so resume deadline
    /// continues from where it left off rather than starting fresh.
    #[serde(default)]
    pub deadline_remaining_secs: Option<u64>,
}

/// Schema version for PauseState JSON files.
pub const PAUSE_STATE_SCHEMA_VERSION: &str = "loop-pause-state-v1";

/// Error type for goal-engine operations, covering profile mismatches, kill signals,
/// timeouts, spawn failures, serialization errors, action failures, research escalations,
/// and pause/redirect/cancel signals.
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

    /// Pause signal detected during subprocess execution.
    /// The subprocess was killed and PauseState was persisted.
    /// Carries the signal description.
    #[error("Pause signaled: {0}")]
    PauseSignaled(String),

    /// Action paused with full pause state persisted.
    /// The run loop should enter pause-wait.
    #[error("Action paused: {0}")]
    Paused(String),

    /// Kill signal received during pause-wait loop.
    #[error("Paused then killed: {0}")]
    PauseKilled(String),

    /// Redirect signal received — action should re-spawn with new goal.
    #[error("Action redirected: {0}")]
    Redirected(String),

    /// Invalid phase transition — state machine invariant violated.
    #[error("Invalid phase transition: {0}")]
    PhaseTransition(String),
}

impl From<serde_json::Error> for LoopError {
    fn from(e: serde_json::Error) -> Self {
        LoopError::Serde(e.to_string())
    }
}

impl From<LoopError> for FrameworkError {
    fn from(e: LoopError) -> Self {
        match e {
            LoopError::ProfileMismatch(msg)
            | LoopError::UnknownProfile(msg)
            | LoopError::ActionFailed(msg) => FrameworkError::validation(msg),
            LoopError::Timeout(secs) => {
                FrameworkError::validation(format!("Timeout after {secs}s"))
            }
            LoopError::KillSignaled(msg) | LoopError::ResearchEscalation(msg) => {
                FrameworkError::hook(msg)
            }
            LoopError::SpawnFailed(msg) | LoopError::Io(msg) => {
                FrameworkError::Io(std::io::Error::other(msg))
            }
            LoopError::Serde(msg) => FrameworkError::validation(format!("serde: {msg}")),
            LoopError::BudgetExceeded(msg) => FrameworkError::config(msg),
            LoopError::PauseSignaled(msg)
            | LoopError::Paused(msg)
            | LoopError::PauseKilled(msg)
            | LoopError::Redirected(msg)
            | LoopError::PhaseTransition(msg) => FrameworkError::hook(msg),
        }
    }
}

// ── Tests ──

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_phase_terminal() {
        assert!(LoopPhase::Completed.is_terminal());
        assert!(
            !LoopPhase::Escalated.is_terminal(),
            "Escalated is no longer terminal: auto-resume restarts the loop at Discovering"
        );
        assert!(LoopPhase::Interrupted.is_terminal());
        assert!(!LoopPhase::Pending.is_terminal());
        assert!(!LoopPhase::Running.is_terminal());
        assert!(!LoopPhase::Paused.is_terminal(), "Paused is non-terminal");
    }

    #[test]
    fn test_loop_phase_as_str() {
        assert_eq!(LoopPhase::Pending.as_str(), "pending");
        assert_eq!(LoopPhase::Verifying.as_str(), "verifying");
        assert_eq!(LoopPhase::Interrupted.as_str(), "interrupted");
        assert_eq!(LoopPhase::Paused.as_str(), "paused");
    }

    #[test]
    fn test_loop_phase_transitions_include_paused() {
        let running_trans = LoopPhase::Running.valid_transitions();
        assert!(
            running_trans.contains(&LoopPhase::Paused),
            "Running must allow transition to Paused"
        );
        let paused_trans = LoopPhase::Paused.valid_transitions();
        assert!(
            paused_trans.contains(&LoopPhase::Running),
            "Paused must allow transition back to Running"
        );
        assert!(
            paused_trans.contains(&LoopPhase::Interrupted),
            "Paused must allow transition to Interrupted (kill)"
        );
    }

    // ── PauseState tests ──

    #[test]
    fn test_pause_state_roundtrip() {
        let state = PauseState {
            schema_version: PAUSE_STATE_SCHEMA_VERSION.to_string(),
            loop_id: "test-loop".to_string(),
            run_id: "run-1".to_string(),
            action_id: "fix-1".to_string(),
            action: LoopAction {
                action_id: "fix-1".to_string(),
                action_type: "fix".to_string(),
                scope_paths: vec!["src/main.rs".to_string()],
                safety: "L2".to_string(),
                description: Some("fix deprecation".to_string()),
                consumed_action_ids: Vec::new(),
            },
            handoff: "## Objective\nfix the issue".to_string(),
            feedback: Some("check edge case".to_string()),
            created_at: "2026-06-30T12:00:00Z".to_string(),
            agent_binary: "/usr/bin/subagent".to_string(),
            deadline_remaining_secs: Some(300),
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: PauseState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.schema_version, PAUSE_STATE_SCHEMA_VERSION);
        assert_eq!(parsed.loop_id, "test-loop");
        assert_eq!(parsed.action_id, "fix-1");
        assert_eq!(parsed.feedback.as_deref(), Some("check edge case"));
        assert_eq!(parsed.deadline_remaining_secs, Some(300));
        assert_eq!(parsed.action.scope_paths, vec!["src/main.rs".to_string()]);
        assert_eq!(parsed.agent_binary, "/usr/bin/subagent");
    }

    #[test]
    fn test_pause_state_default_feedback_is_none() {
        let state = PauseState {
            schema_version: PAUSE_STATE_SCHEMA_VERSION.to_string(),
            loop_id: "test".to_string(),
            run_id: "run-1".to_string(),
            action_id: "a1".to_string(),
            action: LoopAction {
                action_id: "a1".to_string(),
                action_type: "fix".to_string(),
                scope_paths: Vec::new(),
                safety: "L2".to_string(),
                description: None,
                consumed_action_ids: Vec::new(),
            },
            handoff: "handoff".to_string(),
            feedback: None,
            created_at: "2026-06-30T12:00:00Z".to_string(),
            agent_binary: "/usr/bin/subagent".to_string(),
            deadline_remaining_secs: None,
        };
        assert!(state.feedback.is_none());
        assert!(state.deadline_remaining_secs.is_none());
    }

    #[test]
    fn test_pause_state_deny_unknown_fields() {
        let json = r#"{
            "schema_version": "loop-pause-state-v1",
            "loop_id": "test",
            "run_id": "run-1",
            "action_id": "a1",
            "action": {
                "action_id": "a1",
                "type": "fix",
                "scope_paths": [],
                "safety": "L2"
            },
            "handoff": "h",
            "created_at": "2026-06-30T12:00:00Z",
            "agent_binary": "/bin/subagent",
            "unknown_field": "should_fail"
        }"#;
        let result: Result<PauseState, _> = serde_json::from_str(json);
        assert!(result.is_err(), "PauseState must reject unknown fields");
    }

    // ── New LoopError tests ──

    #[test]
    fn test_loop_error_new_variants_display() {
        let e1 = LoopError::PauseSignaled("pause received".into());
        assert!(e1.to_string().contains("Pause signaled"));

        let e2 = LoopError::Paused("action paused".into());
        assert!(e2.to_string().contains("Action paused"));

        let e3 = LoopError::PauseKilled("killed during pause".into());
        assert!(e3.to_string().contains("Paused then killed"));

        let e4 = LoopError::Redirected("new goal".into());
        assert!(e4.to_string().contains("Action redirected"));
    }

    #[test]
    fn test_pause_error_framework_convert() {
        let e = LoopError::PauseSignaled("pause".into());
        let fe: FrameworkError = e.into();
        assert!(fe.to_string().contains("Hook"), "Hook error should display as 'Hook error: ...'");

        let e2 = LoopError::Paused("paused".into());
        let fe2: FrameworkError = e2.into();
        assert!(fe2.to_string().contains("Hook"), "Paused error should convert to Hook error");
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

    #[test]
    fn test_subagent_protocol_default() {
        assert_eq!(SubagentProtocol::default(), SubagentProtocol::V0);
        assert_eq!(SubagentProtocol::V0.as_str(), "v0");
        assert_eq!(SubagentProtocol::V1.as_str(), "v1");
    }

    #[test]
    fn test_subagent_protocol_resolve() {
        assert_eq!(SubagentProtocol::resolve(None), SubagentProtocol::V0);
        assert_eq!(SubagentProtocol::resolve(Some("v1")), SubagentProtocol::V1);
        assert_eq!(SubagentProtocol::resolve(Some("V1")), SubagentProtocol::V1);
        assert_eq!(SubagentProtocol::resolve(Some("v0")), SubagentProtocol::V0);
        assert_eq!(SubagentProtocol::resolve(Some("unknown")), SubagentProtocol::V0);
    }

    #[test]
    fn test_subagent_input_roundtrip() {
        let input = SubagentInput {
            schema_version: SUBAGENT_INPUT_SCHEMA_VERSION.to_string(),
            loop_id: "test-loop".to_string(),
            run_id: "run-1".to_string(),
            action: LoopAction {
                action_id: "fix-1".to_string(),
                action_type: "fix".to_string(),
                scope_paths: vec!["src/main.rs".to_string()],
                safety: "L2".to_string(),
                description: Some("fix deprecation".to_string()),
                consumed_action_ids: Vec::new(),
            },
            repo_root: "/tmp/repo".to_string(),
            closeout_dir: "/tmp/repo/artifacts/closeout".to_string(),
            evidence_dir: "/tmp/repo/artifacts/evidence".to_string(),
            kill_signal_path: "/tmp/repo/.loop-kill/test-loop".to_string(),
            output_path: "/tmp/repo/artifacts/loop/test-loop/output/run-1-fix-1.json".to_string(),
            consumed_inputs: Vec::new(),
        };
        let json = serde_json::to_string(&input).unwrap();
        let parsed: SubagentInput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.schema_version, SUBAGENT_INPUT_SCHEMA_VERSION);
        assert_eq!(parsed.action.action_id, "fix-1");
        assert!(parsed.consumed_inputs.is_empty());
    }

    #[test]
    fn test_subagent_output_roundtrip() {
        let output = SubagentOutput {
            schema_version: SUBAGENT_OUTPUT_SCHEMA_VERSION.to_string(),
            action_id: "fix-1".to_string(),
            success: true,
            closeout: Some(serde_json::json!({
                "task_id": "fix-1",
                "summary": "fixed deprecation",
                "verification_status": "passed"
            })),
            error: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        let parsed: SubagentOutput = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert!(parsed.closeout.is_some());
        assert!(parsed.error.is_none());
    }

    #[test]
    fn test_subagent_output_deny_unknown_fields() {
        let json = r#"{
            "schema_version": "subagent-output-v1",
            "action_id": "a1",
            "success": false,
            "error": "something went wrong",
            "unknown_field": "should be rejected"
        }"#;
        let result: Result<SubagentOutput, _> = serde_json::from_str(json);
        assert!(result.is_err(), "unknown fields must be denied");
    }

    // ── Issue 8: Phase1Profile type validation ────────────────────────────

    #[test]
    fn phase1_profile_valid_types() {
        // Correct field types should succeed deserialization
        let json = serde_json::json!({
            "loop_capable": true,
            "closeout_mode": "hard-block",
            "review_gate": "mandatory",
            "spawn_first_nudge": true,
            "interactive_capable": true,
            "pause_timeout_secs": 3600,
            "extra_field_ok": "since Phase1Profile doesn't deny_unknown_fields"
        });
        let result: Result<super::Phase1Profile, _> = serde_json::from_value(json);
        assert!(result.is_ok(), "valid types should deserialize Phase1Profile: {:?}", result.err());
    }

    #[test]
    fn phase1_profile_type_mismatch() {
        // Wrong field type should fail deserialization
        let json = serde_json::json!({
            "loop_capable": "true",   // string instead of bool
            "closeout_mode": "hard-block",
            "review_gate": "mandatory",
            "spawn_first_nudge": 1,   // number instead of bool
        });
        let result: Result<super::Phase1Profile, _> = serde_json::from_value(json);
        assert!(result.is_err(), "type mismatch should fail Phase1Profile deserialization");
    }

    #[test]
    fn phase1_profile_extra_fields_tolerated() {
        // Extra fields not in Phase1Profile should be tolerated
        let json = serde_json::json!({
            "loop_capable": true,
            "closeout_mode": "hard-block",
            "review_gate": "mandatory",
            "spawn_first_nudge": true,
            "unknown_future_field": "should be OK"
        });
        let result: Result<super::Phase1Profile, _> = serde_json::from_value(json);
        assert!(result.is_ok(), "extra unknown fields must be tolerated");
    }
}
