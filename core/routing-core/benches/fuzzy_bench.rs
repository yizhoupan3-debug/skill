//! Fuzzy matching micro-benchmarks (routing-core trigram Jaccard).
//!
//! ```bash
//! cargo bench -p routing-core --bench fuzzy_bench
//! cargo bench -p routing-core --bench fuzzy_bench -- --sample-size 10  # quick smoke
//! ```
//!
//! No env-var gate — runs unconditionally as a CI performance gate.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use routing_core::fuzzy::{
    best_fuzzy_jaccard, extract_trigrams, jaccard_similarity, trigram_similarity,
};
use std::collections::HashSet;

// ── extract_trigrams ──────────────────────────────────────────────────────────

fn bench_extract_trigrams(c: &mut Criterion) {
    let short = "hello";
    let medium = "the quick brown fox jumps over the lazy dog";
    let long = "这是一个比较长的中文文本，包含了各种路由关键词，比如 review、audit、plan、execute、validate，用于测试三元组分词性能";
    let cjk = "代码审查";

    let mut group = c.benchmark_group("extract_trigrams");
    group.bench_function("short_ascii", |b| {
        b.iter(|| black_box(extract_trigrams(black_box(short))));
    });
    group.bench_function("medium_ascii", |b| {
        b.iter(|| black_box(extract_trigrams(black_box(medium))));
    });
    group.bench_function("long_mixed", |b| {
        b.iter(|| black_box(extract_trigrams(black_box(long))));
    });
    group.bench_function("short_cjk", |b| {
        b.iter(|| black_box(extract_trigrams(black_box(cjk))));
    });
    group.finish();
}

// ── jaccard_similarity ────────────────────────────────────────────────────────

fn bench_jaccard(c: &mut Criterion) {
    let small_a: HashSet<String> = ["hel", "ell", "llo"]
        .into_iter()
        .map(String::from)
        .collect();
    let small_b: HashSet<String> = ["hel", "ell", "xyz"]
        .into_iter()
        .map(String::from)
        .collect();

    let med_a: HashSet<String> = (0..50).map(|i| format!("tri{:03}", i)).collect();
    let med_b: HashSet<String> = (25..75).map(|i| format!("tri{:03}", i)).collect();

    let large_a: HashSet<String> = (0..500).map(|i| format!("tri{:03}", i)).collect();
    let large_b: HashSet<String> = (200..700).map(|i| format!("tri{:03}", i)).collect();

    let empty: HashSet<String> = HashSet::new();

    let mut group = c.benchmark_group("jaccard_similarity");
    group.bench_function("small_sets", |b| {
        b.iter(|| black_box(jaccard_similarity(black_box(&small_a), black_box(&small_b))));
    });
    group.bench_function("medium_sets_50_overlap", |b| {
        b.iter(|| black_box(jaccard_similarity(black_box(&med_a), black_box(&med_b))));
    });
    group.bench_function("large_sets_500", |b| {
        b.iter(|| black_box(jaccard_similarity(black_box(&large_a), black_box(&large_b))));
    });
    group.bench_function("empty_sets", |b| {
        b.iter(|| black_box(jaccard_similarity(black_box(&empty), black_box(&empty))));
    });
    group.finish();
}

// ── best_fuzzy_jaccard ────────────────────────────────────────────────────────

fn bench_best_fuzzy(c: &mut Criterion) {
    let few_candidates: Vec<String> = (0..10).map(|i| format!("skill-{}-match", i)).collect();
    let many_candidates: Vec<String> = (0..200)
        .map(|i| format!("tool-{}-handler-{}", i, i % 5))
        .collect();
    let query = "skill-3-match";

    let mut group = c.benchmark_group("best_fuzzy_jaccard");
    group.bench_function("10_candidates", |b| {
        b.iter(|| {
            black_box(best_fuzzy_jaccard(
                black_box(query),
                black_box(&few_candidates),
            ))
        });
    });
    group.bench_function("200_candidates", |b| {
        b.iter(|| {
            black_box(best_fuzzy_jaccard(
                black_box(query),
                black_box(&many_candidates),
            ))
        });
    });
    group.bench_function("empty_candidates", |b| {
        b.iter(|| {
            black_box(best_fuzzy_jaccard(
                black_box("anything"),
                black_box(&[] as &[String]),
            ))
        });
    });
    group.finish();
}

// ── trigram_similarity (convenience wrapper) ───────────────────────────────────

fn bench_trigram_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("trigram_similarity");
    group.bench_function("identical_short", |b| {
        b.iter(|| black_box(trigram_similarity(black_box("hello"), black_box("hello"))));
    });
    group.bench_function("identical_long", |b| {
        let text = "the quick brown fox jumps over the lazy dog near the riverbank";
        b.iter(|| black_box(trigram_similarity(black_box(text), black_box(text))));
    });
    group.bench_function("no_overlap", |b| {
        b.iter(|| black_box(trigram_similarity(black_box("abcdef"), black_box("ghijkl"))));
    });
    group.bench_function("partial_match_cjk", |b| {
        b.iter(|| {
            black_box(trigram_similarity(
                black_box("代码审查工具"),
                black_box("代码生成工具"),
            ))
        });
    });
    group.finish();
}

criterion_group!(
    fuzzy_benches,
    bench_extract_trigrams,
    bench_jaccard,
    bench_best_fuzzy,
    bench_trigram_similarity
);
criterion_main!(fuzzy_benches);
