//! Skill search / record-load benchmarks (opt-in).
//!
//! ```bash
//! SEARCH_BENCH=1 cargo bench -p router-rs --bench search_bench
//! # quick smoke:
//! SEARCH_BENCH=1 cargo bench -p router-rs --bench search_bench -- --sample-size 10
//! ```
//!
//! Without `SEARCH_BENCH=1` the binary exits immediately (CI-friendly no-op).

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group};
use router_rs::route::{
    SkillRecord, filter_record_indices_for_host, invalidate_records_cache, load_records,
    load_records_cached_for_stdio, load_records_from_runtime, search_skills, search_skills_subset,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical repo root")
}

fn runtime_path(root: &Path) -> PathBuf {
    root.join("skills/SKILL_ROUTING_RUNTIME.json")
}

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
        "[search_bench] {label}: p50={}µs p95={}µs (n={})",
        p50.as_micros(),
        p95.as_micros(),
        samples.len()
    );
}

fn inflate_records(base: &[SkillRecord], target: usize) -> Vec<SkillRecord> {
    if base.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(target);
    while out.len() < target {
        for record in base {
            if out.len() >= target {
                break;
            }
            let mut clone = record.clone();
            clone.slug = format!("{}-{}", record.slug, out.len());
            clone.slug_lower = clone.slug.to_ascii_lowercase();
            out.push(clone);
        }
    }
    out
}

fn bench_record_load(c: &mut Criterion) {
    let root = repo_root();
    let runtime = runtime_path(&root);
    let _ = invalidate_records_cache();

    let mut group = c.benchmark_group("record_load");
    group.throughput(Throughput::Elements(1));
    group.bench_function("cold_load_records", |b| {
        b.iter(|| {
            let _ = invalidate_records_cache();
            black_box(load_records(Some(&runtime)).expect("load records"));
        });
    });
    group.bench_function("warm_load_records_cached", |b| {
        let _ =
            load_records_cached_for_stdio(Some(&runtime)).expect("prime cache");
        b.iter(|| {
            black_box(
                load_records_cached_for_stdio(Some(&runtime))
                    .expect("cached load")
                    .len(),
            );
        });
    });
    group.finish();

    let mut cold = Vec::new();
    let mut warm = Vec::new();
    for _ in 0..200 {
        let _ = invalidate_records_cache();
        let start = Instant::now();
        let _ = load_records(Some(&runtime)).expect("cold");
        cold.push(start.elapsed());
        let start = Instant::now();
        let _ = load_records_cached_for_stdio(Some(&runtime)).expect("warm");
        warm.push(start.elapsed());
    }
    report_latency("record_load/cold_load_records", &mut cold);
    report_latency("record_load/warm_load_records_cached", &mut warm);
}

fn bench_search_core(c: &mut Criterion) {
    let root = repo_root();
    let runtime = runtime_path(&root);
    let records = load_records_from_runtime(&runtime).expect("runtime records");
    let queries = [
        ("short", "pdf"),
        ("medium", "DESIGN.md 设计规范 token"),
        (
            "long",
            "需要多 agent 执行，先判断是否应该拆 bounded subagent sidecar workflow orchestration",
        ),
    ];

    let mut group = c.benchmark_group("search_skills");
    for (label, query) in queries {
        group.bench_with_input(
            BenchmarkId::new("runtime_manifest", label),
            &query,
            |b, query| {
                b.iter(|| black_box(search_skills(&records, query, 10)));
            },
        );
    }
    group.finish();

    let inflated = inflate_records(&records, 96);
    let mut group = c.benchmark_group("search_skills_parallel");
    group.bench_function("96_records_medium_query", |b| {
        let query = "DESIGN.md 设计规范 token";
        b.iter(|| black_box(search_skills(&inflated, query, 10)));
    });
    group.finish();

    let mut serial = Vec::new();
    let mut parallel = Vec::new();
    let query = "DESIGN.md 设计规范 token";
    for _ in 0..100 {
        let start = Instant::now();
        let _ = search_skills(&records, query, 10);
        serial.push(start.elapsed());
        let start = Instant::now();
        let _ = search_skills(&inflated, query, 10);
        parallel.push(start.elapsed());
    }
    report_latency("search_skills/runtime_manifest/medium", &mut serial);
    report_latency(
        "search_skills_parallel/96_records_medium_query",
        &mut parallel,
    );
}

fn bench_host_filter_path(c: &mut Criterion) {
    let root = repo_root();
    let runtime = runtime_path(&root);
    let records =
        load_records_cached_for_stdio(Some(&runtime)).expect("cached records");
    let query = "plugin creator";

    let mut group = c.benchmark_group("mcp_search_path");
    group.bench_function("warm_cached_indices_search", |b| {
        b.iter(|| {
            let indices =
                filter_record_indices_for_host(records.as_ref(), Some("cursor")).expect("indices");
            black_box(search_skills_subset(
                records.as_ref(),
                Some(&indices),
                query,
                20,
            ));
        });
    });
    group.finish();

    let mut samples = Vec::new();
    for _ in 0..200 {
        let start = Instant::now();
        let indices =
            filter_record_indices_for_host(records.as_ref(), Some("cursor")).expect("indices");
        let _ = search_skills_subset(records.as_ref(), Some(&indices), query, 20);
        samples.push(start.elapsed());
    }
    report_latency("mcp_search_path/warm_cached_indices_search", &mut samples);
}

criterion_group!(
    benches,
    bench_record_load,
    bench_search_core,
    bench_host_filter_path
);

fn main() {
    if std::env::var("SEARCH_BENCH").ok().as_deref() != Some("1") {
        eprintln!("skip search_bench (set SEARCH_BENCH=1 to run)");
        return;
    }
    let mut criterion = criterion::Criterion::default();
    bench_record_load(&mut criterion);
    bench_search_core(&mut criterion);
    bench_host_filter_path(&mut criterion);
    criterion.final_summary();
}
