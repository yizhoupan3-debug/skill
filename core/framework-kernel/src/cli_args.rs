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
    /// Codegraph MCP server (feature-gated in dispatch).
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
    pub json: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum MaintSubcommand {
    /// Rebuild router-rs, `framework sync-entrypoints`, non-Codex framework installs, verify projections.
    RefreshHostProjections(MaintRootsArgs),
    /// Verify a host's hook projection installation (registry-driven).
    #[command(name = "verify-host-hooks")]
    VerifyHostHooks {
        /// Host ID to verify (e.g. cursor, claude, codex, opencode).
        #[arg(long)]
        host_id: String,
        #[command(flatten)]
        args: MaintRepoArgs,
    },
    /// Refresh → skill-compiler --apply → cargo test → generated-artifacts-status (optional host publish).
    UpdateOneShot(MaintRootsArgs),
    /// Dry-run `/update` repository knowledge/hygiene audit; prints JSON and does not delete files.
    UpdateAudit(UpdateAuditArgs),
    /// Delete every `target/` tree under the repo (skips `.git/`).
    CleanRustTargets(MaintRepoArgs),
    /// Print `export CODEX_HOME=…` / `CURSOR_HOME=…` for repo-local session homes.
    PrintLocalHomes(MaintRepoArgs),
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
    #[arg(long, default_value = "false")]
    pub dry_run: bool,
}

#[derive(Args, Debug, Clone)]
pub struct UpdateAuditArgs {
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
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
        /// Backfill null registry columns from SKILL.md frontmatter.
        #[arg(long, default_value = "false")]
        backfill: bool,
        /// Preview backfill/generate changes without modifying files.
        #[arg(long, default_value = "false")]
        dry_run: bool,
        /// Generate SKILL.md frontmatter from registry.
        /// Value: "all" for all skills, or a specific slug.
        #[arg(long)]
        generate: Option<String>,
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
#[derive(Subcommand, Debug, Clone)]
pub enum HostCommand {
    /// Run a hook event for any hook-capable host (cursor, claude, opencode, codex).
    Hook {
        host_id: String,
        #[command(flatten)]
        command: GenericHookCommand,
    },
    /// Run an MCP stdio agent loop (opencode, claude).
    Agent {
        host_id: String,
        #[command(flatten)]
        command: GenericAgentCommand,
    },
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

/// Codegraph MCP subcommand (feature-gated in dispatch).
#[derive(Subcommand, Debug, Clone)]
pub enum CodegraphSubcommand {
    McpStdio(CodegraphMcpStdioCommand),
}

/// Arguments for `codegraph mcp-stdio`.
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
#[command(name = "router-rs-cli")]
#[command(about = "Fast Rust routing core for skill lookup")]
#[command(override_usage = "router-rs-cli <COMMAND>")]
#[command(
    help_template = "{about-section}\nUsage: {usage}\n\nCommands:\n{subcommands}\n\nUse `router-rs-cli <command> --help` for command-specific options.\n"
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // ── Cli parser tests ──

    #[test]
    fn cli_parses_with_no_args() {
        let cli = Cli::try_parse_from(["router-rs-cli"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert!(cli.command.is_none());
        assert!(cli.repo_root.is_none());
        assert!(cli.query.is_none());
    }

    #[test]
    fn cli_default_limit() {
        let cli = Cli::try_parse_from(["router-rs-cli"]).unwrap();
        assert_eq!(cli.limit, 5);
    }

    #[test]
    fn cli_with_global_flags() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "--repo-root",
            "/tmp/repo",
            "--query",
            "test query",
            "--limit",
            "10",
            "--json",
        ])
        .unwrap();
        assert_eq!(
            cli.repo_root,
            Some(PathBuf::from("/tmp/repo"))
        );
        assert_eq!(
            cli.query,
            Some("test query".to_string())
        );
        assert_eq!(cli.limit, 10);
        assert!(cli.json);
    }

    // ── RouteCommand tests ──

    #[test]
    fn route_command_parses_query() {
        let cli = Cli::try_parse_from(["router-rs-cli", "route", "my query"]);
        assert!(cli.is_ok());
        match cli.unwrap().command {
            Some(RouterCommand::Route(cmd)) => {
                assert_eq!(cmd.query, "my query");
            }
            other => panic!("expected Route, got {:?}", other),
        }
    }

