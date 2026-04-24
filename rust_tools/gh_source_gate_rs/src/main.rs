use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const DEFAULT_MAX_LINES: usize = 160;
const DEFAULT_CONTEXT_LINES: usize = 30;

const FAILURE_CONCLUSIONS: &[&str] = &["failure", "cancelled", "timed_out", "action_required"];
const FAILURE_STATES: &[&str] = &[
    "failure",
    "error",
    "cancelled",
    "timed_out",
    "action_required",
];
const FAILURE_BUCKETS: &[&str] = &["fail"];
const FAILURE_MARKERS: &[&str] = &[
    "error",
    "fail",
    "failed",
    "traceback",
    "exception",
    "assert",
    "panic",
    "fatal",
    "timeout",
    "segmentation fault",
];
const PENDING_LOG_MARKERS: &[&str] = &[
    "still in progress",
    "log will be available when it is complete",
];
const MISSING_SECRET_MARKERS: &[&str] = &[
    "secret",
    "permission denied",
    "unauthorized",
    "forbidden",
    "bad credentials",
];
const LINT_MARKERS: &[&str] = &["eslint", "ruff", "clippy", "lint"];
const TYPECHECK_MARKERS: &[&str] = &["typecheck", "tsc", "mypy", "pyright", "type error"];
const TEST_MARKERS: &[&str] = &["test", "pytest", "vitest", "jest", "cargo test"];
const BUILD_MARKERS: &[&str] = &[
    "build",
    "compile",
    "compilation",
    "cargo build",
    "npm run build",
];
const FLAKE_MARKERS: &[&str] = &["timed out", "timeout", "rate limit", "connection reset"];

const REVIEW_THREADS_QUERY: &str = r#"query(
  $owner: String!,
  $repo: String!,
  $number: Int!,
  $commentsCursor: String,
  $reviewsCursor: String,
  $threadsCursor: String
) {
  repository(owner: $owner, name: $repo) {
    pullRequest(number: $number) {
      number
      url
      title
      state
      comments(first: 100, after: $commentsCursor) {
        pageInfo { hasNextPage endCursor }
        nodes { id body createdAt updatedAt author { login } }
      }
      reviews(first: 100, after: $reviewsCursor) {
        pageInfo { hasNextPage endCursor }
        nodes { id state body submittedAt author { login } }
      }
      reviewThreads(first: 100, after: $threadsCursor) {
        pageInfo { hasNextPage endCursor }
        nodes {
          id
          isResolved
          isOutdated
          path
          line
          diffSide
          startLine
          startDiffSide
          originalLine
          originalStartLine
          resolvedBy { login }
          comments(first: 100) {
            nodes { id body createdAt updatedAt author { login } }
          }
        }
      }
    }
  }
}"#;

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

struct GhResult {
    code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    status: &'static str,
    checks: Vec<DoctorCheck>,
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    name: &'static str,
    ok: bool,
    detail: String,
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
    let repo_root = find_git_root(&args.repo)?;
    ensure_gh_authenticated(&repo_root)?;
    let pr = resolve_pr(args.pr.as_deref(), &repo_root)?;
    let checks = fetch_checks(&pr, &repo_root)?;
    let failing: Vec<&Value> = checks.iter().filter(|check| is_failing(check)).collect();

    if failing.is_empty() {
        println!("PR #{pr}: no failing checks detected.");
        return Ok(());
    }

    let results: Vec<Value> = failing
        .into_iter()
        .map(|check| {
            analyze_check(
                check,
                &repo_root,
                args.max_lines.max(1),
                args.context.max(1),
            )
        })
        .collect::<Result<_>>()?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "pr": pr, "results": results }))?
        );
    } else {
        render_check_results(&pr, &results);
    }

    std::process::exit(1);
}

