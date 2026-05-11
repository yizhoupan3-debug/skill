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
    Codex {
        #[command(subcommand)]
        command: CodexCommand,
    },
    Cursor {
        #[command(subcommand)]
        command: CursorCommand,
    },
    Claude {
        #[command(subcommand)]
        command: ClaudeCommand,
    },
    Trace {
        #[command(subcommand)]
        command: TraceCommand,
    },
    Storage {
        #[command(subcommand)]
        command: StorageCommand,
    },
    Browser {
        #[command(subcommand)]
        command: BrowserCommand,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
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
}

#[derive(Args, Debug, Clone)]
pub(crate) struct MaintRootsArgs {
    #[arg(long)]
    pub(crate) framework_root: Option<PathBuf>,
    #[arg(long)]
    pub(crate) artifact_root: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct MaintRepoArgs {
    #[arg(long)]
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
    ContractSummary(RepoRootCommand),
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
    Contracts,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CodexCommand {
    HookProjection,
    Sync(RepoRootCommand),
    Check(RepoRootCommand),
    Hook(CodexHookCommand),
    HostIntegration(ForwardedArgsCommand),
    InstallHooks(InstallHooksCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CursorCommand {
    Hook(CursorHookCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum ClaudeCommand {
    Hook(ClaudeHookCommand),
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
pub(crate) enum BrowserCommand {
    McpStdio(BrowserMcpStdioCommand),
    ResolveAttachArtifact(BrowserResolveAttachCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum ProfileCommand {
    Emit(ProfilePathCommand),
    Artifacts(ProfilePathCommand),
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

#[derive(Parser, Debug)]
#[command(name = "router-rs")]
#[command(about = "Fast Rust routing core for skill lookup")]
#[command(override_usage = "router-rs <COMMAND>")]
#[command(
    help_template = "{about-section}\nUsage: {usage}\n\nCommands:\n{subcommands}\n\nUse `router-rs <command> --help` for command-specific options.\n"
)]
pub(crate) struct Cli {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecuteRequestPayload {
    pub(crate) schema_version: String,
    pub(crate) task: String,
    pub(crate) session_id: String,
    pub(crate) user_id: String,
    pub(crate) selected_skill: String,
    pub(crate) overlay_skill: Option<String>,
    pub(crate) layer: String,
    pub(crate) route_engine: Option<String>,
    pub(crate) diagnostic_route_mode: Option<String>,
    pub(crate) reasons: Vec<String>,
    pub(crate) prompt_preview: Option<String>,
    pub(crate) dry_run: bool,
    pub(crate) trace_event_count: usize,
    pub(crate) trace_output_path: Option<String>,
    pub(crate) default_output_tokens: usize,
    #[serde(default)]
    pub(crate) research_mode: Option<String>,
    #[serde(default)]
    pub(crate) execution_protocol: Option<String>,
    #[serde(default)]
    pub(crate) verification_required: Option<bool>,
    #[serde(default)]
    pub(crate) evidence_required: Option<bool>,
    pub(crate) model_id: String,
    pub(crate) aggregator_base_url: String,
    pub(crate) aggregator_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecuteUsagePayload {
    pub(crate) input_tokens: usize,
    pub(crate) output_tokens: usize,
    pub(crate) total_tokens: usize,
    pub(crate) mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecuteResponsePayload {
    pub(crate) execution_schema_version: String,
    pub(crate) authority: String,
    pub(crate) session_id: String,
    pub(crate) user_id: String,
    pub(crate) skill: String,
    pub(crate) overlay: Option<String>,
    pub(crate) live_run: bool,
    pub(crate) content: String,
    pub(crate) usage: ExecuteUsagePayload,
    pub(crate) prompt_preview: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackgroundControlRequestPayload {
    pub(crate) schema_version: String,
    pub(crate) operation: String,
    pub(crate) multitask_strategy: Option<String>,
    pub(crate) current_status: Option<String>,
    pub(crate) task_active: Option<bool>,
    pub(crate) task_done: Option<bool>,
    pub(crate) active_job_count: Option<usize>,
    pub(crate) capacity_limit: Option<usize>,
    pub(crate) attempt: Option<usize>,
    pub(crate) retry_count: Option<usize>,
    pub(crate) max_attempts: Option<usize>,
    pub(crate) backoff_base_seconds: Option<f64>,
    pub(crate) backoff_multiplier: Option<f64>,
    pub(crate) max_backoff_seconds: Option<f64>,
    pub(crate) requested_parallel_group_id: Option<String>,
    pub(crate) request_parallel_group_ids: Option<Vec<Option<String>>>,
    pub(crate) request_lane_ids: Option<Vec<Option<String>>>,
    pub(crate) lane_id_prefix: Option<String>,
    pub(crate) batch_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SandboxControlRequestPayload {
    pub(crate) schema_version: String,
    pub(crate) operation: String,
    pub(crate) sandbox_id: Option<String>,
    pub(crate) profile_id: Option<String>,
    pub(crate) current_state: Option<String>,
    pub(crate) next_state: Option<String>,
    pub(crate) cleanup_failed: Option<bool>,
    pub(crate) tool_category: Option<String>,
    pub(crate) capability_categories: Option<Vec<String>>,
    pub(crate) dedicated_profile: Option<bool>,
    pub(crate) budget_cpu: Option<f64>,
    pub(crate) budget_memory: Option<i64>,
    pub(crate) budget_wall_clock: Option<f64>,
    pub(crate) budget_output_size: Option<i64>,
    pub(crate) probe_cpu: Option<f64>,
    pub(crate) probe_memory: Option<i64>,
    pub(crate) probe_wall_clock: Option<f64>,
    pub(crate) probe_output_size: Option<i64>,
    pub(crate) error_kind: Option<String>,
    pub(crate) event_log_path: Option<String>,
    pub(crate) trace_event: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SandboxControlResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) operation: String,
    pub(crate) current_state: Option<String>,
    pub(crate) next_state: Option<String>,
    pub(crate) allowed: bool,
    pub(crate) resolved_state: Option<String>,
    pub(crate) reason: String,
    pub(crate) error: Option<String>,
    pub(crate) failure_reason: Option<String>,
    pub(crate) budget_violation: Option<String>,
    pub(crate) cleanup_required: Option<bool>,
    pub(crate) quarantined: Option<bool>,
    pub(crate) effective_capabilities: Option<Vec<String>>,
    pub(crate) sandbox_id: Option<String>,
    pub(crate) profile_id: Option<String>,
    pub(crate) event_schema_version: Option<String>,
    pub(crate) event_log_path: Option<String>,
    pub(crate) event_written: bool,
    pub(crate) event_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackgroundControlEffectPlanPayload {
    pub(crate) next_step: String,
    pub(crate) terminal_status: Option<String>,
    pub(crate) resolved_status: Option<String>,
    pub(crate) finalize_immediately: Option<bool>,
    pub(crate) cancel_running_task: Option<bool>,
    pub(crate) next_retry_count: Option<usize>,
    pub(crate) backoff_seconds: Option<f64>,
    pub(crate) wait_timeout_seconds: Option<f64>,
    pub(crate) wait_poll_interval_seconds: Option<f64>,
    pub(crate) sleep_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackgroundControlResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) operation: String,
    pub(crate) resolved_parallel_group_id: Option<String>,
    pub(crate) lane_ids: Option<Vec<String>>,
    pub(crate) normalized_multitask_strategy: Option<String>,
    pub(crate) supported_multitask_strategies: Vec<String>,
    pub(crate) strategy_supported: bool,
    pub(crate) accepted: Option<bool>,
    pub(crate) requires_takeover: Option<bool>,
    pub(crate) error: Option<String>,
    pub(crate) should_retry: Option<bool>,
    pub(crate) next_retry_count: Option<usize>,
    pub(crate) backoff_seconds: Option<f64>,
    pub(crate) terminal_status: Option<String>,
    pub(crate) resolved_status: Option<String>,
    pub(crate) finalize_immediately: Option<bool>,
    pub(crate) cancel_running_task: Option<bool>,
    pub(crate) reason: String,
    pub(crate) effect_plan: BackgroundControlEffectPlanPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayRequestPayload {
    pub(crate) path: Option<String>,
    pub(crate) event_stream_text: Option<String>,
    pub(crate) compaction_manifest_path: Option<String>,
    pub(crate) compaction_manifest_text: Option<String>,
    pub(crate) compaction_state_text: Option<String>,
    pub(crate) compaction_artifact_index_text: Option<String>,
    pub(crate) compaction_delta_text: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) job_id: Option<String>,
    pub(crate) stream_scope_fields: Option<Vec<String>>,
    pub(crate) after_event_id: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamInspectRequestPayload {
    pub(crate) path: Option<String>,
    pub(crate) event_stream_text: Option<String>,
    pub(crate) compaction_manifest_path: Option<String>,
    pub(crate) compaction_manifest_text: Option<String>,
    pub(crate) compaction_state_text: Option<String>,
    pub(crate) compaction_artifact_index_text: Option<String>,
    pub(crate) compaction_delta_text: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) job_id: Option<String>,
    pub(crate) stream_scope_fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayCursorPayload {
    pub(crate) event_id: Option<String>,
    pub(crate) event_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamReplayResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) path: String,
    pub(crate) source_kind: String,
    pub(crate) event_count: usize,
    pub(crate) latest_event_id: Option<String>,
    pub(crate) latest_event_kind: Option<String>,
    pub(crate) latest_event_timestamp: Option<String>,
    pub(crate) latest_cursor: Option<Value>,
    pub(crate) after_event_id: Option<String>,
    pub(crate) window_start_index: usize,
    pub(crate) has_more: bool,
    pub(crate) next_cursor: Option<TraceStreamReplayCursorPayload>,
    pub(crate) events: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceStreamInspectResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) path: String,
    pub(crate) source_kind: String,
    pub(crate) event_count: usize,
    pub(crate) latest_event_id: Option<String>,
    pub(crate) latest_event_kind: Option<String>,
    pub(crate) latest_event_timestamp: Option<String>,
    pub(crate) latest_cursor: Option<Value>,
    pub(crate) recovery: Option<Value>,
    pub(crate) reroute_count: usize,
    pub(crate) retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceCompactionDeltaWriteRequestPayload {
    pub(crate) path: String,
    pub(crate) delta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceCompactionDeltaWriteResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) path: String,
    pub(crate) bytes_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceMetadataWriteRequestPayload {
    pub(crate) output_path: String,
    #[serde(default)]
    pub(crate) mirror_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub(crate) write_outputs: bool,
    pub(crate) task: String,
    #[serde(default)]
    pub(crate) matched_skills: Vec<String>,
    pub(crate) owner: String,
    pub(crate) gate: String,
    pub(crate) overlay: Option<String>,
    pub(crate) reroute_count: Option<usize>,
    pub(crate) retry_count: Option<usize>,
    #[serde(default)]
    pub(crate) artifact_paths: Vec<String>,
    pub(crate) verification_status: String,
    pub(crate) session_id: Option<String>,
    pub(crate) job_id: Option<String>,
    pub(crate) event_stream_path: Option<String>,
    pub(crate) event_stream_text: Option<String>,
    pub(crate) compaction_manifest_path: Option<String>,
    pub(crate) compaction_manifest_text: Option<String>,
    pub(crate) compaction_state_text: Option<String>,
    pub(crate) compaction_artifact_index_text: Option<String>,
    pub(crate) compaction_delta_text: Option<String>,
    pub(crate) stream_scope_fields: Option<Vec<String>>,
    pub(crate) framework_version: Option<String>,
    pub(crate) metadata_schema_version: Option<String>,
    pub(crate) routing_runtime_version: Option<u64>,
    pub(crate) runtime_path: Option<String>,
    pub(crate) ts: Option<String>,
    pub(crate) trace_event_schema_version: Option<String>,
    pub(crate) trace_event_sink_schema_version: Option<String>,
    pub(crate) parallel_group: Option<Value>,
    pub(crate) supervisor_projection: Option<Value>,
    pub(crate) control_plane: Option<Value>,
    pub(crate) stream: Option<Value>,
    pub(crate) events: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TraceMetadataWriteResponsePayload {
    pub(crate) schema_version: String,
    pub(crate) authority: String,
    pub(crate) output_path: String,
    pub(crate) mirror_paths: Vec<String>,
    pub(crate) bytes_written: usize,
    pub(crate) routing_runtime_version: u64,
    pub(crate) payload_text: String,
}
