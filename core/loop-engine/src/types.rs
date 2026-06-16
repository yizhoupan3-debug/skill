//! Loop Architecture 核心类型。
//!
//! 包含 LOOP_REGISTRY.json 反序列化类型、运行时阶段枚举、
//! 安全级别、LoopActionRecord、LoopCloseoutAggregate 等。

use serde::{Deserialize, Serialize};

// ── Phase ──

/// Loop Runner 状态机阶段。
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
        matches!(self, LoopPhase::Completed | LoopPhase::Escalated | LoopPhase::Interrupted)
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

/// Scope-based safety level（§6.1）。
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

/// Profile 配置快照，Loop Runner 在 PREFLIGHT 阶段从 RUNTIME_REGISTRY.json 加载。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopProfileConfig {
    /// profile 标识符（"loop-auto" / "interactive" / "my-light"）
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
        let path = repo_root.join("configs").join("framework").join("RUNTIME_REGISTRY.json");
        let raw = std::fs::read_to_string(&path).ok()?;
        let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let profiles = val.get("lifecycle_profiles")?;
        let profile_val = profiles.get(profile_name)?;

        let loop_capable = profile_val.get("loop_capable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let closeout_enforcement = profile_val.get("closeout_enforcement")
            .and_then(|v| v.as_str())
            .unwrap_or("advisory")
            .to_string();
        let review_gate = profile_val.get("review_gate")
            .and_then(|v| v.as_str())
            .unwrap_or("suppressed")
            .to_string();
        let spawn_first_nudge = profile_val.get("spawn_first_nudge")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let cost_budget = profile_val.get("cost_budget").and_then(|v| {
            serde_json::from_value(v.clone()).ok()
        });
        let escalation = profile_val.get("escalation").and_then(|v| {
            serde_json::from_value(v.clone()).ok()
        });

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

/// Token 预算软限制。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBudgetConfig {
    /// 单次运行的 token 软上限
    #[serde(default)]
    pub tokens_per_run: Option<u64>,
    /// 每日 token 软上限
    #[serde(default)]
    pub daily_tokens: Option<u64>,
}

/// 升级策略。
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

/// LOOP_REGISTRY.json 中的单条循环注册项（§4.1）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopRegistryEntry {
    pub loop_id: String,
    pub profile: String,
    pub trigger: LoopTriggerConfig,
    pub skill: Option<String>,
    #[serde(default)]
    pub scope_based_safety: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub default_safety: Option<String>,
    #[serde(default)]
    pub scope_conflict_resolution: Option<String>,
    #[serde(default)]
    pub cost_budget: Option<CostBudgetConfig>,
    #[serde(default)]
    pub notification: Option<serde_json::Value>,
}

/// 循环触发器配置。
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

/// LOOP_REGISTRY.json 根结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopRegistryRoot {
    pub schema_version: String,
    pub loops: Vec<LoopRegistryEntry>,
}

// ── Loop Action ──

/// 单次循环中分配的一个 action（由 DISCOVERING 阶段产出）。
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

/// 每个 action 的 closeout record（§5.3）。
/// 嵌入现有 `CloseoutRecord`，不修改现有类型。
///
/// 写入路径：`artifacts/closeout/<action-id>.json`
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

/// 单次运行的 closeout 聚合结果（§5.4）。
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

/// 聚合中的单个 action 条目。
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

// ── Loop Run State ──

/// LOOP_RUN_STATE.json 运行时持久化结构（§5.2）。
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
    pub last_refreshed_at: String,
}

/// 当前运行快照。
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

/// DISCOVERING 阶段结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryResult {
    pub actions_found: u32,
    pub actions: Vec<LoopAction>,
}

/// 未消耗的 finding（留给下一轮 DISCOVERING）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnconsumedFinding {
    pub finding_hash: String,
    pub source_action: String,
    pub finding: String,
}

/// 历史运行记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunHistoryEntry {
    pub run_id: String,
    pub phase: String,
    pub result: String,
}

/// 断路器状态（§6.3）。
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

/// loop-engine 错误类型。
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
        assert!(LoopPhase::Escalated.is_terminal());
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
        let err2 = LoopError::UnknownProfile("my-light".into());
        assert!(err2.to_string().contains("Unknown profile"));
    }
}