fn fetch_comments(args: FetchCommentsArgs) -> Result<()> {
    let repo_root = find_git_root(&args.repo)?;
    ensure_gh_authenticated(&repo_root)?;
    let (owner, repo, number) = resolve_pr_ref(args.pr.as_deref(), &repo_root)?;
    let mut result = fetch_all_comments(&owner, &repo, number, &repo_root)?;
    if args.open_only {
        filter_open_review_threads(&mut result);
    }

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

fn find_git_root(start: &Path) -> Result<PathBuf> {
    let result = run_command(&["git", "rev-parse", "--show-toplevel"], Some(start), None);
    if result.code != 0 {
        bail!("Error: not inside a Git repository.");
    }
    Ok(PathBuf::from(result.stdout.trim()))
}

fn build_doctor_report(start: &Path) -> DoctorReport {
    let repo_root = find_git_root(start).unwrap_or_else(|_| start.to_path_buf());
    let checks = vec![
        check_skill_scripts_retired(&repo_root),
        check_docs_are_rust_only(&repo_root),
        check_routing_surfaces_are_rust_only(&repo_root),
        check_workspace_member(&repo_root),
        check_cli_source_owns_commands(&repo_root),
    ];
    let status = if checks.iter().all(|check| check.ok) {
        "ok"
    } else {
        "failed"
    };
    DoctorReport { status, checks }
}

fn check_skill_scripts_retired(repo_root: &Path) -> DoctorCheck {
    let offenders = [
        repo_root.join("skills/gh-fix-ci"),
        repo_root.join("skills/gh-address-comments"),
    ]
    .iter()
    .flat_map(|skill| {
        let mut paths = Vec::new();
        if skill.join("scripts").exists() {
            paths.push(repo_relative(repo_root, &skill.join("scripts")));
        }
        for file in collect_files_with_extension(skill, "py") {
            paths.push(repo_relative(repo_root, &file));
        }
        paths
    })
    .collect::<Vec<_>>();

    DoctorCheck {
        name: "retired-python-helper-files",
        ok: offenders.is_empty(),
        detail: if offenders.is_empty() {
            "no Python helper files or scripts directories under gh source-gate skills".to_string()
        } else {
            offenders.join(", ")
        },
    }
}

fn check_docs_are_rust_only(repo_root: &Path) -> DoctorCheck {
    let docs = markdown_text_under(&[
        repo_root.join("skills/gh-fix-ci"),
        repo_root.join("skills/gh-address-comments"),
    ]);
    let required = [
        "gh_source_gate_rs",
        "gh-source-gate",
        "inspect-pr-checks",
        "fetch-comments",
        "--open-only",
    ];
    let forbidden = ["inspect_pr_checks.py", "fetch_comments.py"];
    let mut issues = Vec::new();
    for marker in required {
        if !docs.contains(marker) {
            issues.push(format!("missing {marker}"));
        }
    }
    for marker in forbidden {
        if docs.contains(marker) {
            issues.push(format!("retired marker present {marker}"));
        }
    }
    if docs.to_ascii_lowercase().contains("python") {
        issues.push("python marker present".to_string());
    }

    DoctorCheck {
        name: "skill-docs-rust-only",
        ok: issues.is_empty(),
        detail: if issues.is_empty() {
            "skill docs route to gh-source-gate Rust CLI only".to_string()
        } else {
            issues.join(", ")
        },
    }
}

fn check_routing_surfaces_are_rust_only(repo_root: &Path) -> DoctorCheck {
    let surfaces = [
        "skills/SKILL_MANIFEST.json",
        "skills/SKILL_ROUTING_RUNTIME.json",
        "skills/SKILL_ROUTING_REGISTRY.md",
        "skills/SKILL_ROUTING_INDEX.md",
        "skills/SKILL_APPROVAL_POLICY.json",
    ];
    let joined = surfaces
        .iter()
        .filter_map(|path| fs::read_to_string(repo_root.join(path)).ok())
        .collect::<Vec<_>>()
        .join("\n");
    let mut issues = Vec::new();
    for marker in ["inspect_pr_checks.py", "fetch_comments.py"] {
        if joined.contains(marker) {
            issues.push(format!("retired marker present {marker}"));
        }
    }
    if !joined.contains("gh-source-gate") {
        issues.push("missing gh-source-gate".to_string());
    }

    DoctorCheck {
        name: "generated-routing-rust-only",
        ok: issues.is_empty(),
        detail: if issues.is_empty() {
            "generated routing surfaces reference Rust CLI and not retired helpers".to_string()
        } else {
            issues.join(", ")
        },
    }
}

fn check_workspace_member(repo_root: &Path) -> DoctorCheck {
    let manifest = fs::read_to_string(repo_root.join("rust_tools/Cargo.toml")).unwrap_or_default();
    let crate_manifest = repo_root.join("rust_tools/gh_source_gate_rs/Cargo.toml");
    let mut issues = Vec::new();
    if !manifest.contains(r#""gh_source_gate_rs""#) {
        issues.push("missing rust_tools workspace member".to_string());
    }
    if !crate_manifest.exists() {
        issues.push("missing gh_source_gate_rs Cargo.toml".to_string());
    }

    DoctorCheck {
        name: "workspace-member",
        ok: issues.is_empty(),
        detail: if issues.is_empty() {
            "gh_source_gate_rs is in the Rust workspace".to_string()
        } else {
            issues.join(", ")
        },
    }
}

fn check_cli_source_owns_commands(repo_root: &Path) -> DoctorCheck {
    let source = fs::read_to_string(repo_root.join("rust_tools/gh_source_gate_rs/src/main.rs"))
        .unwrap_or_default();
    let required = [
        "InspectPrChecks(InspectPrChecksArgs)",
        "FetchComments(FetchCommentsArgs)",
        "Doctor(DoctorArgs)",
        "fn inspect_pr_checks(",
        "fn fetch_comments(",
        "fn doctor(",
        "REVIEW_THREADS_QUERY",
        "fn classify_failure(",
        "fn filter_open_review_threads(",
        "\"summary\"",
    ];
    let issues = required
        .iter()
        .filter(|marker| !source.contains(**marker))
        .map(|marker| format!("missing {marker}"))
        .collect::<Vec<_>>();

    DoctorCheck {
        name: "rust-cli-command-ownership",
        ok: issues.is_empty(),
        detail: if issues.is_empty() {
            "Rust CLI owns checks, comments, doctor, classification, and summary paths".to_string()
        } else {
            issues.join(", ")
        },
    }
}

fn ensure_gh_authenticated(repo_root: &Path) -> Result<()> {
    let result = run_gh(&["auth", "status"], repo_root);
    if result.code == 0 {
        return Ok(());
    }
    let message = joined_message(&result);
    bail!(
        "{}",
        if message.is_empty() {
            "Error: gh not authenticated."
        } else {
            &message
        }
    )
}

fn resolve_pr(pr_value: Option<&str>, repo_root: &Path) -> Result<String> {
    if let Some(value) = pr_value.filter(|value| !value.trim().is_empty()) {
        return Ok(value.to_string());
    }
    let data = run_gh_json(&["pr", "view", "--json", "number"], repo_root)?;
    data.get("number")
        .and_then(Value::as_i64)
        .map(|number| number.to_string())
        .ok_or_else(|| anyhow!("Error: no PR number found."))
}

fn resolve_pr_ref(pr_value: Option<&str>, repo_root: &Path) -> Result<(String, String, i64)> {
    if let Some(value) = pr_value.filter(|value| !value.trim().is_empty()) {
        if let Some((owner, repo, number)) = parse_github_pr_url(value) {
            return Ok((owner, repo, number));
        }
        let current = current_repo_owner_name(repo_root)?;
        let number = parse_pr_number(value)?;
        return Ok((current.0, current.1, number));
    }

    let data = run_gh_json(
        &[
            "pr",
            "view",
            "--json",
            "number,headRepositoryOwner,headRepository",
        ],
        repo_root,
    )?;
    let owner = data
        .pointer("/headRepositoryOwner/login")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Error: unable to resolve PR owner."))?;
    let repo = data
        .pointer("/headRepository/name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Error: unable to resolve PR repository."))?;
    let number = data
        .get("number")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow!("Error: no PR number found."))?;
    Ok((owner.to_string(), repo.to_string(), number))
}

