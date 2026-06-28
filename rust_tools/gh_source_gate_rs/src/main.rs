use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;

use gh_source_gate_rs::{
    DEFAULT_CONTEXT_LINES, DEFAULT_MAX_LINES, build_doctor_report, fetch_comments_json,
    inspect_pr_checks_json, render_check_results, render_comment_summary, render_doctor_report,
};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Rust source-gate CLI for GitHub PR checks and review comments"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect failing GitHub Actions checks for a PR.
    InspectPrChecks(InspectPrChecksArgs),
    /// Fetch PR conversation comments, reviews, and review threads.
    FetchComments(FetchCommentsArgs),
    /// Verify this source gate is fully Rust-owned.
    Doctor(DoctorArgs),
}

#[derive(Args)]
struct InspectPrChecksArgs {
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    #[arg(long)]
    pr: Option<String>,
    #[arg(long, default_value_t = DEFAULT_MAX_LINES)]
    max_lines: usize,
    #[arg(long, default_value_t = DEFAULT_CONTEXT_LINES)]
    context: usize,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct FetchCommentsArgs {
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    #[arg(long)]
    pr: Option<String>,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    open_only: bool,
}

#[derive(Args)]
struct DoctorArgs {
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    #[arg(long)]
    json: bool,
}

fn main() {
    if let Err(error) = run_cli() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_cli() -> Result<()> {
    match Cli::parse().command {
        Commands::InspectPrChecks(args) => inspect_pr_checks(args),
        Commands::FetchComments(args) => fetch_comments(args),
        Commands::Doctor(args) => doctor(args),
    }
}

fn inspect_pr_checks(args: InspectPrChecksArgs) -> Result<()> {
    let repo_root = gh_source_gate_rs::find_git_root(&args.repo)?;
    let result =
        inspect_pr_checks_json(&repo_root, args.pr.as_deref(), args.max_lines, args.context)?;

    let failing = result
        .get("failing_check_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    if failing == 0 {
        println!(
            "{}",
            result
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("No failing checks.")
        );
        return Ok(());
    }

    let pr = result.get("pr").and_then(Value::as_str).unwrap_or("?");
    let results = result
        .get("results")
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        render_check_results(pr, results);
    }

    std::process::exit(1);
}

fn fetch_comments(args: FetchCommentsArgs) -> Result<()> {
    let repo_root = gh_source_gate_rs::find_git_root(&args.repo)?;
    let result = fetch_comments_json(&repo_root, args.pr.as_deref(), args.open_only)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        render_comment_summary(&result);
    }
    Ok(())
}

fn doctor(args: DoctorArgs) -> Result<()> {
    let report = build_doctor_report(&args.repo);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render_doctor_report(&report);
    }
    if report.checks.iter().all(|check| check.ok) {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
