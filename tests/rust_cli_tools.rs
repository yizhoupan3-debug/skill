mod common;

use common::{
    assert_success, cargo_manifest_command, json_from_output, project_root, read_text, run,
};
use serde_json::Value;
use std::process::{Command, Output};
use tempfile::tempdir;

#[test]
fn financial_data_rejects_zero_limit() {
    let result = run_financial_data_error(&[
        "ohlcv", "--market", "crypto", "--symbol", "BTC/USDT", "--limit", "0",
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("--limit must be greater than zero"));
}

#[test]
fn financial_data_rejects_adjusted_stooq() {
    let result = run_financial_data_error(&[
        "ohlcv",
        "--market",
        "us",
        "--symbol",
        "AAPL",
        "--source",
        "stooq",
        "--adjusted",
    ]);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("Stooq does not support adjusted OHLCV")
    );
}

#[test]
fn image_generated_rust_cli_is_workspace_member() {
    let manifest = read_text(&project_root().join("rust_tools/Cargo.toml"));
    assert!(manifest.contains(r#""image_gen_rs""#));
}

#[test]
fn image_generated_skill_docs_point_to_rust_cli_only() {
    let docs = collect_text_under(&project_root().join("skills/image-generated"), "md");
    assert!(docs.contains("rust_tools/image_gen_rs"));
    assert!(!docs.contains("scripts/image_gen.py"));
    assert!(!docs.contains("python \"$IMAGE_GEN\""));
}

#[test]
fn update_audit_cli_contract_is_registered() {
    let args = read_text(&project_root().join("scripts/router-rs/src/cli/args.inc"));
    let maint = read_text(&project_root().join("scripts/router-rs/src/framework_maint.rs"));
    assert!(args.contains("UpdateAudit(UpdateAuditArgs)"));
    assert!(args.contains("repo_root: Option<PathBuf>"));
    assert!(args.contains("Dry-run `/update` repository knowledge/hygiene audit"));
    assert!(maint.contains("MaintSubcommand::UpdateAudit(args) => update_audit(args)"));
    assert!(maint.contains("\"schema_version\": \"framework-maint-update-audit-v1\""));
    for key in [
        "key_document_candidates",
        "git_tracking",
        "suspected_dead_code_markers",
        "suspected_stale_docs",
        "suspected_retired_files",
        "recommended_actions",
    ] {
        assert!(
            maint.contains(key),
            "missing update-audit output key: {key}"
        );
    }
    assert!(maint.contains("research data"));
}

#[test]
fn refresh_host_projections_includes_claude_projection_verification() {
    let args = read_text(&project_root().join("scripts/router-rs/src/cli/args.inc"));
    let maint = read_text(&project_root().join("scripts/router-rs/src/framework_maint.rs"));
    assert!(args.contains("non-Codex framework installs"));
    assert!(maint.contains("for tool in [\"cursor\", \"claude\"]"));
    assert!(maint.contains("verify_claude_projection(&fw)"));
    assert!(maint.contains(".claude/rules/framework.md"));
    assert!(maint.contains(".claude/.framework-projection.json"));
    assert!(maint.contains("host_projection: claude-code"));
}

#[test]
fn update_audit_runs_on_plain_git_repo_and_preserves_status_columns() {
    let tmp = tempdir().unwrap();
    let mut git_init = Command::new("git");
    git_init.arg("init").current_dir(tmp.path());
    assert_success(&run(git_init));
    std::fs::write(tmp.path().join("README.md"), "initial\n").unwrap();
    let mut git_add = Command::new("git");
    git_add.args(["add", "README.md"]).current_dir(tmp.path());
    assert_success(&run(git_add));
    let mut git_commit = Command::new("git");
    git_commit
        .args([
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=Test User",
            "commit",
            "-m",
            "seed",
        ])
        .current_dir(tmp.path());
    assert_success(&run(git_commit));
    std::fs::write(tmp.path().join("README.md"), "changed\n").unwrap();
    std::fs::create_dir_all(tmp.path().join("notes")).unwrap();
    std::fs::write(
        tmp.path().join("notes/research-plan.md"),
        "TODO: update experiment plan\n",
    )
    .unwrap();

    let output = run(cargo_manifest_command(
        &project_root().join("scripts/router-rs/Cargo.toml"),
        &[
            "framework",
            "maint",
            "update-audit",
            "--repo-root",
            tmp.path().to_str().unwrap(),
        ],
    ));
    let payload = json_from_output(&output);
    assert_eq!(payload["schema_version"], "framework-maint-update-audit-v1");
    assert_eq!(payload["mode"], "dry-run");
    assert_eq!(payload["mutates_files"], false);
    let status = payload["git_tracking"]["status_porcelain"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        status.iter().any(|line| line.starts_with(" M README.md")),
        "status output should preserve porcelain leading columns: {status:?}"
    );
    let key_docs = payload["key_document_candidates"].as_array().unwrap();
    assert!(key_docs.iter().any(|item| {
        item["path"] == "notes/research-plan.md" && item["tracking"] == "untracked"
    }));
    let stale_docs = payload["suspected_stale_docs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    assert!(
        stale_docs
            .iter()
            .any(|line| line.starts_with("notes/research-plan.md:1:TODO")),
        "untracked research doc TODO should be scanned: {stale_docs:?}"
    );
}

#[test]
fn image_generated_generate_dry_run_emits_openai_images_payload() {
    let result = run_image_generated_ok(&[
        "generate",
        "--prompt",
        "red square",
        "--use-case",
        "infographic-diagram",
        "--dry-run",
    ]);
    let payload: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        payload["endpoint"],
        "https://api.openai.com/v1/images/generations"
    );
    assert_eq!(payload["model"], "dall-e-3");
    assert_eq!(payload["n"], 1);
    assert_eq!(payload["size"], "1024x1024");
    assert_eq!(payload["quality"], "auto");
    assert_eq!(payload["response_format"], "b64_json");
    let prompt = payload["prompt"].as_str().unwrap();
    assert!(prompt.starts_with("Use case: infographic-diagram"));
    assert!(prompt.contains("Primary request: red square"));
    let outputs = payload["outputs"].as_array().unwrap();
    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].as_str().unwrap().ends_with("output.png"));
}