fn parse_github_pr_url(value: &str) -> Option<(String, String, i64)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"github\.com/([^/\s]+)/([^/\s]+)/pull/(\d+)").expect("valid regex")
    });
    let captures = re.captures(value)?;
    let owner = captures.get(1)?.as_str().to_string();
    let repo = captures
        .get(2)?
        .as_str()
        .trim_end_matches(".git")
        .to_string();
    let number = captures.get(3)?.as_str().parse().ok()?;
    Some((owner, repo, number))
}

fn current_repo_owner_name(repo_root: &Path) -> Result<(String, String)> {
    let data = run_gh_json(&["repo", "view", "--json", "nameWithOwner"], repo_root)?;
    let value = data
        .get("nameWithOwner")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Error: unable to resolve repository name."))?;
    let (owner, repo) = value
        .split_once('/')
        .ok_or_else(|| anyhow!("Error: invalid repository nameWithOwner."))?;
    Ok((owner.to_string(), repo.to_string()))
}

fn parse_pr_number(value: &str) -> Result<i64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(\d+)(?:/?$)").expect("valid regex"));
    re.captures(value)
        .and_then(|captures| captures.get(1))
        .and_then(|matched| matched.as_str().parse::<i64>().ok())
        .ok_or_else(|| anyhow!("Error: unable to parse PR number from {value}"))
}

fn fetch_checks(pr: &str, repo_root: &Path) -> Result<Vec<Value>> {
    let primary_fields = "name,state,conclusion,detailsUrl,startedAt,completedAt";
    let mut result = run_gh(&["pr", "checks", pr, "--json", primary_fields], repo_root);

    if result.code != 0 {
        let message = joined_message(&result);
        let available_fields = parse_available_fields(&message);
        if available_fields.is_empty() {
            bail!(
                "{}",
                if message.is_empty() {
                    "Error: gh pr checks failed."
                } else {
                    &message
                }
            );
        }
        let fallback = [
            "name",
            "state",
            "bucket",
            "link",
            "startedAt",
            "completedAt",
            "workflow",
        ];
        let selected: Vec<&str> = fallback
            .into_iter()
            .filter(|field| available_fields.iter().any(|available| available == field))
            .collect();
        if selected.is_empty() {
            bail!("Error: no usable fields available for gh pr checks.");
        }
        let selected = selected.join(",");
        result = run_gh(&["pr", "checks", pr, "--json", &selected], repo_root);
        if result.code != 0 {
            let message = joined_message(&result);
            bail!(
                "{}",
                if message.is_empty() {
                    "Error: gh pr checks failed."
                } else {
                    &message
                }
            );
        }
    }

    let data: Value =
        serde_json::from_str(&result.stdout).context("Error: unable to parse checks JSON.")?;
    data.as_array()
        .cloned()
        .ok_or_else(|| anyhow!("Error: unexpected checks JSON shape."))
}

