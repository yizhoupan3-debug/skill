mod fixtures;

use fixtures::{blank_pages_pdf_in, hello_pdf_in, image_only_pdf_in};
use pdf_tool_rs::batch::{load_paths, run_batch, BatchOptions};
use pdf_tool_rs::read::shallow_scan_classify;
use pdf_tool_rs::schema::{ContentClass, ProcessStatus};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tempfile::tempdir;

fn batch_opts(out_dir: PathBuf) -> BatchOptions {
    BatchOptions {
        out_dir,
        jobs: 2,
        resume: false,
        fail_fast: false,
        max_chars: 8000,
    }
}

#[test]
fn hello_pdf_yields_extractable_text() {
    let tmp = tempdir().unwrap();
    let pdf = hello_pdf_in(tmp.path());
    let text = pdf_tool_rs::read::extract_text(&pdf).unwrap_or_default();
    assert!(
        text.contains("Hello") || text.contains("World"),
        "expected extractable text, got: {text:?}"
    );
}

#[test]
fn batch_writes_jsonl_catalog_and_text() {
    let tmp = tempdir().unwrap();
    let pdf = hello_pdf_in(tmp.path());
    let out = tmp.path().join("out");

    let manifest_path = tmp.path().join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::json!({ "paths": [pdf.display().to_string()] }).to_string(),
    )
    .unwrap();

    let paths = load_paths(Some(&manifest_path), false).unwrap();
    let summary = run_batch(paths, &batch_opts(out.clone()), false).unwrap();
    assert_eq!(summary.total, 1);
    assert_eq!(summary.processed, 1);

    let catalog: Value =
        serde_json::from_str(&fs::read_to_string(out.join("catalog.json")).unwrap()).unwrap();
    assert_eq!(catalog["processed"], 1);
    let class = catalog["entries"][0]["content_class"].as_str().unwrap();
    assert!(
        class == "text" || class == "mixed",
        "expected text or mixed, got {class}"
    );

    let jsonl = fs::read_to_string(out.join("results.jsonl")).unwrap();
    let line = jsonl.lines().next().expect("one jsonl line");
    let row: Value = serde_json::from_str(line).unwrap();
    assert_eq!(row["status"], "ok");
    let row_class = row["content_class"].as_str().unwrap();
    assert!(
        row_class == "text" || row_class == "mixed",
        "expected text or mixed, got {row_class}"
    );
    assert!(row["path"].as_str().unwrap().ends_with("hello.pdf"));
    assert!(row["sha256"].as_str().unwrap().len() == 64);
    assert!(row.get("warnings").unwrap().is_array());

    let text_rel = row["text_path"].as_str().unwrap();
    let text_body = fs::read_to_string(out.join(text_rel)).unwrap();
    assert!(text_body.contains("Hello") || text_body.contains("World"));

    assert!(out.join("index.md").exists());
    assert!(out.join("checkpoint.json").exists());
}

#[test]
fn batch_resume_skips_completed() {
    let tmp = tempdir().unwrap();
    let pdf = hello_pdf_in(tmp.path());
    let out = tmp.path().join("out");

    let manifest_path = tmp.path().join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::json!({ "paths": [pdf.display().to_string()] }).to_string(),
    )
    .unwrap();

    let paths = load_paths(Some(&manifest_path), false).unwrap();
    run_batch(paths.clone(), &batch_opts(out.clone()), false).unwrap();

    let jsonl_before = fs::read_to_string(out.join("results.jsonl")).unwrap();
    let lines_before = jsonl_before.lines().count();

    let mut resume_opts = batch_opts(out.clone());
    resume_opts.resume = true;
    let summary = run_batch(paths, &resume_opts, false).unwrap();
    assert_eq!(summary.processed, 1);

    let jsonl_after = fs::read_to_string(out.join("results.jsonl")).unwrap();
    let lines_after = jsonl_after.lines().count();
    assert_eq!(lines_before, lines_after, "resume must not duplicate jsonl rows");
}

#[test]
fn image_only_pdf_has_no_extractable_text() {
    let tmp = tempdir().unwrap();
    let pdf = image_only_pdf_in(tmp.path());
    let text = pdf_tool_rs::read::extract_text(&pdf).unwrap_or_default();
    assert!(
        text.trim().is_empty(),
        "image-only fixture should have no text layer, got: {text:?}"
    );
    let (_, class, should_skip) = shallow_scan_classify(&pdf).unwrap();
    assert_eq!(class, ContentClass::Scanned);
    assert!(should_skip);
}

#[test]
fn skip_scanned_shallow_probe_skips_blank_pages() {
    let tmp = tempdir().unwrap();
    let blank = blank_pages_pdf_in(tmp.path(), 3);
    let (_, class, should_skip) = shallow_scan_classify(&blank).unwrap();
    assert_eq!(class, ContentClass::Scanned);
    assert!(should_skip);

    let out = tmp.path().join("out");
    let opts = batch_opts(out.clone());
    let summary = run_batch(vec![blank], &opts, true).unwrap();
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.processed, 0);

    let jsonl = fs::read_to_string(out.join("results.jsonl")).unwrap();
    let row: serde_json::Value = serde_json::from_str(jsonl.lines().next().unwrap()).unwrap();
    assert_eq!(row["status"], "skipped");
    assert_eq!(row["content_class"], "scanned");
    assert!(
        row["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "skip_scanned")
    );
    assert!(row.get("text_path").is_none() || row["text_path"].is_null());
}

#[test]
fn skip_scanned_skips_image_only_pdf() {
    let tmp = tempdir().unwrap();
    let pdf = image_only_pdf_in(tmp.path());
    let out = tmp.path().join("out");
    let opts = batch_opts(out.clone());
    let summary = run_batch(vec![pdf], &opts, true).unwrap();
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.processed, 0);

    let jsonl = fs::read_to_string(out.join("results.jsonl")).unwrap();
    let row: serde_json::Value = serde_json::from_str(jsonl.lines().next().unwrap()).unwrap();
    assert_eq!(row["status"], "skipped");
    assert_eq!(row["content_class"], "scanned");
}

#[test]
fn skip_scanned_does_not_skip_text_pdf() {
    let tmp = tempdir().unwrap();
    let pdf = hello_pdf_in(tmp.path());
    let (_, _, should_skip) = shallow_scan_classify(&pdf).unwrap();
    assert!(!should_skip);

    let out = tmp.path().join("out");
    let opts = batch_opts(out.clone());
    let summary = run_batch(vec![pdf], &opts, true).unwrap();
    assert_eq!(summary.processed, 1);
    assert_eq!(summary.skipped, 0);
}

#[test]
fn jsonl_each_line_is_valid_file_result() {
    let tmp = tempdir().unwrap();
    let pdf = hello_pdf_in(tmp.path());
    let out = tmp.path().join("out");
    let paths = vec![pdf];
    run_batch(paths, &batch_opts(out.clone()), false).unwrap();

    let file = fs::File::open(out.join("results.jsonl")).unwrap();
    for line in BufReader::new(file).lines() {
        let line = line.unwrap();
        let parsed: pdf_tool_rs::schema::FileResult = serde_json::from_str(&line).unwrap();
        assert!(matches!(
            parsed.status,
            ProcessStatus::Ok | ProcessStatus::Error | ProcessStatus::Skipped
        ));
        assert!(matches!(
            parsed.content_class,
            ContentClass::Text
                | ContentClass::Scanned
                | ContentClass::Empty
                | ContentClass::Mixed
                | ContentClass::Error
        ));
    }
}