#[test]
fn image_generated_batch_dry_run_has_single_out_dir_flag() {
    let tmp = tempdir().unwrap();
    let prompts = tmp.path().join("prompts.jsonl");
    std::fs::write(
        &prompts,
        "{\"prompt\":\"red square\",\"out\":\"red.png\"}\nblue circle\n",
    )
    .unwrap();
    let result = run_image_generated_ok(&[
        "generate-batch",
        "--input",
        prompts.to_str().unwrap(),
        "--out-dir",
        tmp.path().join("out").to_str().unwrap(),
        "--dry-run",
    ]);
    let payloads = parse_concatenated_json(&String::from_utf8_lossy(&result.stdout));
    assert_eq!(payloads.len(), 2);
    assert!(payloads[0]["outputs"][0]
        .as_str()
        .unwrap()
        .ends_with("/out/red.png"));
    assert!(payloads[1]["outputs"][0]
        .as_str()
        .unwrap()
        .ends_with("/out/002-blue-circle.png"));
}

#[test]
fn image_generated_dry_run_does_not_create_output_dirs() {
    let tmp = tempdir().unwrap();
    let out_dir = tmp.path().join("preview-only");
    run_image_generated_ok(&[
        "generate",
        "--prompt",
        "preview",
        "--out-dir",
        out_dir.to_str().unwrap(),
        "--dry-run",
    ]);
    assert!(!out_dir.exists());
}

#[test]
fn image_generated_rejects_invalid_output_compression() {
    let result = run_image_generated_error(&[
        "generate",
        "--prompt",
        "red square",
        "--output-compression",
        "101",
        "--dry-run",
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr)
        .contains("output-compression must be between 0 and 100"));
}

#[test]
fn image_generated_batch_slugs_non_ascii_prompts() {
    let tmp = tempdir().unwrap();
    let prompts = tmp.path().join("prompts.jsonl");
    std::fs::write(&prompts, "一只猫\n").unwrap();
    let result = run_image_generated_ok(&[
        "generate-batch",
        "--input",
        prompts.to_str().unwrap(),
        "--out-dir",
        tmp.path().join("out").to_str().unwrap(),
        "--dry-run",
    ]);
    let payload: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(payload["outputs"][0]
        .as_str()
        .unwrap()
        .ends_with("/out/001-image.png"));
}

fn run_financial_data_error(args: &[&str]) -> Output {
    run(cargo_manifest_command(
        &project_root().join("rust_tools/financial_data_rs/Cargo.toml"),
        args,
    ))
}

fn run_image_generated_ok(args: &[&str]) -> Output {
    let output = run(cargo_manifest_command(
        &project_root().join("rust_tools/image_gen_rs/Cargo.toml"),
        args,
    ));
    common::assert_success(&output);
    output
}

fn run_image_generated_error(args: &[&str]) -> Output {
    run(cargo_manifest_command(
        &project_root().join("rust_tools/image_gen_rs/Cargo.toml"),
        args,
    ))
}

fn parse_concatenated_json(text: &str) -> Vec<Value> {
    let mut payloads = Vec::new();
    let stream = serde_json::Deserializer::from_str(text).into_iter::<Value>();
    for payload in stream {
        payloads.push(payload.unwrap());
    }
    payloads
}

fn collect_text_under(root: &std::path::Path, extension: &str) -> String {
    let mut chunks = Vec::new();
    collect_files(root, extension, &mut |path| chunks.push(read_text(path)));
    chunks.join("\n")
}

fn collect_files(
    root: &std::path::Path,
    extension: &str,
    visitor: &mut dyn FnMut(&std::path::Path),
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, extension, visitor);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            visitor(&path);
        }
    }
}