fn analyze_check(
    check: &Value,
    repo_root: &Path,
    max_lines: usize,
    context: usize,
) -> Result<Value> {
    let url = value_text(check.get("detailsUrl"))
        .or_else(|| value_text(check.get("link")))
        .unwrap_or_default();
    let run_id = extract_run_id(&url);
    let job_id = extract_job_id(&url);
    let mut base = json!({
        "name": value_text(check.get("name")).unwrap_or_default(),
        "detailsUrl": url,
        "runId": run_id,
        "jobId": job_id,
    });

    let Some(run_id) = run_id else {
        base["status"] = json!("external");
        base["note"] = json!("No GitHub Actions run id detected in detailsUrl.");
        return Ok(base);
    };

    let metadata = fetch_run_metadata(&run_id, repo_root);
    let (log_text, log_error, log_status) = fetch_check_log(&run_id, job_id.as_deref(), repo_root);

    if log_status == "pending" {
        base["status"] = json!("log_pending");
        base["note"] = json!(if log_error.is_empty() {
            "Logs are not available yet."
        } else {
            &log_error
        });
        if let Some(metadata) = metadata {
            base["run"] = metadata;
        }
        return Ok(base);
    }

    if !log_error.is_empty() {
        base["status"] = json!("log_unavailable");
        base["error"] = json!(log_error);
        if let Some(metadata) = metadata {
            base["run"] = metadata;
        }
        return Ok(base);
    }

    let snippet = extract_failure_snippet(&log_text, max_lines, context);
    let tail = tail_lines(&log_text, max_lines);
    base["status"] = json!("ok");
    base["run"] = metadata.unwrap_or_else(|| json!({}));
    base["failureType"] = json!(classify_failure(
        &value_text(check.get("name")).unwrap_or_default(),
        &snippet,
        &tail
    ));
    base["failureMarkerLine"] = json!(failure_marker_line(&log_text));
    base["logSnippet"] = json!(snippet);
    base["logTail"] = json!(tail);
    Ok(base)
}

fn fetch_run_metadata(run_id: &str, repo_root: &Path) -> Option<Value> {
    let fields = "conclusion,status,workflowName,name,event,headBranch,headSha,url";
    run_gh_json(&["run", "view", run_id, "--json", fields], repo_root).ok()
}

fn fetch_check_log(
    run_id: &str,
    job_id: Option<&str>,
    repo_root: &Path,
) -> (String, String, String) {
    let (log_text, log_error) = fetch_run_log(run_id, repo_root);
    if log_error.is_empty() {
        return (log_text, String::new(), "ok".to_string());
    }

    if is_log_pending_message(&log_error) && job_id.is_some() {
        let (job_log, job_error) = fetch_job_log(job_id.expect("checked"), repo_root);
        if !job_log.is_empty() {
            return (job_log, String::new(), "ok".to_string());
        }
        if !job_error.is_empty() && is_log_pending_message(&job_error) {
            return (String::new(), job_error, "pending".to_string());
        }
        if !job_error.is_empty() {
            return (String::new(), job_error, "error".to_string());
        }
        return (String::new(), log_error, "pending".to_string());
    }

    if is_log_pending_message(&log_error) {
        return (String::new(), log_error, "pending".to_string());
    }
    (String::new(), log_error, "error".to_string())
}

fn fetch_run_log(run_id: &str, repo_root: &Path) -> (String, String) {
    let result = run_gh(&["run", "view", run_id, "--log"], repo_root);
    if result.code != 0 {
        let message = joined_message(&result);
        return (
            String::new(),
            if message.is_empty() {
                "gh run view failed".to_string()
            } else {
                message
            },
        );
    }
    (result.stdout, String::new())
}

fn fetch_job_log(job_id: &str, repo_root: &Path) -> (String, String) {
    let repo_slug = match fetch_repo_slug(repo_root) {
        Some(value) => value,
        None => {
            return (
                String::new(),
                "Error: unable to resolve repository name for job logs.".to_string(),
            )
        }
    };
    let endpoint = format!("/repos/{repo_slug}/actions/jobs/{job_id}/logs");
    let result = run_gh(&["api", &endpoint], repo_root);
    if result.code != 0 {
        let message = joined_message(&result);
        return (
            String::new(),
            if message.is_empty() {
                "gh api job logs failed".to_string()
            } else {
                message
            },
        );
    }
    if result.stdout.as_bytes().starts_with(b"PK") {
        return (
            String::new(),
            "Job logs returned a zip archive; unable to parse.".to_string(),
        );
    }
    (result.stdout, String::new())
}

