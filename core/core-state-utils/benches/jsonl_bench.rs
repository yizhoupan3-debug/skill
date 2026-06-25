//! JSONL maintenance benchmarks: compaction and corrupt-tail truncation.
//!
//! ```bash
//! cargo bench -p core-state-utils --bench jsonl_bench
//! cargo bench -p core-state-utils --bench jsonl_bench -- --sample-size 10
//! ```
//!
//! No env-var gate — runs unconditionally as a CI performance gate.
//! All input data is generated in-memory.

use criterion::{Criterion, black_box, criterion_group};
use core_state_utils::jsonl_maintenance::compact_jsonl_with_content;
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
        "[jsonl_bench] {label}: p50={}µs p95={}µs (n={})",
        p50.as_micros(),
        p95.as_micros(),
        samples.len()
    );
}

/// Generate JSONL content with `num_steps` step-type lines and `num_snapshot` state-snapshot lines.
/// The last state-snapshot of each type is the "current" one; earlier ones are dedup candidates.
fn make_jsonl_content(num_steps: usize, num_snapshots: usize, last_seq: usize) -> String {
    use serde_json::json;
    let mut buf = String::new();
    for i in 0..num_steps {
        let line = json!({"tx_type": "step", "seq": i, "payload": {"i": i}});
        buf.push_str(&line.to_string());
        buf.push('\n');
    }
    for i in 0..num_snapshots {
        let line = json!({
            "tx_type": "goal_state",
            "seq": last_seq + i,
            "version": i,
            "task_id": "test-task",
            "status": "active"
        });
        buf.push_str(&line.to_string());
        buf.push('\n');
    }
    buf
}

/// Generate JSONL content with ONLY non-snapshot rows (no dedup opportunity).
fn make_no_snapshot_content(extra: usize) -> String {
    use serde_json::json;
    let mut buf = String::new();
    for i in 0..(100 + extra) {
        let line = json!({"tx_type": "step", "seq": i, "i": i});
        buf.push_str(&line.to_string());
        buf.push('\n');
    }
    buf
}

// ── compact_jsonl_with_content ────────────────────────────────────────────────

fn bench_compact_jsonl(c: &mut Criterion) {
    // Under threshold: 50 lines, max_lines=100 → fast path, no compaction
    let under_threshold = make_jsonl_content(50, 0, 50);

    // Over threshold, no snapshots: 150 step lines, max_lines=100 → scan but no dedup
    let over_no_snap = make_no_snapshot_content(50);

    // Over threshold, with snapshots: 95 steps + 10 goal_state = 105 lines, max_lines=100 → compaction
    let over_with_snap = make_jsonl_content(95, 10, 95);

    // Large content: 500 steps + 50 snapshots = 550 lines, max_lines=100 → heavy compaction
    let large = make_jsonl_content(500, 50, 500);

    let tmp_dir = std::env::temp_dir().join("jsonl-bench-tmp");
    let tmp_path = tmp_dir.join("bench.jsonl");

    let mut group = c.benchmark_group("compact_jsonl");
    group.bench_function("under_threshold_50of100", |b| {
        b.iter(|| {
            let _ = black_box(compact_jsonl_with_content(
                black_box(&tmp_path),
                black_box(&under_threshold),
                100,
            ));
        });
    });
    group.bench_function("over_threshold_no_snapshots", |b| {
        b.iter(|| {
            let _ = black_box(compact_jsonl_with_content(
                black_box(&tmp_path),
                black_box(&over_no_snap),
                100,
            ));
        });
    });
    group.bench_function("over_threshold_with_dedup_105of100", |b| {
        b.iter(|| {
            let _ = black_box(compact_jsonl_with_content(
                black_box(&tmp_path),
                black_box(&over_with_snap),
                100,
            ));
        });
    });
    group.bench_function("large_550of100", |b| {
        b.iter(|| {
            let _ = black_box(compact_jsonl_with_content(
                black_box(&tmp_path),
                black_box(&large),
                100,
            ));
        });
    });
    group.finish();

    // p50/p95 for the dedup case
    let mut samples = Vec::new();
    for _ in 0..200 {
        let start = Instant::now();
        let _ = compact_jsonl_with_content(&tmp_path, &over_with_snap, 100);
        samples.push(start.elapsed());
    }
    let _ = std::fs::remove_dir_all(&tmp_dir);
    report_latency("compact_jsonl/over_threshold_with_dedup_105of100", &mut samples);
}

criterion_group!(jsonl_benches, bench_compact_jsonl);

fn main() {
    let mut criterion = criterion::Criterion::default();
    bench_compact_jsonl(&mut criterion);
    criterion.final_summary();
}
