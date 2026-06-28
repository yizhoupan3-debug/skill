use anyhow::{Result, bail};
use citation_tool_rs::*;
use clap::{Args, Parser, Subcommand};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Rust-first citation audit, lint, and reference rendering CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Audit(AuditArgs),
    ClaimLint(ClaimLintArgs),
    Render(RenderArgs),
}

#[derive(Args)]
struct AuditArgs {
    #[arg(long)]
    bib: PathBuf,
    #[arg(long)]
    manuscript: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
    format: OutputFormat,
    #[arg(long, default_value_t = 3)]
    cluster_threshold: usize,
    #[arg(long, value_enum, default_value_t = FailOn::Never)]
    fail_on: FailOn,
}

#[derive(Args)]
struct ClaimLintArgs {
    #[arg(long)]
    manuscript: PathBuf,
    #[arg(long, default_value_t = 3)]
    threshold: usize,
    #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
    format: OutputFormat,
    #[arg(long, default_value_t = false)]
    fail_on_findings: bool,
}

#[derive(Args)]
struct RenderArgs {
    #[arg(long)]
    bib: PathBuf,
    #[arg(long, value_enum)]
    style: ReferenceStyle,
    #[arg(long)]
    only: Option<String>,
}

fn main() -> Result<()> {
    run_cli(Cli::parse())
}

fn run_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Audit(args) => run_audit(args),
        Commands::ClaimLint(args) => run_claim_lint(args),
        Commands::Render(args) => run_render(args),
    }
}

fn run_audit(args: AuditArgs) -> Result<()> {
    let entries = parse_bibtex(&read_text(&args.bib)?)?;
    let manuscript_text = args
        .manuscript
        .as_ref()
        .map(|p| read_text(p))
        .transpose()
        .map_err(|e| anyhow::anyhow!("failed to read manuscript: {e}"))?;
    let report = make_report(&entries, manuscript_text.as_deref(), args.cluster_threshold)?;
    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Markdown => print!("{}", audit_report_to_markdown(&report)),
    }
    enforce_audit_fail_on(&report, args.fail_on)?;
    Ok(())
}

fn run_claim_lint(args: ClaimLintArgs) -> Result<()> {
    let findings = lint_claims(&read_text(&args.manuscript)?, args.threshold)?;
    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&findings)?),
        OutputFormat::Markdown => print!("{}", claim_findings_to_markdown(&findings)),
    }
    if args.fail_on_findings && !findings.is_empty() {
        bail!("claim citation lint found {} issue(s)", findings.len());
    }
    Ok(())
}

fn run_render(args: RenderArgs) -> Result<()> {
    let entries = parse_bibtex(&read_text(&args.bib)?)?;
    let selected = args.only.map(|keys| {
        keys.split(',')
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
            .collect::<BTreeSet<_>>()
    });
    for entry in entries {
        if selected
            .as_ref()
            .is_some_and(|keys| !keys.contains(&entry.key))
        {
            continue;
        }
        println!("[{}] {}", entry.key, render_entry(&entry, args.style));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_help_lists_migrated_commands() {
        let mut help = Vec::new();
        Cli::command()
            .write_long_help(&mut help)
            .expect("write help");
        let help = String::from_utf8(help).expect("help is utf8");

        assert!(help.contains("audit"));
        assert!(help.contains("claim-lint"));
        assert!(help.contains("render"));
    }

    #[test]
    fn skill_entrypoint_points_to_rust_only() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rust_tools")
            .parent()
            .expect("repo root");
        let skill_root = repo_root.join("skills/citation-management");
        let skill = std::fs::read_to_string(skill_root.join("SKILL.md")).expect("read skill doc");

        assert!(!skill_root.join("scripts").exists());
        assert!(skill.contains("rust_tools/citation_tool_rs"));
        assert!(skill.contains("cargo run"));
        assert!(!skill.contains("python3"));
        assert!(!skill.contains(".py"));
    }
}
