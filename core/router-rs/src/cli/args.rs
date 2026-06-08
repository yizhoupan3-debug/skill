//! clap 类型与 execute / trace JSON 载荷（serde）。
use crate::router_self;
use clap::{ArgAction, Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

fn default_true() -> bool {
    true
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum RouterCommand {
    Route(RouteCommand),
    Search(SearchCommand),
    Framework {
        #[command(subcommand)]
        command: FrameworkCommand,
    },
    /// Unified host commands: codex, cursor, claude, antigravity, opencode
    Host {
        #[command(subcommand)]
        command: HostCommand,
    },
    Antigravity {
        #[command(subcommand)]
        command: AntigravitySubcommand,
    },
    #[command(name = "antigravity-app")]
    AntigravityApp {
        #[command(subcommand)]
        command: AntigravitySubcommand,
    },
    Trace {
        #[command(subcommand)]
        command: TraceCommand,
    },
    Storage {
        #[command(subcommand)]
        command: StorageCommand,
    },
    /// Browser MCP utilities (`diagnose browser` is equivalent).
    Browser {
        #[command(subcommand)]
        command: BrowserSubcommand,
    },
    /// CodeGraph MCP utilities (independent process; feature `codegraph`).
    #[cfg(feature = "codegraph")]
    Codegraph {
        #[command(subcommand)]
        command: CodegraphSubcommand,
    },
    /// Diagnostic commands: profile, browser
    Diagnose {
        #[command(subcommand)]
        command: DiagnoseCommand,
    },
    Migrate {
        #[command(subcommand)]
        command: MigrateCommand,
    },
    HookPolicy {
        #[command(subcommand)]
        command: HookPolicyCommand,
    },
    Closeout {
        #[command(subcommand)]
        command: CloseoutCommand,
    },
    Eval {
        #[command(subcommand)]
        command: EvalCommand,
    },
    #[command(name = "schema-drift")]
    SchemaDrift {
        #[command(subcommand)]
        command: SchemaDriftCommand,
    },
    /// Install globally or clean intermediate Cargo artifacts for this crate.
    #[command(name = "self")]
    RouterSelf {
        #[command(subcommand)]
        command: router_self::RouterSelfCommands,
    },
}

#[derive(Args, Debug, Clone)]
pub(crate) struct RouteCommand {
    pub(crate) query: String,
    #[arg(long)]
    pub(crate) host_id: Option<String>,
    #[arg(long, default_value = "route-cli")]
    pub(crate) session_id: String,
    #[arg(long, default_value_t = true, action = ArgAction::Set, num_args = 1)]
    pub(crate) allow_overlay: bool,
    #[arg(long, default_value_t = true, action = ArgAction::Set, num_args = 1)]
    pub(crate) first_turn: bool,
    #[arg(long)]
    pub(crate) runtime: Option<PathBuf>,
    #[arg(long)]
    pub(crate) manifest: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct SearchCommand {
    pub(crate) query: String,
    #[arg(long)]
    pub(crate) host_id: Option<String>,
    #[arg(long, default_value_t = 5)]
    pub(crate) limit: usize,
    #[arg(long)]
    pub(crate) runtime: Option<PathBuf>,
    #[arg(long)]
    pub(crate) manifest: Option<PathBuf>,
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum MaintSubcommand {
    /// Rebuild router-rs, `codex sync`, non-Codex framework installs, verify projections.
    RefreshHostProjections(MaintRootsArgs),
    VerifyCursorHooks(MaintRepoArgs),
    VerifyCodexHooks(MaintRepoArgs),
    /// Refresh → skill-compiler --apply → cargo test → generated-artifacts-status (optional host publish).
    UpdateOneShot(MaintRootsArgs),
    /// Dry-run `/update` repository knowledge/hygiene audit; prints JSON and does not delete files.
    UpdateAudit(UpdateAuditArgs),
    /// Delete every `target/` tree under the repo (skips `.git/`).
    CleanRustTargets(MaintRepoArgs),
    /// Print `export CODEX_HOME=…` / `CURSOR_HOME=…` for repo-local session homes.
    PrintLocalHomes(MaintRepoArgs),
    InstallCodexUserHooks(InstallCodexUserHooksArgs),
    /// Check task pointers, registry consistency, and identify orphan directories.
    ContinuityAudit(MaintRepoArgs),
    /// Clean stale hook-state files older than TTL (default 7 days).
    CleanHookState(CleanHookStateArgs),
    /// Clean orphan task directories not referenced by any pointer or registry.
    CleanOrphans(CleanOrphansArgs),
}

/// Clean hook-state files by age.
#[derive(Args, Debug, Clone)]
pub(crate) struct CleanHookStateArgs {
    #[arg(long)]
    pub(crate) framework_root: Option<PathBuf>,
    /// Only print what would be deleted without actually deleting.
    #[arg(long, default_value = "false")]
    pub(crate) dry_run: bool,
    /// Delete files older than N days (default: 7).
    #[arg(long)]
    pub(crate) older_than_days: Option<u64>,
}

/// Clean orphan task directories.
#[derive(Args, Debug, Clone)]
pub(crate) struct CleanOrphansArgs {
    #[arg(long)]
    pub(crate) framework_root: Option<PathBuf>,
    /// Only print what would be deleted without actually deleting.
    #[arg(long, default_value = "false")]
    pub(crate) dry_run: bool,
    /// Delete directories older than N days (default: 30).
    #[arg(long)]
    pub(crate) older_than_days: Option<u64>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct MaintRootsArgs {
    #[arg(long)]
    pub(crate) framework_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) artifact_root: Option<PathBuf>,
    /// Isolated OS account home for user-scope projections (Claude Desktop Application Support, etc.).
    #[arg(long)]
    pub(crate) home: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct MaintRepoArgs {
    #[arg(long, alias = "repo-root")]
    pub(crate) framework_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct UpdateAuditArgs {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) framework_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct InstallCodexUserHooksArgs {
    #[arg(long)]
    pub(crate) framework_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) codex_home: Option<PathBuf>,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum FrameworkCommand {
    /// Repository maintenance / `/update` (replaces retired `scripts/*.sh` wrappers).
    Maint {
        #[command(subcommand)]
        command: MaintSubcommand,
    },
    Snapshot(FrameworkSnapshotCommand),
    /// Human-readable workspace checks (paths, hooks, Codex sync hint).
    Doctor(RepoRootCommand),
    /// Host-neutral alias for full host-entrypoint materialization (same as `codex sync --repo-root`).
    #[command(name = "sync-entrypoints", visible_alias = "sync_entrypoints")]
    SyncEntrypoints(RepoRootCommand),
    PromptCompression(JsonInputCommand),
    Statusline(RepoRootCommand),
    SessionArtifactWrite(JsonInputCommand),
    /// 追加一条外部 hook 验证记录到 `EVIDENCE_INDEX.json`（需连续性已初始化）。
    HookEvidenceAppend(JsonInputCommand),
    Alias(FrameworkAliasCommand),
    /// 只读聚合 `ResolvedTaskView`（调试与未来 hook 消费）；见 `docs/task_state_unified_resolve.md`。
    TaskStateResolve(FrameworkTaskStateResolveCommand),
    /// 统一任务账本写分发（envelope：`kind` + `payload`）；见 `docs/task_state_unified_resolve.md` §5 阶段 2.5。
    TaskLedgerDispatch(JsonInputCommand),
    /// 将 GOAL/RFV/Evidence 投影写入 `TASK_STATE.json`（阶段 3）；见 `docs/task_state_unified_resolve.md`。
    TaskStateAggregateSync(FrameworkTaskStateAggregateSyncCommand),
    /// Append or summarize task-scoped `STEP_LEDGER.jsonl` recovery records.
    StepLedger(JsonInputCommand),
    HostIntegration(ForwardedArgsCommand),
    /// Print sorted JSON array of `NL_SIGNAL_REGISTRY` signal names (align `NL_ROUTE_ADJUSTMENTS.json` with Rust).
    #[command(hide = true)]
    NlRouteSignalRegistryContract,
    Contracts(FrameworkContractsCommand),
    /// Validate or refresh skill routing artifacts (replaces skill-compiler-rs).
    Skills {
        #[command(subcommand)]
        command: SkillsSubcommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum SkillsSubcommand {
    /// Check SKILL_MANIFEST / SKILL_ROUTING_RUNTIME paths and GSD rows.
    Validate(MaintRepoArgs),
    /// Write derived reports (e.g. SKILL_TIERS.json) then validate.
    Refresh {
        #[arg(long, alias = "framework-root")]
        repo_root: Option<PathBuf>,
        #[arg(long, default_value = "false")]
        write: bool,
        /// Emit minimal PLUGIN/HEALTH companion stubs (not policy-complete; prefer checked-in catalogs).
        #[arg(long, default_value = "false")]
        write_companions: bool,
    },
}

/// Unified host commands (merged: codex, cursor, claude, antigravity-app, opencode)
#[derive(Subcommand, Debug, Clone)]
pub(crate) enum HostCommand {
    Codex {
        #[command(subcommand)]
        command: CodexSubcommand,
    },
    Cursor {
        #[command(subcommand)]
        command: CursorSubcommand,
    },
    Claude {
        #[command(subcommand)]
        command: ClaudeSubcommand,
    },
    Antigravity {
        #[command(subcommand)]
        command: AntigravitySubcommand,
    },
    #[command(name = "antigravity-app")]
    AntigravityAppHost {
        #[command(subcommand)]
        command: AntigravitySubcommand,
    },
    Opencode {
        #[command(subcommand)]
        command: OpenCodeSubcommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CodexSubcommand {
    HookProjection,
    Sync(RepoRootCommand),
    Check(RepoRootCommand),
    Hook(CodexHookCommand),
    HostIntegration(ForwardedArgsCommand),
    InstallHooks(InstallHooksCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CursorSubcommand {
    Hook(CursorHookCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum ClaudeSubcommand {
    Hook(ClaudeHookCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum AntigravitySubcommand {
    /// Run MCP stdio agent loop (skills, evidence, closeout gating; hard block for non-my-light).
    #[command(name = "agent")]
    Agent(AntigravityAgentCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum OpenCodeSubcommand {
    /// Run MCP stdio agent loop (framework snapshot, skill routing, goal/closeout).
    #[command(name = "agent")]
    Agent(OpenCodeAgentCommand),
}

#[derive(Args, Debug, Clone)]
pub(crate) struct OpenCodeAgentCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct AntigravityAgentCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
}

/// Diagnostic commands (merged: profile, browser)
#[derive(Subcommand, Debug, Clone)]
pub(crate) enum DiagnoseCommand {
    Profile {
        #[command(subcommand)]
        command: ProfileSubcommand,
    },
    Browser {
        #[command(subcommand)]
        command: BrowserSubcommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum ProfileSubcommand {
    Emit(ProfilePathCommand),
    Artifacts(ProfilePathCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum BrowserSubcommand {
    McpStdio(BrowserMcpStdioCommand),
    ResolveAttachArtifact(BrowserResolveAttachCommand),
}

#[cfg(feature = "codegraph")]
#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CodegraphSubcommand {
    McpStdio(CodegraphMcpStdioCommand),
}

#[cfg(feature = "codegraph")]
#[derive(Args, Debug, Clone)]
pub(crate) struct CodegraphMcpStdioCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum TraceCommand {
    RecordEvent(JsonInputCommand),
    StreamReplay(JsonInputCommand),
    StreamInspect(JsonInputCommand),
    Compact(JsonInputCommand),
    WriteCompactionDelta(JsonInputCommand),
    WriteMetadata(JsonInputCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum StorageCommand {
    Runtime(JsonInputCommand),
    CheckpointControlPlane(JsonInputCommand),
    BackendCatalog,
    BackendParity(StorageBackendParityCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum MigrateCommand {
    CurrentArtifactClutter(CurrentArtifactClutterCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum HookPolicyCommand {
    Evaluate(JsonInputCommand),
    Contract,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum SchemaDriftCommand {
    /// Capture harness/task schema fingerprints and write SCHEMA_DRIFT_BASELINE.json.
    Baseline(SchemaDriftRepoArgs),
    /// Compare current repo state against the on-disk baseline for a task.
    Check(SchemaDriftRepoArgs),
    /// Print schema-drift contract (versions, paths, cursor hook sets).
    Contract,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct SchemaDriftRepoArgs {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) task_id: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CloseoutCommand {
    /// Evaluate a closeout record JSON payload against the enforcement rules.
    Evaluate(CloseoutEvaluateCommand),
    /// Print the closeout enforcement contract (rules, schema versions, statuses).
    Contract,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum EvalCommand {
    /// Evaluate routing decisions against expected outcomes using live skill records.
    Route(EvalRouteCommand),
    /// Print the eval route contract (metrics, schema versions).
    RouteContract,
    /// Print the harness behavioral eval and failure taxonomy contract.
    HarnessContract,
    /// Run lightweight SKILL.md/tool contract lint against selected skills.
    SkillContractLint(JsonInputCommand),
}

#[derive(Args, Debug, Clone)]
pub(crate) struct EvalRouteCommand {
    /// Path to the eval cases JSON file.
    #[arg(long)]
    pub(crate) cases: PathBuf,
    /// Optional path to SKILL_ROUTING_RUNTIME.json (default: skills/SKILL_ROUTING_RUNTIME.json).
    #[arg(long)]
    pub(crate) runtime: Option<PathBuf>,
    /// Optional path to SKILL_MANIFEST.json fallback.
    #[arg(long)]
    pub(crate) manifest: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CloseoutEvaluateCommand {
    /// Inline JSON payload describing the closeout record. Mutually exclusive with --record-path.
    #[arg(long)]
    pub(crate) input_json: Option<String>,
    /// Path to a JSON file containing the closeout record.
    #[arg(long, conflicts_with = "input_json")]
    pub(crate) record_path: Option<PathBuf>,
    /// Optional repository root used to attach task-level evidence context.
    #[arg(long, requires = "task_id")]
    pub(crate) repo_root: Option<PathBuf>,
    /// Optional task id used with --repo-root to attach task-level evidence context.
    #[arg(long, requires = "repo_root")]
    pub(crate) task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct RepoRootCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct InstallHooksCommand {
    #[arg(long)]
    pub(crate) codex_home: Option<PathBuf>,
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    /// Apply changes (default when neither --apply nor --check given).
    #[arg(long)]
    pub(crate) apply: bool,
    /// Dry-run: report what would change without writing (mutually exclusive with --apply).
    #[arg(long, conflicts_with = "apply")]
    pub(crate) check: bool,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct JsonInputCommand {
    #[arg(long)]
    pub(crate) input_json: String,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct FrameworkSnapshotCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) artifact_source_dir: Option<PathBuf>,
    #[arg(long)]
    pub(crate) task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct FrameworkAliasCommand {
    pub(crate) alias: String,
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long, default_value_t = 4)]
    pub(crate) max_lines: usize,
    #[arg(long)]
    pub(crate) compact: bool,
    #[arg(long)]
    pub(crate) host_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct FrameworkTaskStateResolveCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct FrameworkTaskStateAggregateSyncCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CodexHookCommand {
    /// Hook name (positional, kept for backwards compat)
    #[arg(value_name = "EVENT", required_unless_present = "event")]
    pub(crate) name: Option<String>,
    /// Hook name (alias of positional)
    #[arg(long, conflicts_with = "name")]
    pub(crate) event: Option<String>,
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CursorHookCommand {
    #[arg(long)]
    pub(crate) event: String,
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ClaudeHookCommand {
    #[arg(long)]
    pub(crate) event: String,
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ForwardedArgsCommand {
    #[arg(num_args = 1.., trailing_var_arg = true, allow_hyphen_values = true)]
    pub(crate) args: Vec<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct StorageBackendParityCommand {
    #[arg(long)]
    pub(crate) store: Option<String>,
    #[arg(long)]
    pub(crate) checkpointer: Option<String>,
    #[arg(long)]
    pub(crate) trace: Option<String>,
    #[arg(long)]
    pub(crate) state: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct BrowserMcpStdioCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) headless: Option<String>,
    #[arg(long)]
    pub(crate) runtime_attach_artifact_path: Option<String>,
    #[arg(long)]
    pub(crate) runtime_attach_descriptor_path: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct BrowserResolveAttachCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) search_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ProfilePathCommand {
    #[arg(long)]
    pub(crate) framework_profile: PathBuf,
    #[arg(long, default_value_t = false)]
    pub(crate) full: bool,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct CurrentArtifactClutterCommand {
    pub(crate) active_task_id: String,
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
}

/// Framework contracts command (merged: contract-summary + contracts)
#[derive(Args, Debug, Clone)]
pub(crate) struct FrameworkContractsCommand {
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    /// Include contract summary (default: false, returns full contracts)
    #[arg(long, default_value_t = false)]
    pub(crate) summary: bool,
}

#[derive(Parser, Debug)]
#[command(name = "router-rs")]
#[command(about = "Fast Rust routing core for skill lookup")]
#[command(override_usage = "router-rs <COMMAND>")]
#[command(
    help_template = "{about-section}\nUsage: {usage}\n\nCommands:\n{subcommands}\n\nUse `router-rs <command> --help` for command-specific options.\n"
)]
pub struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<RouterCommand>,
    #[arg(long)]
    pub(crate) repo_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) query: Option<String>,
    #[arg(long, default_value_t = 5)]
    pub(crate) limit: usize,
    #[arg(long)]
    pub(crate) runtime: Option<PathBuf>,
    #[arg(long)]
    pub(crate) manifest: Option<PathBuf>,
    #[arg(long)]
    pub(crate) framework_profile: Option<PathBuf>,
    #[arg(long)]
    pub(crate) json: bool,
    #[arg(long)]
    pub(crate) stdio_json: bool,
    #[arg(long)]
    pub(crate) stdio_max_concurrency: Option<usize>,
    #[arg(long)]
    pub(crate) compute_threads: Option<usize>,
}

pub use crate::stdio_payload_types::*;
