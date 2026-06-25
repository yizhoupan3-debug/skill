//! Scoring pipeline benchmarks for skill routing (routing-engine).
//!
//! ```bash
//! cargo bench -p routing-engine --bench scoring_bench
//! cargo bench -p routing-engine --bench scoring_bench -- --sample-size 10
//! ```
//!
//! No env-var gate — runs unconditionally as a CI performance gate.
//! Generates synthetic SkillRecord data in-memory (no disk I/O).

use criterion::{BenchmarkId, Criterion, black_box};
use routing_engine::route::{search_skills, SkillRecord};
use std::collections::HashSet;
use std::time::{Duration, Instant};

// ── helpers ───────────────────────────────────────────────────────────────────

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn report_latency(label: &str, samples: &mut [Duration]) {
    samples.sort_unstable();
    let p50 = percentile(samples, 0.50);
    let p95 = percentile(samples, 0.95);
    eprintln!(
        "[scoring_bench] {label}: p50={}µs p95={}µs (n={})",
        p50.as_micros(),
        p95.as_micros(),
        samples.len()
    );
}

fn make_skill_records(count: usize) -> Vec<SkillRecord> {
    let layers = ["runtime", "internal", "framework", "research", "tool"];
    let owners = ["framework", "research", "codegraph", "browser", "core"];
    let gates = ["none", "guard", "sandbox"];
    let summaries = [
        "search and retrieve documents from the knowledge base",
        "analyze code patterns and detect potential bugs",
        "generate reports with structured formatting",
        "transform data between different schema formats",
        "validate configuration files against schema",
    ];
    let name_token_sets: &[&[&str]] = &[
        &["search", "retrieve", "find", "lookup"],
        &["analyze", "detect", "inspect", "audit"],
        &["generate", "report", "format", "produce"],
        &["transform", "convert", "parse", "translate"],
        &["validate", "check", "verify", "lint"],
    ];
    let keyword_sets: &[&[&str]] = &[
        &["search", "find", "grep", "lookup"],
        &["analysis", "bug", "pattern", "smell"],
        &["report", "summary", "format", "markdown"],
        &["convert", "transform", "schema", "mapping"],
        &["validate", "lint", "check", "schema"],
    ];

    (0..count)
        .map(|i| {
            let cat = i % 5;
            SkillRecord {
                slug: format!("skill-{}-{}", i, ["search", "analyze", "generate", "transform", "validate"][cat]),
                skill_path: Some(format!("skills/skill-{}/SKILL.md", i)),
                layer: layers[cat].to_string(),
                owner: owners[cat].to_string(),
                gate: gates[i % gates.len()].to_string(),
                priority: "normal".to_string(),
                session_start: "any".to_string(),
                summary: format!("Skill #{}: {}", i, summaries[cat]),
                slug_lower: format!("skill-{}-{}", i, ["search", "analyze", "generate", "transform", "validate"][cat]),
                owner_lower: owners[cat].to_string(),
                gate_lower: gates[i % gates.len()].to_string(),
                session_start_lower: "any".to_string(),
                gate_phrases: vec![],
                trigger_hints: vec![format!("hint_{}", i)],
                name_tokens: name_token_sets[cat].iter().map(|s| (*s).to_string()).collect(),
                keyword_tokens: keyword_sets[cat].iter().map(|s| (*s).to_string()).collect(),
                alias_tokens: HashSet::new(),
                do_not_use_tokens: HashSet::new(),
                framework_alias_entrypoints: vec![],
                metadata_positive_triggers: vec![],
                host_platforms: vec![],
                record_kind: "normal".to_string(),
                primary_allowed: true,
                fallback_policy_mode: "normal".to_string(),
                skill_flags: vec![],
            }
        })
        .collect()
}

// ── benchmark functions ───────────────────────────────────────────────────────

fn bench_search_skills(c: &mut Criterion) {
    let records_20 = make_skill_records(20);
    let records_50 = make_skill_records(50);
    let records_100 = make_skill_records(100);

    let queries: &[(&str, &str)] = &[
        ("short", "search"),
        ("medium", "find code analysis tools"),
        ("long", "I need a skill that can search through the codebase and find relevant patterns for code review and analysis"),
    ];

    let mut group = c.benchmark_group("search_skills");
    for (label, query) in queries {
        group.bench_with_input(BenchmarkId::new("20_records", label), query, |b, q| {
            b.iter(|| black_box(search_skills(black_box(&records_20), q, 10)));
        });
        group.bench_with_input(BenchmarkId::new("50_records", label), query, |b, q| {
            b.iter(|| black_box(search_skills(black_box(&records_50), q, 10)));
        });
        group.bench_with_input(BenchmarkId::new("100_records", label), query, |b, q| {
            b.iter(|| black_box(search_skills(black_box(&records_100), q, 10)));
        });
    }
    group.finish();

    // p50/p95 manual timing for 100-record, medium-query case
    let mut samples = Vec::new();
    let query = "find code analysis tools";
    for _ in 0..200 {
        let start = Instant::now();
        let _ = search_skills(&records_100, query, 10);
        samples.push(start.elapsed());
    }
    report_latency("search_skills/100_records/medium", &mut samples);
}

fn main() {
    let mut criterion = criterion::Criterion::default();
    bench_search_skills(&mut criterion);
    criterion.final_summary();
}
