//! Stdio JSON serialization/deserialization benchmarks.
//!
//! ```bash
//! cargo bench -p fr-utils --bench stdio_serde_bench
//! cargo bench -p fr-utils --bench stdio_serde_bench -- --sample-size 10
//! ```
//!
//! No env-var gate — runs unconditionally as a CI performance gate.
//! Benchmarks deserialization of StdioJsonRequestPayload and serialization
//! of StdioJsonResponsePayload with realistic payload shapes.
//!
//! Note: StdioJsonRequestPayload derives only Deserialize (no Serialize);
//! StdioJsonResponsePayload derives only Serialize (no Deserialize).
//! Roundtrip is measured via serde_json::Value as intermediate.

use criterion::{Criterion, black_box};
use fr_utils::types::{StdioJsonRequestPayload, StdioJsonResponsePayload};
use serde_json::Value;

// ── raw JSON strings representative of real stdio traffic ─────────────────────

const ROUTE_REQUEST_JSON: &str = r#"{"id":1,"op":"route","payload":{"task":"find me a pdf summary tool","session_id":"sess_abc123","first_turn":true}}"#;

const SEARCH_REQUEST_JSON: &str = r#"{"id":2,"op":"search_skills","payload":{"query":"code review automation","top_k":10}}"#;

const EXECUTE_REQUEST_JSON: &str = r#"{"id":3,"op":"execute","payload":{"task_id":"task_001","skill":"code-review","args":{"mode":"deep","target":"src/main.rs"}}}"#;

const LARGE_REQUEST_JSON: &str = r#"{"id":100,"op":"eval_route","payload":{"task":"a long multi-turn routing query with many tokens that should trigger the full scoring pipeline across all 16 steps of the routing engine including NL adjustment and fuzzy fallback","session_id":"sess_large_001","first_turn":false,"context":{"previous_skills":["search","analyze"],"user_intent":"complex analysis workflow"}}}"#;

// ── pre-built response payloads (StdioJsonResponsePayload: Serialize only) ────

fn make_route_response() -> StdioJsonResponsePayload {
    StdioJsonResponsePayload {
        id: serde_json::json!(1),
        ok: true,
        payload: Some(serde_json::json!({
            "selected_skill": "pdf-summary",
            "score": 0.85,
            "layer": "runtime",
            "fuzzy_match": false
        })),
        error: None,
    }
}

fn make_error_response() -> StdioJsonResponsePayload {
    StdioJsonResponsePayload {
        id: serde_json::json!(99),
        ok: false,
        payload: None,
        error: Some("skill not found: unknown_skill".to_string()),
    }
}

fn make_large_response() -> StdioJsonResponsePayload {
    StdioJsonResponsePayload {
        id: serde_json::json!(100),
        ok: true,
        payload: Some(serde_json::json!({
            "selected_skill": "deep-review",
            "score": 0.92,
            "layer": "framework",
            "matched_token_count": 12,
            "reasons": [
                "name_tokens: code, review",
                "keyword_tokens: audit, inspect",
                "description_match: performs deep code review with static analysis"
            ],
            "route_snapshot": {
                "selected_skill": "deep-review",
                "score": 0.92,
                "layer": "framework",
                "fuzzy_match": false
            }
        })),
        error: None,
    }
}

// ── deserialization benchmarks ────────────────────────────────────────────────

fn bench_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("deserialize_request");

    group.bench_function("route_op", |b| {
        b.iter(|| {
            let _: StdioJsonRequestPayload =
                black_box(serde_json::from_str(black_box(ROUTE_REQUEST_JSON)).unwrap());
        });
    });

    group.bench_function("search_op", |b| {
        b.iter(|| {
            let _: StdioJsonRequestPayload =
                black_box(serde_json::from_str(black_box(SEARCH_REQUEST_JSON)).unwrap());
        });
    });

    group.bench_function("execute_op", |b| {
        b.iter(|| {
            let _: StdioJsonRequestPayload =
                black_box(serde_json::from_str(black_box(EXECUTE_REQUEST_JSON)).unwrap());
        });
    });

    group.bench_function("large_payload", |b| {
        b.iter(|| {
            let _: StdioJsonRequestPayload =
                black_box(serde_json::from_str(black_box(LARGE_REQUEST_JSON)).unwrap());
        });
    });

    group.finish();
}

// ── serialization benchmarks ──────────────────────────────────────────────────

fn bench_serialize(c: &mut Criterion) {
    let route_resp = make_route_response();
    let err_resp = make_error_response();
    let large_resp = make_large_response();

    let mut group = c.benchmark_group("serialize_response");

    group.bench_function("success_payload", |b| {
        b.iter(|| {
            let _ = black_box(serde_json::to_string(black_box(&route_resp)).unwrap());
        });
    });

    group.bench_function("error_payload", |b| {
        b.iter(|| {
            let _ = black_box(serde_json::to_string(black_box(&err_resp)).unwrap());
        });
    });

    group.bench_function("large_with_reasons", |b| {
        b.iter(|| {
            let _ = black_box(serde_json::to_string(black_box(&large_resp)).unwrap());
        });
    });

    group.finish();
}

// ── roundtrip benchmarks (via serde_json::Value) ──────────────────────────────

fn bench_value_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("value_roundtrip");

    group.bench_function("route_request", |b| {
        b.iter(|| {
            let v: Value = serde_json::from_str(black_box(ROUTE_REQUEST_JSON)).unwrap();
            let _ = serde_json::to_string(black_box(&v)).unwrap();
        });
    });

    group.bench_function("large_request", |b| {
        b.iter(|| {
            let v: Value = serde_json::from_str(black_box(LARGE_REQUEST_JSON)).unwrap();
            let _ = serde_json::to_string(black_box(&v)).unwrap();
        });
    });

    group.finish();
}

fn main() {
    let mut criterion = criterion::Criterion::default();
    bench_deserialize(&mut criterion);
    bench_serialize(&mut criterion);
    bench_value_roundtrip(&mut criterion);
    criterion.final_summary();
}
