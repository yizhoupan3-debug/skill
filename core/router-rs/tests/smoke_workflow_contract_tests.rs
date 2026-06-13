//! workflow script static contract smoke (meta.phases shape).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

fn framework_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn workflow_scripts_dir() -> PathBuf {
    framework_repo_root().join(".claude/workflows")
}

static META_PHASE_OBJECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"phases\s*:\s*\[\s*\{[^}]*title\s*:")
        .expect("workflow meta object phases regex")
});

static META_PHASE_STRING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"phases\s*:\s*\[\s*['"]"#).expect("workflow meta string phases regex")
});

static META_PHASE_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{\s*title:\s*['"]([^'"]+)['"]"#).expect("workflow meta phase title regex")
});

static PHASE_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"phase\s*\(\s*['"]([^'"]+)['"]\s*\)"#).expect("workflow phase() call regex")
});

static AGENT_PHASE_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"phase:\s*['"]([^'"]+)['"]"#).expect("workflow agent phase tag regex")
});

fn workflow_meta_contract_ok(content: &str) -> Result<(), String> {
    if !content.contains("export const meta") {
        return Ok(());
    }
    if !content.contains("name:") || !content.contains("description:") {
        return Err("meta missing name or description".to_string());
    }
    if !META_PHASE_OBJECT.is_match(content) && !META_PHASE_STRING.is_match(content) {
        return Err(
            "meta.phases must be a non-empty array ({ title } objects or string titles)".to_string(),
        );
    }
    Ok(())
}

fn assert_workflow_dir_contract(workflow_dir: &Path) {
    assert!(
        workflow_dir.is_dir(),
        "workflow scripts dir missing: {}",
        workflow_dir.display()
    );
    let mut checked = 0usize;
    for entry in fs::read_dir(workflow_dir).expect("read workflow dir") {
        let entry = entry.expect("workflow dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read workflow script");
        if !content.contains("export const meta") {
            continue;
        }
        workflow_meta_contract_ok(&content).unwrap_or_else(|reason| {
            panic!(
                "workflow meta contract failed for {}: {reason}",
                path.file_name().unwrap().to_string_lossy()
            );
        });
        checked += 1;
    }
    assert!(
        checked >= 1,
        "expected at least one workflow script with export const meta under {}",
        workflow_dir.display()
    );
}

/// every workflow entry script exposes meta.phases with titles.
#[test]
fn workflow_single_phase_meta_contract_smoke() {
    assert_workflow_dir_contract(&workflow_scripts_dir());
}