    #[test]
    fn route_command_default_session_id() {
        let cli =
            Cli::try_parse_from(["router-rs-cli", "route", "q"]).unwrap();
        match cli.command {
            Some(RouterCommand::Route(cmd)) => {
                assert_eq!(cmd.session_id, "route-cli");
                assert!(cmd.allow_overlay);
                assert!(cmd.first_turn);
                assert!(cmd.host_id.is_none());
            }
            other => panic!("expected Route, got {:?}", other),
        }
    }

    #[test]
    fn route_command_with_host_id() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "route",
            "q",
            "--host-id",
            "claude",
            "--session-id",
            "s1",
            "--allow-overlay",
            "false",
            "--first-turn",
            "false",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Route(cmd)) => {
                assert_eq!(cmd.host_id.as_deref(), Some("claude"));
                assert_eq!(cmd.session_id, "s1");
                assert!(!cmd.allow_overlay);
                assert!(!cmd.first_turn);
            }
            other => panic!("expected Route, got {:?}", other),
        }
    }

    // ── SearchCommand tests ──

    #[test]
    fn search_command_defaults() {
        let cli =
            Cli::try_parse_from(["router-rs-cli", "search", "find me"]).unwrap();
        match cli.command {
            Some(RouterCommand::Search(cmd)) => {
                assert_eq!(cmd.query, "find me");
                assert_eq!(cmd.limit, 5);
                assert!(!cmd.json);
            }
            other => panic!("expected Search, got {:?}", other),
        }
    }

    #[test]
    fn search_command_with_options() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "search",
            "q",
            "--limit",
            "20",
            "--json",
            "--host-id",
            "codex",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Search(cmd)) => {
                assert_eq!(cmd.limit, 20);
                assert!(cmd.json);
                assert_eq!(cmd.host_id.as_deref(), Some("codex"));
            }
            other => panic!("expected Search, got {:?}", other),
        }
    }

    // ── Framework subcommand tests ──

    #[test]
    fn framework_doctor_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "doctor",
            "--repo-root",
            "/tmp",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::Doctor(cmd),
            }) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp")));
            }
            other => panic!("expected Framework::Doctor, got {:?}", other),
        }
    }

    #[test]
    fn framework_sync_entrypoints_default() {
        let cli =
            Cli::try_parse_from(["router-rs-cli", "framework", "sync-entrypoints"])
                .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::SyncEntrypoints(cmd),
            }) => {
                assert!(cmd.repo_root.is_none());
                assert!(cmd.host_id.is_none());
            }
            other => panic!("expected SyncEntrypoints, got {:?}", other),
        }
    }

    #[test]
    fn framework_scaffold_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "scaffold",
            "--host-id",
            "windsurf",
            "--dry-run",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::Scaffold(cmd),
            }) => {
                assert_eq!(cmd.host_id, "windsurf");
                assert!(cmd.dry_run);
            }
            other => panic!("expected Scaffold, got {:?}", other),
        }
    }

    // ── Host command tests ──

    #[test]
    fn host_hook_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "host",
            "hook",
            "claude",
            "--event",
            "PreToolUse",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Host {
                command: HostCommand::Hook { host_id, command },
            }) => {
                assert_eq!(host_id, "claude");
                assert_eq!(command.event, "PreToolUse");
            }
            other => panic!("expected Host::Hook, got {:?}", other),
        }
    }

    #[test]
    fn host_agent_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "host",
            "agent",
            "opencode",
            "--repo-root",
            "/tmp",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Host {
                command: HostCommand::Agent { host_id, command },
            }) => {
                assert_eq!(host_id, "opencode");
                assert_eq!(
                    command.repo_root,
                    Some(PathBuf::from("/tmp"))
                );
            }
            other => panic!("expected Host::Agent, got {:?}", other),
        }
    }

    // ── Trace command tests ──

    #[test]
    fn trace_compact_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "trace",
            "compact",
            "--input-json",
            "{}",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Trace {
                command: TraceCommand::Compact(cmd),
            }) => {
                assert_eq!(cmd.input_json, "{}");
            }
            other => panic!("expected Trace::Compact, got {:?}", other),
        }
    }

    // ── Storage command tests ──

    #[test]
    fn storage_backend_catalog_parses() {
        let cli =
            Cli::try_parse_from(["router-rs-cli", "storage", "backend-catalog"])
                .unwrap();
        match cli.command {
            Some(RouterCommand::Storage {
                command: StorageCommand::BackendCatalog,
            }) => {}
            other => panic!("expected BackendCatalog, got {:?}", other),
        }
    }

    // ── Loop command tests ──

    #[test]
    fn loop_run_defaults() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "loop",
            "run",
            "--loop-id",
            "my-loop",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Loop {
                command: LoopCommand::Run(cmd),
            }) => {
                assert_eq!(cmd.loop_id, "my-loop");
                assert!(!cmd.dry_run);
                assert_eq!(cmd.timeout, 600);
            }
            other => panic!("expected Loop::Run, got {:?}", other),
        }
    }

    #[test]
    fn loop_kill_with_all_flag() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "loop",
            "kill",
            "--loop-id",
            "l1",
            "--all",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Loop {
                command: LoopCommand::Kill(cmd),
            }) => {
                assert_eq!(cmd.loop_id, "l1");
                assert!(cmd.all);
            }
            other => panic!("expected Loop::Kill, got {:?}", other),
        }
    }

    // ── Eval command tests ──

    #[test]
    fn eval_route_contract_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "eval",
            "route-contract",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Eval {
                command: EvalCommand::RouteContract,
            }) => {}
            other => panic!("expected RouteContract, got {:?}", other),
        }
    }

    // ── SchemaDrift command tests ──

    #[test]
    fn schema_drift_contract_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "schema-drift",
            "contract",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::SchemaDrift {
                command: SchemaDriftCommand::Contract,
            }) => {}
            other => panic!("expected Contract, got {:?}", other),
        }
    }

    // ── Closeout command tests ──

    #[test]
    fn closeout_contract_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "closeout",
            "contract",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Closeout {
                command: CloseoutCommand::Contract,
            }) => {}
            other => panic!("expected Contract, got {:?}", other),
        }
    }

    // ── Maint subcommand tests ──

    #[test]
    fn framework_maint_refresh_host_projections() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "maint",
            "refresh-host-projections",
            "--framework-root",
            "/fw",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command:
                    FrameworkCommand::Maint {
                        command:
                            MaintSubcommand::RefreshHostProjections(args),
                    },
            }) => {
                assert_eq!(
                    args.framework_root,
                    Some(PathBuf::from("/fw"))
                );
            }
            other => panic!("expected RefreshHostProjections, got {:?}", other),
        }
    }

    #[test]
    fn framework_maint_clean_hook_state_dry_run() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "maint",
            "clean-hook-state",
            "--dry-run",
            "--older-than-days",
            "14",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command:
                    FrameworkCommand::Maint {
                        command:
                            MaintSubcommand::CleanHookState(args),
                    },
            }) => {
                assert!(args.dry_run);
                assert_eq!(args.older_than_days, Some(14));
            }
            other => panic!("expected CleanHookState, got {:?}", other),
        }
    }

    // ── Diagnose command tests ──

    #[test]
    fn diagnose_profile_emit() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "diagnose",
            "profile",
            "emit",
            "--framework-profile",
            "/tmp/profile.json",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Diagnose {
                command:
                    DiagnoseCommand::Profile {
                        command: ProfileSubcommand::Emit(cmd),
                    },
            }) => {
                assert_eq!(
                    cmd.framework_profile,
                    PathBuf::from("/tmp/profile.json")
                );
                assert!(!cmd.full);
            }
            other => panic!("expected Profile::Emit, got {:?}", other),
        }
    }

    // ── Skills subcommand tests ──

    #[test]
    fn skills_validate_parses() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "skills",
            "validate",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::Skills { command },
            }) => {
                assert!(matches!(command, SkillsSubcommand::Validate(_)));
            }
            other => panic!("expected Skills::Validate, got {:?}", other),
        }
    }

    // ── HookPolicy command tests ──

    #[test]
    fn hook_policy_contract() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "hook-policy",
            "contract",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::HookPolicy {
                command: HookPolicyCommand::Contract,
            }) => {}
            other => panic!("expected Contract, got {:?}", other),
        }
    }

    // ── ForwardedArgsCommand tested via Cli host integration ──

    #[test]
    fn host_integration_forwarded_args_via_cli() {
        // ForwardedArgsCommand is tested through the Cli parser since it derives
        // Args (not Parser). The host integration command uses it with trailing var args.
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "host-integration",
            "--",
            "--host-id",
            "claude",
            "--extra-flag",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::HostIntegration(cmd),
            }) => {
                assert_eq!(
                    cmd.args,
                    vec!["--host-id", "claude", "--extra-flag"]
                );
            }
            other => panic!("expected HostIntegration, got {:?}", other),
        }
    }

    // ── JsonInputCommand tested via Cli trace compact ──

    #[test]
    fn json_input_command_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "trace",
            "compact",
            "--input-json",
            r#"{"key":"value"}"#,
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Trace {
                command: TraceCommand::Compact(cmd),
            }) => {
                assert_eq!(cmd.input_json, r#"{"key":"value"}"#);
            }
            other => panic!("expected Trace::Compact, got {:?}", other),
        }
    }

    // ── MaintRootsArgs tested via framework maint refresh ──

    #[test]
    fn maint_roots_args_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "maint",
            "refresh-host-projections",
            "--framework-root",
            "/fw",
            "--artifact-root",
            "/art",
            "--home",
            "/usr",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command:
                    FrameworkCommand::Maint {
                        command:
                            MaintSubcommand::RefreshHostProjections(args),
                    },
            }) => {
                assert_eq!(args.framework_root, Some(PathBuf::from("/fw")));
                assert_eq!(args.artifact_root, Some(PathBuf::from("/art")));
                assert_eq!(args.home, Some(PathBuf::from("/usr")));
            }
            other => panic!("expected RefreshHostProjections, got {:?}", other),
        }
    }

    // ── MaintRepoArgs tested via verify-host-hooks ──

    #[test]
    fn maint_repo_args_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "maint",
            "verify-host-hooks",
            "--host-id",
            "cursor",
            "--framework-root",
            "/fw",
            "--dry-run",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command:
                    FrameworkCommand::Maint {
                        command:
                            MaintSubcommand::VerifyHostHooks {
                                host_id,
                                args,
                            },
                    },
            }) => {
                assert_eq!(host_id, "cursor");
                assert_eq!(args.framework_root, Some(PathBuf::from("/fw")));
                assert!(args.dry_run);
            }
            other => panic!("expected VerifyHostHooks, got {:?}", other),
        }
    }

    // ── CloseoutEvaluateCommand tested via closeout evaluate ──

    #[test]
    fn closeout_evaluate_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "closeout",
            "evaluate",
            "--input-json",
            "{}",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Closeout {
                command: CloseoutCommand::Evaluate(cmd),
            }) => {
                assert_eq!(cmd.input_json.as_deref(), Some("{}"));
                assert!(cmd.record_path.is_none());
            }
            other => panic!("expected Closeout::Evaluate, got {:?}", other),
        }
    }

    // ── EvalRouteCommand tested via eval route ──

    #[test]
    fn eval_route_command_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "eval",
            "route",
            "--cases",
            "/tmp/cases.json",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Eval {
                command: EvalCommand::Route(cmd),
            }) => {
                assert_eq!(cmd.cases, PathBuf::from("/tmp/cases.json"));
                assert!(cmd.runtime.is_none());
            }
            other => panic!("expected Eval::Route, got {:?}", other),
        }
    }

    // ── LoopStatusCommand tested via loop status ──

    #[test]
    fn loop_status_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "loop",
            "status",
            "--loop-id",
            "abc",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Loop {
                command: LoopCommand::Status(cmd),
            }) => {
                assert_eq!(cmd.loop_id, "abc");
            }
            other => panic!("expected Loop::Status, got {:?}", other),
        }
    }

    // ── SchemaDriftRepoArgs tested via schema-drift baseline ──

    #[test]
    fn schema_drift_baseline_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "schema-drift",
            "baseline",
            "--repo-root",
            "/tmp",
            "--task-id",
            "task-1",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::SchemaDrift {
                command: SchemaDriftCommand::Baseline(args),
            }) => {
                assert_eq!(args.repo_root, Some(PathBuf::from("/tmp")));
                assert_eq!(args.task_id.as_deref(), Some("task-1"));
            }
            other => panic!("expected SchemaDrift::Baseline, got {:?}", other),
        }
    }

    // ── ProfilePathCommand tested via diagnose profile emit ──

    #[test]
    fn profile_path_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "diagnose",
            "profile",
            "emit",
            "--framework-profile",
            "/tmp/p.json",
            "--full",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Diagnose {
                command:
                    DiagnoseCommand::Profile {
                        command: ProfileSubcommand::Emit(cmd),
                    },
            }) => {
                assert_eq!(cmd.framework_profile, PathBuf::from("/tmp/p.json"));
                assert!(cmd.full);
            }
            other => panic!("expected Profile::Emit, got {:?}", other),
        }
    }

    // ── FrameworkContractsCommand tested via framework contracts ──

    #[test]
    fn framework_contracts_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "contracts",
            "--repo-root",
            "/tmp",
            "--summary",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::Contracts(cmd),
            }) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp")));
                assert!(cmd.summary);
            }
            other => panic!("expected Contracts, got {:?}", other),
        }
    }

    // ── StorageBackendParityCommand tested via storage backend-parity ──

    #[test]
    fn storage_backend_parity_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "storage",
            "backend-parity",
            "--store",
            "local",
            "--checkpointer",
            "fs",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Storage {
                command: StorageCommand::BackendParity(cmd),
            }) => {
                assert_eq!(cmd.store.as_deref(), Some("local"));
                assert_eq!(cmd.checkpointer.as_deref(), Some("fs"));
                assert!(cmd.trace.is_none());
                assert!(cmd.state.is_none());
            }
            other => panic!("expected BackendParity, got {:?}", other),
        }
    }

    // ── BrowserMcpStdioCommand tested via browser mcp-stdio ──

    #[test]
    fn browser_mcp_stdio_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "browser",
            "mcp-stdio",
            "--repo-root",
            "/tmp",
            "--headless",
            "true",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Browser {
                command: BrowserSubcommand::McpStdio(cmd),
            }) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp")));
                assert_eq!(cmd.headless.as_deref(), Some("true"));
            }
            other => panic!("expected McpStdio, got {:?}", other),
        }
    }

    // ── BrowserResolveAttachCommand tested via browser resolve-attach-artifact ──

    #[test]
    fn browser_resolve_attach_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "browser",
            "resolve-attach-artifact",
            "--repo-root",
            "/tmp",
            "--search-root",
            "/search",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Browser {
                command: BrowserSubcommand::ResolveAttachArtifact(cmd),
            }) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp")));
                assert_eq!(cmd.search_root, Some(PathBuf::from("/search")));
            }
            other => panic!("expected ResolveAttachArtifact, got {:?}", other),
        }
    }

    // ── FrameworkSnapshotCommand tested via framework snapshot ──

    #[test]
    fn framework_snapshot_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "snapshot",
            "--repo-root",
            "/tmp",
            "--task-id",
            "t1",
            "--detail-level",
            "full",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::Snapshot(cmd),
            }) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp")));
                assert_eq!(cmd.task_id.as_deref(), Some("t1"));
                assert_eq!(cmd.detail_level.as_deref(), Some("full"));
            }
            other => panic!("expected Snapshot, got {:?}", other),
        }
    }

    // ── FrameworkAliasCommand tested via framework alias ──

    #[test]
    fn framework_alias_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "alias",
            "my-alias",
            "--max-lines",
            "10",
            "--compact",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::Alias(cmd),
            }) => {
                assert_eq!(cmd.alias, "my-alias");
                assert_eq!(cmd.max_lines, 10);
                assert!(cmd.compact);
            }
            other => panic!("expected Alias, got {:?}", other),
        }
    }

    // ── FrameworkTaskStateResolveCommand tested via task-state-resolve ──

    #[test]
    fn task_state_resolve_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "framework",
            "task-state-resolve",
            "--repo-root",
            "/tmp",
            "--task-id",
            "t1",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Framework {
                command: FrameworkCommand::TaskStateResolve(cmd),
            }) => {
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp")));
                assert_eq!(cmd.task_id.as_deref(), Some("t1"));
            }
            other => panic!("expected TaskStateResolve, got {:?}", other),
        }
    }

    // ── GenericHookCommand tested via diag profile artifacts ──

    #[test]
    fn diagnose_profile_artifacts_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "diagnose",
            "profile",
            "artifacts",
            "--framework-profile",
            "/tmp/p.json",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Diagnose {
                command:
                    DiagnoseCommand::Profile {
                        command: ProfileSubcommand::Artifacts(cmd),
                    },
            }) => {
                assert_eq!(cmd.framework_profile, PathBuf::from("/tmp/p.json"));
                assert!(!cmd.full);
            }
            other => panic!("expected Profile::Artifacts, got {:?}", other),
        }
    }

    // ── CurrentArtifactClutterCommand tested via migrate current-artifact-clutter ──

    #[test]
    fn migrate_current_artifact_clutter_via_cli() {
        let cli = Cli::try_parse_from([
            "router-rs-cli",
            "migrate",
            "current-artifact-clutter",
            "task-123",
            "--repo-root",
            "/tmp",
        ])
        .unwrap();
        match cli.command {
            Some(RouterCommand::Migrate {
                command:
                    MigrateCommand::CurrentArtifactClutter(cmd),
            }) => {
                assert_eq!(cmd.active_task_id, "task-123");
                assert_eq!(cmd.repo_root, Some(PathBuf::from("/tmp")));
            }
            other => panic!("expected CurrentArtifactClutter, got {:?}", other),
        }
    }
}
