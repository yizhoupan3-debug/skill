//! Tool routing benchmarks: search_tools and route_tool_from_records.
//!
//! ```bash
//! cargo bench -p tool-routing-engine --bench routing_bench
//! cargo bench -p tool-routing-engine --bench routing_bench -- --sample-size 10
//! ```
//!
//! No env-var gate — runs unconditionally as a CI performance gate.
//! Generates synthetic McpToolRecord data in-memory.

use criterion::{BenchmarkId, Criterion, black_box};
use mcp_tool_registry::{DispatchDomain, McpToolRecord, ToolLayer, ToolOwner};
use std::time::{Duration, Instant};
use tool_routing_engine::{route_tool_from_records, search_tools};

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
        "[routing_bench] {label}: p50={}µs p95={}µs (n={})",
        p50.as_micros(),
        p95.as_micros(),
        samples.len()
    );
}

fn make_tool_records(count: usize) -> Vec<McpToolRecord> {
    const DD: [DispatchDomain; 5] = [
        DispatchDomain::DomainFramework,
        DispatchDomain::Research,
        DispatchDomain::Browser,
        DispatchDomain::CodeGraph,
        DispatchDomain::StdioBinary,
    ];
    let descriptions = [
        "Search and retrieve documents from the knowledge base using full-text search",
        "Analyze code patterns and detect potential bugs via static analysis",
        "Take screenshots of web pages and extract visual information from the browser",
        "Query the codegraph database for symbol definitions and call relationships",
        "Execute shell commands and capture stdout/stderr output streams",
    ];
    let hints: &[&[&str]] = &[
        &["search", "find", "retrieve", "lookup", "query"],
        &["analyze", "audit", "inspect", "check", "review"],
        &["screenshot", "browser", "capture", "snapshot", "view"],
        &["codegraph", "symbol", "call", "dependency", "import"],
        &["shell", "execute", "run", "command", "terminal"],
    ];

    (0..count)
        .map(|i| {
            let cat = i % 5;
            McpToolRecord {
                slug: format!(
                    "tool_{}_{}",
                    ["search", "analyze", "screenshot", "codegraph", "exec"][cat],
                    i
                ),
                display_name: format!(
                    "{} Tool #{}",
                    ["Search", "Analyze", "Screenshot", "CodeGraph", "Execute"][cat],
                    i
                ),
                description: descriptions[cat].to_string(),
                layer: ToolLayer::Builtin,
                dispatch_domain: DD[cat].clone(),
                owner: ToolOwner::Framework,
                trigger_hints: hints[cat].iter().map(|s| s.to_string()).collect(),
                mcp_server: format!("server-{}", DD[cat]),
                tool_flags: vec![],
                input_schema_json: None,
            // Precomputed routing tokens (empty for tests — populated at load time)
            slug_lower: String::new(),
            display_name_lower: String::new(),
            name_tokens: std::collections::HashSet::new(),
            keyword_tokens: std::collections::HashSet::new(),
            desc_tokens: std::collections::HashSet::new(),
            alias_tokens: std::collections::HashSet::new(),
}

        })
        .collect()
}

// ── search_tools ──────────────────────────────────────────────────────────────

fn bench_search_tools(c: &mut Criterion) {
    let records_20 = make_tool_records(20);
    let records_50 = make_tool_records(50);
    let records_100 = make_tool_records(100);

    let queries: &[(&str, &str)] = &[
        ("short", "search"),
        ("medium", "find browser screenshot"),
        (
            "long",
            "I need a tool that can search the codebase for symbol definitions and display them",
        ),
    ];

    let mut group = c.benchmark_group("search_tools");
    for (label, query) in queries {
        group.bench_with_input(BenchmarkId::new("20_tools", label), query, |b, q| {
            b.iter(|| black_box(search_tools(black_box(q), black_box(&records_20), 10)));
        });
        group.bench_with_input(BenchmarkId::new("50_tools", label), query, |b, q| {
            b.iter(|| black_box(search_tools(black_box(q), black_box(&records_50), 10)));
        });
        group.bench_with_input(BenchmarkId::new("100_tools", label), query, |b, q| {
            b.iter(|| {
                black_box(search_tools(
                    black_box(q),
                    black_box(&records_100),
                    10,
                ))
            });
        });
    }
    group.finish();
}

// ── route_tool_from_records ───────────────────────────────────────────────────

fn bench_route_tool(c: &mut Criterion) {
    let records_20 = make_tool_records(20);
    let records_50 = make_tool_records(50);
    let records_100 = make_tool_records(100);

    let queries: &[(&str, &str)] = &[
        ("short", "search"),
        ("medium", "capture browser screenshot"),
        (
            "long",
            "I need to use the codegraph tool to find the callers of a function in the codebase",
        ),
    ];

    let mut group = c.benchmark_group("route_tool");
    for (label, query) in queries {
        group.bench_with_input(BenchmarkId::new("20_tools", label), query, |b, q| {
            b.iter(|| {
                black_box(route_tool_from_records(
                    black_box(q),
                    black_box(&records_20),
                ))
            });
        });
        group.bench_with_input(BenchmarkId::new("50_tools", label), query, |b, q| {
            b.iter(|| {
                black_box(route_tool_from_records(
                    black_box(q),
                    black_box(&records_50),
                ))
            });
        });
        group.bench_with_input(BenchmarkId::new("100_tools", label), query, |b, q| {
            b.iter(|| {
                black_box(route_tool_from_records(
                    black_box(q),
                    black_box(&records_100),
                ))
            });
        });
    }
    group.finish();

    // p50/p95 manual timing
    let records = make_tool_records(50);
    let mut samples = Vec::new();
    for _ in 0..200 {
        let start = Instant::now();
        let _ = route_tool_from_records("capture browser screenshot", &records);
        samples.push(start.elapsed());
    }
    report_latency("route_tool/50_tools/medium", &mut samples);
}

fn main() {
    let mut criterion = criterion::Criterion::default();
    bench_search_tools(&mut criterion);
    bench_route_tool(&mut criterion);
    criterion.final_summary();
}