fn fetch_repo_slug(repo_root: &Path) -> Option<String> {
    run_gh_json(&["repo", "view", "--json", "nameWithOwner"], repo_root)
        .ok()
        .and_then(|data| {
            data.get("nameWithOwner")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn fetch_all_comments(owner: &str, repo: &str, number: i64, repo_root: &Path) -> Result<Value> {
    let mut conversation_comments = Vec::new();
    let mut reviews = Vec::new();
    let mut review_threads = Vec::new();
    let mut comments_cursor: Option<String> = None;
    let mut reviews_cursor: Option<String> = None;
    let mut threads_cursor: Option<String> = None;
    let mut pr_meta: Option<Value> = None;

    loop {
        let payload = gh_api_graphql(
            repo_root,
            owner,
            repo,
            number,
            comments_cursor.as_deref(),
            reviews_cursor.as_deref(),
            threads_cursor.as_deref(),
        )?;
        if payload
            .get("errors")
            .and_then(Value::as_array)
            .is_some_and(|errors| !errors.is_empty())
        {
            bail!(
                "GitHub GraphQL errors:\n{}",
                serde_json::to_string_pretty(&payload["errors"])?
            );
        }
        let pr = payload
            .pointer("/data/repository/pullRequest")
            .ok_or_else(|| anyhow!("Error: missing pullRequest in GraphQL response."))?;

        if pr_meta.is_none() {
            pr_meta = Some(json!({
                "number": pr.get("number").cloned().unwrap_or(Value::Null),
                "url": pr.get("url").cloned().unwrap_or(Value::Null),
                "title": pr.get("title").cloned().unwrap_or(Value::Null),
                "state": pr.get("state").cloned().unwrap_or(Value::Null),
                "owner": owner,
                "repo": repo,
            }));
        }

        extend_nodes(&mut conversation_comments, pr.pointer("/comments/nodes"));
        extend_nodes(&mut reviews, pr.pointer("/reviews/nodes"));
        extend_nodes(&mut review_threads, pr.pointer("/reviewThreads/nodes"));

        comments_cursor = next_cursor(pr.pointer("/comments/pageInfo"));
        reviews_cursor = next_cursor(pr.pointer("/reviews/pageInfo"));
        threads_cursor = next_cursor(pr.pointer("/reviewThreads/pageInfo"));

        if comments_cursor.is_none() && reviews_cursor.is_none() && threads_cursor.is_none() {
            break;
        }
    }

    let summary = comments_summary(&conversation_comments, &reviews, &review_threads);
    Ok(json!({
        "pull_request": pr_meta.unwrap_or_else(|| json!({})),
        "summary": summary,
        "conversation_comments": conversation_comments,
        "reviews": reviews,
        "review_threads": review_threads,
    }))
}

fn comments_summary(
    conversation_comments: &[Value],
    reviews: &[Value],
    review_threads: &[Value],
) -> Value {
    let unresolved_thread_count = review_threads
        .iter()
        .filter(|thread| {
            !thread
                .get("isResolved")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let outdated_thread_count = review_threads
        .iter()
        .filter(|thread| {
            thread
                .get("isOutdated")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let actionable_thread_count = review_threads
        .iter()
        .filter(|thread| {
            !thread
                .get("isResolved")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && !thread
                    .get("isOutdated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
        .count();

    json!({
        "conversation_comment_count": conversation_comments.len(),
        "review_count": reviews.len(),
        "review_thread_count": review_threads.len(),
        "unresolved_thread_count": unresolved_thread_count,
        "outdated_thread_count": outdated_thread_count,
        "actionable_thread_count": actionable_thread_count,
    })
}

fn filter_open_review_threads(payload: &mut Value) {
    if let Some(threads) = payload
        .get_mut("review_threads")
        .and_then(Value::as_array_mut)
    {
        threads.retain(|thread| {
            !thread
                .get("isResolved")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && !thread
                    .get("isOutdated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        });
    }

    let empty = Vec::new();
    let conversation_comments = payload
        .get("conversation_comments")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let reviews = payload
        .get("reviews")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    let threads = payload
        .get("review_threads")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    payload["summary"] = comments_summary(conversation_comments, reviews, threads);
}

fn gh_api_graphql(
    repo_root: &Path,
    owner: &str,
    repo: &str,
    number: i64,
    comments_cursor: Option<&str>,
    reviews_cursor: Option<&str>,
    threads_cursor: Option<&str>,
) -> Result<Value> {
    let mut args = vec![
        "api".to_string(),
        "graphql".to_string(),
        "-F".to_string(),
        "query=@-".to_string(),
        "-F".to_string(),
        format!("owner={owner}"),
        "-F".to_string(),
        format!("repo={repo}"),
        "-F".to_string(),
        format!("number={number}"),
    ];
    if let Some(cursor) = comments_cursor {
        args.extend(["-F".to_string(), format!("commentsCursor={cursor}")]);
    }
    if let Some(cursor) = reviews_cursor {
        args.extend(["-F".to_string(), format!("reviewsCursor={cursor}")]);
    }
    if let Some(cursor) = threads_cursor {
        args.extend(["-F".to_string(), format!("threadsCursor={cursor}")]);
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = run_command(
        &["gh"].into_iter().chain(arg_refs).collect::<Vec<_>>(),
        Some(repo_root),
        Some(REVIEW_THREADS_QUERY),
    );
    if result.code != 0 {
        let message = joined_message(&result);
        bail!(
            "{}",
            if message.is_empty() {
                "Error: gh api graphql failed."
            } else {
                &message
            }
        );
    }
    serde_json::from_str(&result.stdout).context("Failed to parse JSON from gh api graphql output.")
}

fn extend_nodes(target: &mut Vec<Value>, nodes: Option<&Value>) {
    if let Some(items) = nodes.and_then(Value::as_array) {
        target.extend(items.iter().cloned());
    }
}

fn next_cursor(page_info: Option<&Value>) -> Option<String> {
    let info = page_info?;
    if !info
        .get("hasNextPage")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    info.get("endCursor")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn run_gh_json(args: &[&str], repo_root: &Path) -> Result<Value> {
    let result = run_gh(args, repo_root);
    if result.code != 0 {
        let message = joined_message(&result);
        bail!(
            "{}",
            if message.is_empty() {
                "Error: gh command failed."
            } else {
                &message
            }
        );
    }
    serde_json::from_str(&result.stdout).context("Error: unable to parse gh JSON.")
}

fn run_gh(args: &[&str], repo_root: &Path) -> GhResult {
    let mut command_args = vec!["gh"];
    command_args.extend(args);
    run_command(&command_args, Some(repo_root), None)
}

fn run_command(args: &[&str], cwd: Option<&Path>, stdin: Option<&str>) -> GhResult {
    let Some((program, rest)) = args.split_first() else {
        return GhResult {
            code: 1,
            stdout: String::new(),
            stderr: "empty command".to_string(),
        };
    };
    let mut command = Command::new(program);
    command.args(rest);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    if stdin.is_some() {
        command.stdin(std::process::Stdio::piped());
    }
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let output = if let Some(stdin) = stdin {
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return GhResult {
                    code: 1,
                    stdout: String::new(),
                    stderr: error.to_string(),
                }
            }
        };
        if let Some(mut child_stdin) = child.stdin.take() {
            use std::io::Write;
            if let Err(error) = child_stdin.write_all(stdin.as_bytes()) {
                return GhResult {
                    code: 1,
                    stdout: String::new(),
                    stderr: error.to_string(),
                };
            }
        }
        child.wait_with_output()
    } else {
        command.output()
    };

    match output {
        Ok(output) => GhResult {
            code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        },
        Err(error) => GhResult {
            code: 1,
            stdout: String::new(),
            stderr: error.to_string(),
        },
    }
}

fn is_failing(check: &Value) -> bool {
    let conclusion = normalized(check.get("conclusion"));
    if FAILURE_CONCLUSIONS.contains(&conclusion.as_str()) {
        return true;
    }
    let state = normalized(check.get("state").or_else(|| check.get("status")));
    if FAILURE_STATES.contains(&state.as_str()) {
        return true;
    }
    let bucket = normalized(check.get("bucket"));
    FAILURE_BUCKETS.contains(&bucket.as_str())
}

fn classify_failure(check_name: &str, snippet: &str, tail: &str) -> &'static str {
    let haystack = format!("{check_name}\n{snippet}\n{tail}").to_ascii_lowercase();
    if contains_any(&haystack, MISSING_SECRET_MARKERS) {
        return "missing-secret-or-env";
    }
    if contains_any(&haystack, LINT_MARKERS) {
        return "lint";
    }
    if contains_any(&haystack, TYPECHECK_MARKERS) {
        return "typecheck";
    }
    if contains_any(&haystack, TEST_MARKERS) {
        return "unit-test";
    }
    if contains_any(&haystack, BUILD_MARKERS) {
        return "build";
    }
    if contains_any(&haystack, FLAKE_MARKERS) {
        return "infra-or-flake";
    }
    "unknown"
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn normalized(value: Option<&Value>) -> String {
    value_text(value)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn value_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn extract_run_id(url: &str) -> Option<String> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            vec![
                Regex::new(r"/actions/runs/(\d+)").expect("valid regex"),
                Regex::new(r"/runs/(\d+)").expect("valid regex"),
            ]
        })
        .iter()
        .find_map(|pattern| {
            pattern
                .captures(url)
                .and_then(|captures| captures.get(1))
                .map(|matched| matched.as_str().to_string())
        })
}

fn extract_job_id(url: &str) -> Option<String> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            vec![
                Regex::new(r"/actions/runs/\d+/job/(\d+)").expect("valid regex"),
                Regex::new(r"/job/(\d+)").expect("valid regex"),
            ]
        })
        .iter()
        .find_map(|pattern| {
            pattern
                .captures(url)
                .and_then(|captures| captures.get(1))
                .map(|matched| matched.as_str().to_string())
        })
}

fn parse_available_fields(message: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut collecting = false;
    for line in message.lines() {
        if line.contains("Available fields:") {
            collecting = true;
            continue;
        }
        if !collecting {
            continue;
        }
        let field = line.trim();
        if !field.is_empty() {
            fields.push(field.to_string());
        }
    }
    fields
}

fn is_log_pending_message(message: &str) -> bool {
    let lowered = message.to_ascii_lowercase();
    PENDING_LOG_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
}

fn extract_failure_snippet(log_text: &str, max_lines: usize, context: usize) -> String {
    let lines: Vec<&str> = log_text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let Some(marker_index) = find_failure_index(&lines) else {
        return tail_lines(log_text, max_lines);
    };
    let start = marker_index.saturating_sub(context);
    let end = (marker_index + context).min(lines.len());
    let mut window = lines[start..end].to_vec();
    if window.len() > max_lines {
        window = window[window.len() - max_lines..].to_vec();
    }
    window.join("\n")
}

fn find_failure_index(lines: &[&str]) -> Option<usize> {
    lines.iter().enumerate().rev().find_map(|(idx, line)| {
        let lowered = line.to_ascii_lowercase();
        FAILURE_MARKERS
            .iter()
            .any(|marker| lowered.contains(marker))
            .then_some(idx)
    })
}

fn failure_marker_line(log_text: &str) -> Option<String> {
    let lines: Vec<&str> = log_text.lines().collect();
    let idx = find_failure_index(&lines)?;
    Some(lines[idx].to_string())
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

fn render_check_results(pr: &str, results: &[Value]) {
    println!("PR #{pr}: {} failing checks analyzed.", results.len());
    for result in results {
        println!("{}", "-".repeat(60));
        println!(
            "Check: {}",
            value_text(result.get("name")).unwrap_or_default()
        );
        if let Some(url) = value_text(result.get("detailsUrl")).filter(|url| !url.is_empty()) {
            println!("Details: {url}");
        }
        if let Some(run_id) = value_text(result.get("runId")).filter(|id| !id.is_empty()) {
            println!("Run ID: {run_id}");
        }
        if let Some(job_id) = value_text(result.get("jobId")).filter(|id| !id.is_empty()) {
            println!("Job ID: {job_id}");
        }
        println!(
            "Status: {}",
            value_text(result.get("status")).unwrap_or_else(|| "unknown".to_string())
        );
        if let Some(failure_type) =
            value_text(result.get("failureType")).filter(|kind| !kind.is_empty())
        {
            println!("Type: {failure_type}");
        }

        if let Some(run_meta) = result
            .get("run")
            .and_then(Value::as_object)
            .filter(|meta| !meta.is_empty())
        {
            let branch = run_meta
                .get("headBranch")
                .and_then(Value::as_str)
                .unwrap_or("");
            let sha = run_meta
                .get("headSha")
                .and_then(Value::as_str)
                .map(|sha| truncate_chars(sha, 12))
                .unwrap_or_default();
            let workflow = run_meta
                .get("workflowName")
                .or_else(|| run_meta.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let conclusion = run_meta
                .get("conclusion")
                .or_else(|| run_meta.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("");
            println!("Workflow: {workflow} ({conclusion})");
            if !branch.is_empty() || !sha.is_empty() {
                println!("Branch/SHA: {branch} {sha}");
            }
            if let Some(url) = run_meta.get("url").and_then(Value::as_str) {
                println!("Run URL: {url}");
            }
        }

        if let Some(note) = value_text(result.get("note")).filter(|note| !note.is_empty()) {
            println!("Note: {note}");
        }
        if let Some(error) = value_text(result.get("error")).filter(|error| !error.is_empty()) {
            println!("Error fetching logs: {error}");
            continue;
        }
        if let Some(snippet) =
            value_text(result.get("logSnippet")).filter(|snippet| !snippet.is_empty())
        {
            println!("Failure snippet:");
            println!("{}", indent_block(&snippet, "  "));
        } else {
            println!("No snippet available.");
        }
    }
    println!("{}", "-".repeat(60));
}

fn render_comment_summary(payload: &Value) {
    let pr = payload.get("pull_request").unwrap_or(&Value::Null);
    let summary = payload.get("summary").unwrap_or(&Value::Null);
    let number = value_text(pr.get("number")).unwrap_or_else(|| "?".to_string());
    let title = value_text(pr.get("title")).unwrap_or_default();
    let url = value_text(pr.get("url")).unwrap_or_default();

    println!("PR #{number}: {title}");
    if !url.is_empty() {
        println!("URL: {url}");
    }
    println!(
        "Comments: conversation={}, reviews={}, threads={}, unresolved={}, actionable={}",
        summary
            .get("conversation_comment_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        summary
            .get("review_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        summary
            .get("review_thread_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        summary
            .get("unresolved_thread_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        summary
            .get("actionable_thread_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    );

    if let Some(threads) = payload.get("review_threads").and_then(Value::as_array) {
        for (idx, thread) in threads.iter().enumerate() {
            let resolved = thread
                .get("isResolved")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let outdated = thread
                .get("isOutdated")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let status = if resolved {
                "resolved"
            } else if outdated {
                "outdated"
            } else {
                "open"
            };
            let path = value_text(thread.get("path")).unwrap_or_default();
            let line = value_text(thread.get("line")).unwrap_or_default();
            println!("{}. [{status}] {path}:{line}", idx + 1);
            if let Some(body) = first_thread_comment_body(thread) {
                println!("   {}", one_line(&body, 180));
            }
        }
    }
}

fn render_doctor_report(report: &DoctorReport) {
    println!("gh-source-gate doctor: {}", report.status);
    for check in &report.checks {
        let status = if check.ok { "ok" } else { "fail" };
        println!("- [{status}] {}: {}", check.name, check.detail);
    }
}

fn first_thread_comment_body(thread: &Value) -> Option<String> {
    thread
        .pointer("/comments/nodes")
        .and_then(Value::as_array)
        .and_then(|nodes| nodes.first())
        .and_then(|comment| value_text(comment.get("body")))
}

fn one_line(value: &str, max_len: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_len {
        compact
    } else {
        format!("{}...", compact.chars().take(max_len).collect::<String>())
    }
}

fn indent_block(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_chars(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}

fn joined_message(result: &GhResult) -> String {
    [result.stderr.trim(), result.stdout.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn collect_files_with_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut results = Vec::new();
    collect_files(root, &mut |path| {
        if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            results.push(path.to_path_buf());
        }
    });
    results
}

fn markdown_text_under(roots: &[PathBuf]) -> String {
    let mut chunks = Vec::new();
    for root in roots {
        collect_files(root, &mut |path| {
            if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                if let Ok(text) = fs::read_to_string(path) {
                    chunks.push(text);
                }
            }
        });
    }
    chunks.join("\n")
}

fn collect_files(root: &Path, visitor: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, visitor);
        } else if path.is_file() {
            visitor(&path);
        }
    }
}

fn repo_relative(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_run_and_job_ids() {
        let url = "https://github.com/a/b/actions/runs/123456/job/7890";
        assert_eq!(extract_run_id(url).as_deref(), Some("123456"));
        assert_eq!(extract_job_id(url).as_deref(), Some("7890"));
    }

    #[test]
    fn parses_github_pr_url() {
        assert_eq!(
            parse_github_pr_url("https://github.com/acme/widgets/pull/42"),
            Some(("acme".to_string(), "widgets".to_string(), 42))
        );
    }

    #[test]
    fn extracts_failure_snippet_near_last_marker() {
        let log = "setup\nfirst error\nok\nmore\nfatal: bad\nend";
        assert_eq!(extract_failure_snippet(log, 3, 2), "more\nfatal: bad\nend");
    }

    #[test]
    fn detects_failing_check_shapes() {
        assert!(is_failing(&json!({"conclusion": "failure"})));
        assert!(is_failing(&json!({"state": "TIMED_OUT"})));
        assert!(is_failing(&json!({"bucket": "fail"})));
        assert!(!is_failing(&json!({"conclusion": "success"})));
    }

    #[test]
    fn classifies_common_failure_types() {
        assert_eq!(classify_failure("lint", "cargo clippy failed", ""), "lint");
        assert_eq!(classify_failure("build", "compilation error", ""), "build");
        assert_eq!(classify_failure("tests", "pytest failed", ""), "unit-test");
        assert_eq!(
            classify_failure("deploy", "Error: bad credentials", ""),
            "missing-secret-or-env"
        );
    }

    #[test]
    fn summarizes_and_filters_review_threads() {
        let conversation = vec![json!({"id": "c1"})];
        let reviews = vec![json!({"id": "r1"})];
        let threads = vec![
            json!({"id": "t1", "isResolved": false, "isOutdated": false}),
            json!({"id": "t2", "isResolved": true, "isOutdated": false}),
            json!({"id": "t3", "isResolved": false, "isOutdated": true}),
        ];
        let summary = comments_summary(&conversation, &reviews, &threads);
        assert_eq!(summary["actionable_thread_count"], 1);
        assert_eq!(summary["unresolved_thread_count"], 2);

        let mut payload = json!({
            "conversation_comments": conversation,
            "reviews": reviews,
            "review_threads": threads,
        });
        filter_open_review_threads(&mut payload);
        assert_eq!(payload["review_threads"].as_array().unwrap().len(), 1);
        assert_eq!(payload["summary"]["review_thread_count"], 1);
    }

    #[test]
    fn parses_available_fields_from_gh_error() {
        let fields = parse_available_fields("bad\nAvailable fields:\n  name\n  bucket\n");
        assert_eq!(fields, vec!["name", "bucket"]);
    }

    #[test]
    fn doctor_detects_fully_rust_owned_fixture() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(
            &root.join("rust_tools/Cargo.toml"),
            r#"[workspace]
members = ["gh_source_gate_rs"]
"#,
        );
        write_file(&root.join("rust_tools/gh_source_gate_rs/Cargo.toml"), "");
        write_file(
            &root.join("rust_tools/gh_source_gate_rs/src/main.rs"),
            r#"
enum Commands {
    InspectPrChecks(InspectPrChecksArgs),
    FetchComments(FetchCommentsArgs),
    Doctor(DoctorArgs),
}
fn inspect_pr_checks() {}
fn fetch_comments() {}
fn doctor() {}
fn classify_failure() {}
fn filter_open_review_threads() {}
const REVIEW_THREADS_QUERY: &str = "";
const SUMMARY_MARKER: &str = "summary";
"#,
        );
        write_file(
            &root.join("skills/gh-fix-ci/SKILL.md"),
            "gh_source_gate_rs gh-source-gate inspect-pr-checks fetch-comments --open-only",
        );
        write_file(
            &root.join("skills/gh-address-comments/SKILL.md"),
            "gh_source_gate_rs gh-source-gate inspect-pr-checks fetch-comments --open-only",
        );
        for path in [
            "skills/SKILL_MANIFEST.json",
            "skills/SKILL_ROUTING_RUNTIME.json",
            "skills/SKILL_ROUTING_REGISTRY.md",
            "skills/SKILL_ROUTING_INDEX.md",
            "skills/SKILL_APPROVAL_POLICY.json",
        ] {
            write_file(&root.join(path), "gh-source-gate");
        }

        let report = build_doctor_report(root);
        assert_eq!(report.status, "ok");
        assert!(report.checks.iter().all(|check| check.ok));
    }

    #[test]
    fn doctor_rejects_retired_python_helpers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        write_file(
            &root.join("skills/gh-fix-ci/scripts/inspect_pr_checks.py"),
            "print('old helper')",
        );
        let report = build_doctor_report(root);
        assert_eq!(report.status, "failed");
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "retired-python-helper-files" && !check.ok));
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write file");
    }
}