fn extract_meta_phase_titles(content: &str) -> Vec<String> {
    if !content.contains("export const meta") {
        return Vec::new();
    }
    META_PHASE_TITLE
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

fn extract_phase_call_titles(content: &str) -> Vec<String> {
    PHASE_CALL
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

fn meta_titles_match_call_order(meta_titles: &[String], call_titles: &[String]) -> bool {
    let mut meta_idx = 0usize;
    for call in call_titles {
        if meta_idx < meta_titles.len() && meta_titles[meta_idx] == *call {
            meta_idx += 1;
        }
    }
    meta_idx == meta_titles.len()
}

fn workflow_multi_phase_sequential_contract(content: &str) -> Result<(), String> {
    if !content.contains("export const meta") {
        return Err("missing export const meta".to_string());
    }
    let meta_titles = extract_meta_phase_titles(content);
    if meta_titles.len() < 2 {
        return Err(format!(
            "meta.phases must declare at least 2 titled phases, got {}",
            meta_titles.len()
        ));
    }
    let call_titles = extract_phase_call_titles(content);
    if call_titles.len() < 2 {
        return Err(format!(
            "expected at least 2 phase() calls, got {}",
            call_titles.len()
        ));
    }
    if !meta_titles_match_call_order(&meta_titles, &call_titles) {
        return Err(format!(
            "phase() call order must follow meta.phases titles sequentially; meta={meta_titles:?} calls={call_titles:?}"
        ));
    }
    Ok(())
}

/// at least one workflow script runs multi-phase in meta order (static).
#[test]
fn workflow_multi_phase_sequential_smoke() {
    let workflow_dir = workflow_scripts_dir();
    assert!(
        workflow_dir.is_dir(),
        "workflow scripts dir missing: {}",
        workflow_dir.display()
    );
    let mut matched = Vec::new();
    for entry in fs::read_dir(&workflow_dir).expect("read workflow dir") {
        let entry = entry.expect("workflow dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read workflow script");
        if workflow_multi_phase_sequential_contract(&content).is_ok() {
            matched.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(
        !matched.is_empty(),
        "expected at least one multi-phase sequential workflow under {}",
        workflow_dir.display()
    );
}

fn top_level_pipeline_assignments(content: &str) -> Vec<&str> {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !line.starts_with(' ') && !line.starts_with('\t') && trimmed.starts_with("const ")
                && trimmed.contains("await pipeline(")
        })
        .collect()
}

fn workflow_pipeline_failure_isolation_ok(content: &str) -> Result<(), String> {
    let pipelines = top_level_pipeline_assignments(content);
    if pipelines.is_empty() {
        return Err("no top-level await pipeline() assignment".to_string());
    }
    if !content.contains(".catch(") {
        return Err(
            "top-level pipeline workflows must include .catch() failure isolation".to_string(),
        );
    }
    Ok(())
}

fn workflow_verify_outer_catch_ok(content: &str) -> Result<(), String> {
    let has_verify = content.contains("phase('Verify')") || content.contains("phase(\"Verify\")");
    if !has_verify {
        return Err("missing Verify phase".to_string());
    }
    if top_level_pipeline_assignments(content).is_empty() {
        return Err("Verify workflow missing top-level pipeline".to_string());
    }
    if !content.contains(").catch(") {
        return Err("Verify pipeline must end with outer ).catch() per workflow conventions".to_string());
    }
    let handles_null = content.contains("filter(Boolean)")
        || content.contains("!v")
        || content.contains("rejected.push")
        || content.contains("v?.");
    if !handles_null {
        return Err("Verify workflow must handle isolated null agent results".to_string());
    }
    Ok(())
}

/// top-level pipeline workflows isolate agent failures via .catch().
#[test]
fn workflow_error_isolation_smoke() {
    let workflow_dir = workflow_scripts_dir();
    let mut pipeline_isolated = Vec::new();
    let mut verify_isolated = Vec::new();
    for entry in fs::read_dir(&workflow_dir).expect("read workflow dir") {
        let entry = entry.expect("workflow dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read workflow script");
        if !content.contains("export const meta") {
            continue;
        }
        if workflow_pipeline_failure_isolation_ok(&content).is_ok() {
            pipeline_isolated.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
        if workflow_verify_outer_catch_ok(&content).is_ok() {
            verify_isolated.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(
        !pipeline_isolated.is_empty(),
        "expected at least one top-level pipeline workflow with .catch() isolation under {}",
        workflow_dir.display()
    );
    assert!(
        !verify_isolated.is_empty(),
        "expected at least one Verify-phase workflow with outer ).catch() and null handling under {}",
        workflow_dir.display()
    );
}

fn workflow_parallel_pipeline_contract(content: &str) -> Result<(), String> {
    if !content.contains("export const meta") {
        return Err("missing export const meta".to_string());
    }
    let has_parallel = content.contains("await parallel(") || content.contains("Promise.all(");
    let has_pipeline = content.contains("await pipeline(");
    if !has_parallel {
        return Err(
            "workflow must use await parallel(...) or Promise.all(...) for Scan-style concurrency"
                .to_string(),
        );
    }
    if !has_pipeline {
        return Err("workflow must use await pipeline(...) for Verify-style serial stages".to_string());
    }
    let scan_phase = content.contains("phase('Scan')") || content.contains("phase(\"Scan\")");
    let verify_phase = content.contains("phase('Verify')") || content.contains("phase(\"Verify\")");
    if !scan_phase || !verify_phase {
        return Err("parallel+pipeline workflow must declare Scan and Verify phases".to_string());
    }
    let parallel_pos = content
        .find("await parallel(")
        .or_else(|| content.find("Promise.all("))
        .ok_or_else(|| "missing parallel invocation".to_string())?;
    let pipeline_pos = content
        .find("await pipeline(")
        .ok_or_else(|| "missing pipeline invocation".to_string())?;
    if parallel_pos >= pipeline_pos {
        return Err(
            "parallel/Scan concurrency must precede pipeline/Verify serial stages in script order"
                .to_string(),
        );
    }
    Ok(())
}

/// at least one workflow combines Scan `parallel` with Verify `pipeline` (static).
#[test]
fn workflow_parallel_pipeline_smoke() {
    let workflow_dir = workflow_scripts_dir();
    let mut matched = Vec::new();
    for entry in fs::read_dir(&workflow_dir).expect("read workflow dir") {
        let entry = entry.expect("workflow dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read workflow script");
        if workflow_parallel_pipeline_contract(&content).is_ok() {
            matched.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(
        !matched.is_empty(),
        "expected at least one Scan parallel + Verify pipeline workflow under {} (e.g. deep-review-template.js); matched={matched:?}",
        workflow_dir.display()
    );
}

/// Phase-tagged agents must run under the nearest preceding `phase()` boundary (static isolation).
fn workflow_state_isolation_contract(content: &str) -> Result<(), String> {
    if !content.contains("export const meta") {
        return Err("missing export const meta".to_string());
    }
    if !content.contains("agent(") {
        return Err("not an agent workflow".to_string());
    }
    let meta_titles: std::collections::HashSet<String> =
        extract_meta_phase_titles(content).into_iter().collect();
    if meta_titles.len() < 2 {
        return Err(format!(
            "need at least 2 meta phases for isolation contract, got {}",
            meta_titles.len()
        ));
    }
    if content.contains("fork_context: true") || content.contains("fork_context:true") {
        return Err("workflows must not set fork_context:true (breaks inter-phase isolation)".to_string());
    }
    for cap in PHASE_CALL.captures_iter(content) {
        let title = cap[1].to_string();
        if !meta_titles.contains(&title) {
            return Err(format!("phase() title '{title}' not declared in meta.phases"));
        }
    }
    let phase_markers: Vec<(usize, String)> = PHASE_CALL
        .captures_iter(content)
        .map(|cap| (cap.get(0).unwrap().start(), cap[1].to_string()))
        .collect();
    if phase_markers.is_empty() {
        return Err("multi-phase workflows must call phase() boundaries".to_string());
    }
    let mut tagged = 0usize;
    for cap in AGENT_PHASE_TAG.captures_iter(content) {
        tagged += 1;
        let tag = cap[1].to_string();
        if !meta_titles.contains(&tag) {
            return Err(format!("agent phase tag '{tag}' not in meta.phases"));
        }
        let pos = cap.get(0).unwrap().start();
        let active = phase_markers
            .iter()
            .rev()
            .find(|(start, _)| *start <= pos)
            .map(|(_, title)| title)
            .ok_or_else(|| format!("agent phase tag '{tag}' appears before any phase() call"))?;
        if active != &tag {
            return Err(format!(
                "agent phase tag '{tag}' must match active phase '{active}' (state isolation)"
            ));
        }
    }
    if tagged == 0 {
        return Err("multi-phase workflows must tag agents with phase:".to_string());
    }
    Ok(())
}

/// workflow agents stay within declared phase boundaries (static).
#[test]
fn workflow_state_isolation_smoke() {
    let workflow_dir = workflow_scripts_dir();
    let mut matched = Vec::new();
    for entry in fs::read_dir(&workflow_dir).expect("read workflow dir") {
        let entry = entry.expect("workflow dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read workflow script");
        if workflow_state_isolation_contract(&content).is_ok() {
            matched.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(
        !matched.is_empty(),
        "expected at least one multi-phase workflow with phase-tagged agents under {} (e.g. deep-review-template.js); matched={matched:?}",
        workflow_dir.display()
    );
}
