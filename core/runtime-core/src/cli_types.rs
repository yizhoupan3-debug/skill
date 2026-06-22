//! CLI 类型定义 — 仅含 runtime-core 内部模块所需的类型。
//! 完整 CLI 层已迁至 router-rs crate；router-rs 通过 `runtime_core::cli_types` 引用这些类型。

use clap::{Args, Subcommand};
use std::path::PathBuf;

// ── Browser dispatch hook ──

#[derive(Subcommand, Debug, Clone)]
pub enum BrowserSubcommand {
    McpStdio(BrowserMcpStdioCommand),
    ResolveAttachArtifact(BrowserResolveAttachCommand),
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

// ── Framework maint subcommand types ──

#[derive(Subcommand, Debug, Clone)]
pub enum MaintSubcommand {
    RefreshHostProjections(MaintRootsArgs),
    VerifyCursorHooks(MaintRepoArgs),
    VerifyCodexHooks(MaintRepoArgs),
    UpdateOneShot(MaintRootsArgs),
    UpdateAudit(UpdateAuditArgs),
    CleanRustTargets(MaintRepoArgs),
    PrintLocalHomes(MaintRepoArgs),
    InstallCodexUserHooks(InstallCodexUserHooksArgs),
    ContinuityAudit(MaintRepoArgs),
    CleanHookState(CleanHookStateArgs),
    CleanOrphans(CleanOrphansArgs),
}

#[derive(Args, Debug, Clone)]
pub struct CleanHookStateArgs {
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
    #[arg(long, default_value = "false")]
    pub dry_run: bool,
    #[arg(long)]
    pub older_than_days: Option<u64>,
}

#[derive(Args, Debug, Clone)]
pub struct CleanOrphansArgs {
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
    #[arg(long, default_value = "false")]
    pub dry_run: bool,
    #[arg(long)]
    pub older_than_days: Option<u64>,
}

#[derive(Args, Debug, Clone)]
pub struct MaintRootsArgs {
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
    #[arg(long)]
    pub artifact_root: Option<PathBuf>,
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

#[derive(Args, Debug, Clone)]
pub struct InstallCodexUserHooksArgs {
    #[arg(long)]
    pub framework_root: Option<PathBuf>,
    #[arg(long)]
    pub codex_home: Option<PathBuf>,
}

// ── env_usize helper (originally in common.inc) ──

pub fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}
