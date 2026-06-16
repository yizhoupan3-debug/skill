//! clap 类型与 execute / trace JSON 载荷（serde）。
use crate::router_self;
use clap::{ArgAction, Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone)]
pub enum RouterCommand {
    Route(RouteCommand),
    Search(SearchCommand),
    Framework {
        #[command(subcommand)]
        command: FrameworkCommand,
    },
    /// Unified host commands: codex, cursor, claude, opencode
    Host {
        #[command(subcommand)]
        command: HostCommand,
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
    /// Loop scheduler: run, status, kill
    Loop {
        #[command(subcommand)]
        command: LoopCommand,
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
pub struct RouteCommand {
    pub query: String,
    #[arg(long)]
    pub host_id: Option<String>,
    #[arg(long, default_value = "route-cli")]
    pub session_id: String,
    #[arg(long, default_value_t = true, action = ArgAction::Set, num_args = 1)]
    pub allow_overlay: bool,
    #[arg(long, default_value_t = true, action = ArgAction::Set, num_args = 1)]
    pub first_turn: bool,
    #[arg(long)]
    pub runtime: Option<PathBuf>,
    #[arg(long)]
    pub manifest: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct SearchCommand {
    pub query: String,
    #[arg(long)]
    pub host_id: Option<String>,
    #[arg(long, default_value_t = 5)]
    pub limit: usize,
    #[arg(long)]
    pub runtime: Option<PathBuf>,
    #[arg(long)]
    pub manifest: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum MaintSubcommand {
    /// Rebuild router-rs, `framework sync-entrypoints`, non-Codex framework installs, verify projections.
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
pub struct CleanHookStateArgs {
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
    /// Only print what would be deleted without actually deleting.
    #[arg(long, default_value = "false")]
    pub dry_run: bool,
    /// Delete files older than N days (default: 7).
    #[arg(long)]
    pub older_than_days: Option<u64>,
}

/// Clean orphan task directories.
#[derive(Args, Debug, Clone)]
pub struct CleanOrphansArgs {
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
    /// Only print what would be deleted without actually deleting.
    #[arg(long, default_value = "false")]
    pub dry_run: bool,
    /// Delete directories older than N days (default: 30).
    #[arg(long)]
    pub older_than_days: Option<u64>,
}

#[derive(Args, Debug, Clone)]
pub struct MaintRootsArgs {
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
    #[arg(long)]
    pub artifact_root: Option<PathBuf>,
    /// Isolated OS account home for user-scope projections (Claude Desktop Application Support, etc.).
    #[arg(long)]
    pub home: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct MaintRepoArgs {
    #[arg(long, alias = "repo-root")]
    pub framework_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct UpdateAuditArgs {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct InstallCodexUserHooksArgs {
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
    #[arg(long)]
    pub codex_home: Option<PathBuf>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum FrameworkCommand {
    /// Repository maintenance / `/update` (replaces retired `scripts/*.sh` wrappers).
    Maint {
        #[command(subcommand)]
        command: MaintSubcommand,
    },
    Snapshot(FrameworkSnapshotCommand),
    /// Human-readable workspace checks (paths, hooks, Codex sync hint).
    Doctor(RepoRootCommand),
    /// Host-neutral host-entrypoint materialization (use `--host-id` to select host; default: all supported).
    #[command(name = "sync-entrypoints", visible_alias = "sync_entrypoints")]
    SyncEntrypoints(SyncEntrypointsCommand),
    PromptCompression(JsonInputCommand),
    Statusline(RepoRootCommand),
    SessionArtifactWrite(JsonInputCommand),
    /// 追加一条外部 hook 验证记录到 `EVIDENCE_INDEX.json`（需连续性已初始化）。
    HookEvidenceAppend(JsonInputCommand),
    Alias(FrameworkAliasCommand),
    /// 只读聚合 `ResolvedTaskView`（调试与未来 hook 消费）；见 `core/core-state/src/task_state.rs`。
    TaskStateResolve(FrameworkTaskStateResolveCommand),
    /// 统一任务账本写分发（envelope：`kind` + `payload`）；见 `core/runtime-core/src/task_command.rs`。
    TaskLedgerDispatch(JsonInputCommand),
    /// 将 GOAL/RFV/Evidence 投影写入 `TASK_STATE.json`（阶段 3）；见 `core/core-state/src/task_state_aggregate.rs`。
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
    /// §5.4: Generate scaffold files for a new host integration.
    Scaffold(ScaffoldCommand),
}

#[derive(Args, Debug, Clone)]
pub struct ScaffoldCommand {
    /// Host ID to scaffold (e.g., "windsurf", "aider")
    #[arg(long = "host-id")]
    pub host_id: String,
    /// Framework root (defaults to cwd)
    #[arg(long = "framework-root")]
    pub framework_root: Option<PathBuf>,
    /// Dry run: print what would be generated without writing files
    #[arg(long, default_value = "false")]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SkillsSubcommand {
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

/// Generic hook command shared across all hook-capable hosts.
#[derive(Args, Debug, Clone)]
pub struct GenericHookCommand {
    /// Hook event name (e.g. PreToolUse, Stop)
    #[arg(long)]
    pub event: String,
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

/// Generic MCP agent command shared across all MCP-native hosts.
#[derive(Args, Debug, Clone)]
pub struct GenericAgentCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

/// Registry-driven host commands.
///
/// Generic `Hook` and `Agent` variants eliminate per-host enum variants.
/// `Codex` retains its own variant due to unique subcommands (HookProjection, InstallHooks).
#[derive(Subcommand, Debug, Clone)]
pub enum HostCommand {
    Codex {
        #[command(subcommand)]
        command: CodexSubcommand,
    },
    /// Run a hook event for any hook-capable host (cursor, claude-code, opencode, codex, mimo).
    Hook {
        host_id: String,
        #[command(flatten)]
        command: GenericHookCommand,
    },
    /// Run an MCP stdio agent loop (opencode, claude-code, mimo).
    Agent {
        host_id: String,
        #[command(flatten)]
        command: GenericAgentCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum CodexSubcommand {
    HookProjection,
    Check(RepoRootCommand),
    Hook(CodexHookCommand),
    HostIntegration(ForwardedArgsCommand),
    InstallHooks(InstallHooksCommand),
}

/// Diagnostic commands (merged: profile, browser)
#[derive(Subcommand, Debug, Clone)]
pub enum DiagnoseCommand {
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
pub enum ProfileSubcommand {
    Emit(ProfilePathCommand),
    Artifacts(ProfilePathCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub enum BrowserSubcommand {
    McpStdio(BrowserMcpStdioCommand),
    ResolveAttachArtifact(BrowserResolveAttachCommand),
}

#[cfg(feature = "codegraph")]
#[derive(Subcommand, Debug, Clone)]
pub enum CodegraphSubcommand {
    McpStdio(CodegraphMcpStdioCommand),
}

#[cfg(feature = "codegraph")]
#[derive(Args, Debug, Clone)]
pub struct CodegraphMcpStdioCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TraceCommand {
    RecordEvent(JsonInputCommand),
    StreamReplay(JsonInputCommand),
    StreamInspect(JsonInputCommand),
    Compact(JsonInputCommand),
    WriteCompactionDelta(JsonInputCommand),
    WriteMetadata(JsonInputCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub enum StorageCommand {
    Runtime(JsonInputCommand),
    CheckpointControlPlane(JsonInputCommand),
    BackendCatalog,
    BackendParity(StorageBackendParityCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub enum MigrateCommand {
    CurrentArtifactClutter(CurrentArtifactClutterCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub enum HookPolicyCommand {
    Evaluate(JsonInputCommand),
    Contract,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SchemaDriftCommand {
    /// Capture harness/task schema fingerprints and write SCHEMA_DRIFT_BASELINE.json.
    Baseline(SchemaDriftRepoArgs),
    /// Compare current repo state against the on-disk baseline for a task.
    Check(SchemaDriftRepoArgs),
    /// Print schema-drift contract (versions, paths, cursor hook sets).
    Contract,
}

#[derive(Args, Debug, Clone)]
pub struct SchemaDriftRepoArgs {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub task_id: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CloseoutCommand {
    /// Evaluate a closeout record JSON payload against the enforcement rules.
    Evaluate(CloseoutEvaluateCommand),
    /// Print the closeout enforcement contract (rules, schema versions, statuses).
    Contract,
}

#[derive(Subcommand, Debug, Clone)]
pub enum LoopCommand {
    /// Run a loop by ID (full execution or dry-run)
    Run(LoopRunCommand),
    /// Show loop status and recent runs
    Status(LoopStatusCommand),
    /// Send kill signal to a running loop
    Kill(LoopKillCommand),
}

#[derive(Args, Debug, Clone)]
pub struct LoopRunCommand {
    /// Loop ID to run (must exist in LOOP_REGISTRY.json)
    #[arg(long)]
    pub loop_id: String,
    /// Dry-run mode: discover and report without executing
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    /// Action timeout in seconds (default: 600)
    #[arg(long, default_value_t = 600)]
    pub timeout: u64,
}

#[derive(Args, Debug, Clone)]
pub struct LoopStatusCommand {
    /// Loop ID to check status for
    #[arg(long)]
    pub loop_id: String,
}

#[derive(Args, Debug, Clone)]
pub struct LoopKillCommand {
    /// Loop ID to send kill signal to
    #[arg(long)]
    pub loop_id: String,
    /// Kill all running loops
    #[arg(long, default_value_t = false)]
    pub all: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum EvalCommand {
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
pub struct EvalRouteCommand {
    /// Path to the eval cases JSON file.
    #[arg(long)]
    pub cases: PathBuf,
    /// Optional path to SKILL_ROUTING_RUNTIME.json (default: skills/SKILL_ROUTING_RUNTIME.json).
    #[arg(long)]
    pub runtime: Option<PathBuf>,
    /// Optional path to SKILL_MANIFEST.json fallback.
    #[arg(long)]
    pub manifest: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct CloseoutEvaluateCommand {
    /// Inline JSON payload describing the closeout record. Mutually exclusive with --record-path.
    #[arg(long)]
    pub input_json: Option<String>,
    /// Path to a JSON file containing the closeout record.
    #[arg(long, conflicts_with = "input_json")]
    pub record_path: Option<PathBuf>,
    /// Optional repository root used to attach task-level evidence context.
    #[arg(long, requires = "task_id")]
    pub repo_root: Option<PathBuf>,
    /// Optional task id used with --repo-root to attach task-level evidence context.
    #[arg(long, requires = "repo_root")]
    pub task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct RepoRootCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

/// `framework sync-entrypoints` arguments.
#[derive(Args, Debug, Clone)]
pub struct SyncEntrypointsCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    /// Select host for entrypoint materialization (e.g. `codex`). Default: all supported hosts.
    #[arg(long)]
    pub host_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct InstallHooksCommand {
    #[arg(long)]
    pub codex_home: Option<PathBuf>,
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    /// Apply changes (default when neither --apply nor --check given).
    #[arg(long)]
    pub apply: bool,
    /// Dry-run: report what would change without writing (mutually exclusive with --apply).
    #[arg(long, conflicts_with = "apply")]
    pub check: bool,
}

#[derive(Args, Debug, Clone)]
pub struct JsonInputCommand {
    #[arg(long)]
    pub input_json: String,
}

#[derive(Args, Debug, Clone)]
pub struct FrameworkSnapshotCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub artifact_source_dir: Option<PathBuf>,
    #[arg(long)]
    pub task_id: Option<String>,
    #[arg(long)]
    pub detail_level: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct FrameworkAliasCommand {
    pub alias: String,
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long, default_value_t = 4)]
    pub max_lines: usize,
    #[arg(long)]
    pub compact: bool,
    #[arg(long)]
    pub host_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct FrameworkTaskStateResolveCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct FrameworkTaskStateAggregateSyncCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub task_id: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct CodexHookCommand {
    /// Hook name (positional, kept for backwards compat)
    #[arg(value_name = "EVENT", required_unless_present = "event")]
    pub name: Option<String>,
    /// Hook name (alias of positional)
    #[arg(long, conflicts_with = "name")]
    pub event: Option<String>,
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct CursorHookCommand {
    #[arg(long)]
    pub event: String,
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ClaudeHookCommand {
    #[arg(long)]
    pub event: String,
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ClaudeHookDirectCommand {
    /// Hook event name (PreToolUse, PostToolUse, Stop, etc.)
    pub event: String,
    /// Override repo root (default: CLAUDE_PROJECT_ROOT env or git rev-parse)
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    /// Path to .env file (default: <repo_root>/.claude/router-rs-hook.env)
    #[arg(long)]
    pub env_file: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ForwardedArgsCommand {
    #[arg(num_args = 1.., trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug, Clone)]
pub struct StorageBackendParityCommand {
    #[arg(long)]
    pub store: Option<String>,
    #[arg(long)]
    pub checkpointer: Option<String>,
    #[arg(long)]
    pub trace: Option<String>,
    #[arg(long)]
    pub state: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct BrowserMcpStdioCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub headless: Option<String>,
    #[arg(long)]
    pub runtime_attach_artifact_path: Option<String>,
    #[arg(long)]
    pub runtime_attach_descriptor_path: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct BrowserResolveAttachCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub search_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ProfilePathCommand {
    #[arg(long)]
    pub framework_profile: PathBuf,
    #[arg(long, default_value_t = false)]
    pub full: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CurrentArtifactClutterCommand {
    pub active_task_id: String,
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

/// Framework contracts command (merged: contract-summary + contracts)
#[derive(Args, Debug, Clone)]
pub struct FrameworkContractsCommand {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    /// Include contract summary (default: false, returns full contracts)
    #[arg(long, default_value_t = false)]
    pub summary: bool,
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
    pub command: Option<RouterCommand>,
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub query: Option<String>,
    #[arg(long, default_value_t = 5)]
    pub limit: usize,
    #[arg(long)]
    pub runtime: Option<PathBuf>,
    #[arg(long)]
    pub manifest: Option<PathBuf>,
    #[arg(long)]
    pub framework_profile: Option<PathBuf>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub stdio_json: bool,
    #[arg(long)]
    pub stdio_max_concurrency: Option<usize>,
    #[arg(long)]
    pub compute_threads: Option<usize>,
}

pub use crate::stdio_payload_types::*;
