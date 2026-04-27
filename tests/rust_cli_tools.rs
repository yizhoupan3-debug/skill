mod common;

use common::{cargo_manifest_command, project_root, read_text, run};
use serde_json::Value;
use std::process::Output;
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
fn image_generated_generate_dry_run_uses_responses_tool() {
    let result = run_image_generated_ok(&[
        "generate",
        "--prompt",
        "red square",
        "--use-case",
        "infographic-diagram",
        "--dry-run",
    ]);
    let payload: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(payload["endpoint"], "http://127.0.0.1:8318/v1/responses");
    assert_eq!(payload["model"], "gpt-5.4");
    assert_eq!(payload["tools"][0]["type"], "image_generation");
    assert!(payload["input"]
        .as_str()
        .unwrap()
        .starts_with("Use case: infographic-diagram"));
    assert!(payload["input"]
        .as_str()
        .unwrap()
        .contains("Primary request: red square"));
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
