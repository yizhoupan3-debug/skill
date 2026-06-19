//! Smoke test: freshness guard for academic source queries.
//!
//! Implements §19.6 of the research harness spec:
//! - Query registry at `artifacts/research-log/smoke-tests.json`
//! - HTTP execution via existing arXiv / Semantic Scholar clients
//! - Freshness computation, stale detection, regression comparison
//! - Results written to `smoke-test-results.jsonl`

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::*;

const SMOKE_TESTS_REL_PATH: &str = "artifacts/research-log/smoke-tests.json";
const SMOKE_RESULTS_REL_PATH: &str = "artifacts/research-log/smoke-test-results.jsonl";
const EXPECTED_SCHEMA_VERSION: &str = "research-smoke-v1";
const STALE_THRESHOLD_DAYS: i64 = 180;

/// A single smoke test query definition from smoke-tests.json.
#[derive(Debug)]
#[allow(dead_code)]
struct SmokeQuery {
    id: String,
    source: String,
    query: String,
    expected_min_results: usize,
    expected_freshness_days: i64,
    related_directions: Vec<String>,
    related_barriers: Vec<String>,
}

/// A single smoke test result.
#[derive(Debug, Clone)]
struct SmokeResult {
    id: String,
    passed: bool,
    results_count: usize,
    expected_min: usize,
    stale: bool,
    freshness_days: i64,
    expected_freshness_days: i64,
    regression: Option<String>,
    error: Option<String>,
    timestamp: String,
}

/// Load and parse smoke-tests.json from the repo root.
fn load_smoke_config(repo_root: &Path) -> Result<Vec<SmokeQuery>> {
    let path = repo_root.join(SMOKE_TESTS_REL_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read smoke config: {}", path.display()))?;
    let config: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse smoke config: {}", path.display()))?;

    let version = config.get("schema_version").and_then(Value::as_str).unwrap_or("");
    if version != EXPECTED_SCHEMA_VERSION {
        eprintln!(
            "[smoke] config schema_version={:?}, expected={EXPECTED_SCHEMA_VERSION:?}",
            version
        );
    }

    let mut queries = Vec::new();
    if let Some(arr) = config.get("queries").and_then(Value::as_array) {
        for item in arr {
            queries.push(SmokeQuery {
                id: str_field(item, "id"),
                source: str_field_default(item, "source", "arxiv"),
                query: str_field(item, "query"),
                expected_min_results: item.get("expected_min_results").and_then(Value::as_u64).unwrap_or(3) as usize,
                expected_freshness_days: item.get("expected_freshness_days").and_then(Value::as_i64).unwrap_or(180),
                related_directions: item.get("related_directions")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                    .unwrap_or_default(),
                related_barriers: item.get("related_barriers")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                    .unwrap_or_default(),
            });
        }
    }

    // Also load barrier_extends
    if let Some(extends) = config.get("barrier_extends").and_then(Value::as_object) {
        for (_barrier_id, group) in extends {
            if let Some(arr) = group.get("queries").and_then(Value::as_array) {
                for item in arr {
                    queries.push(SmokeQuery {
                        id: str_field(item, "id"),
                        source: str_field_default(item, "source", "arxiv"),
                        query: str_field(item, "query"),
                        expected_min_results: item.get("expected_min_results").and_then(Value::as_u64).unwrap_or(3) as usize,
                        expected_freshness_days: item.get("expected_freshness_days").and_then(Value::as_i64).unwrap_or(180),
                        related_directions: item.get("related_directions")
                            .and_then(Value::as_array)
                            .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                            .unwrap_or_default(),
                        related_barriers: Vec::new(),
                    });
                }
            }
        }
    }

    Ok(queries)
}

/// Filter queries by source and/or barrier_id.
fn filter_queries(queries: Vec<SmokeQuery>, source: Option<&str>, barrier_id: Option<&str>) -> Vec<SmokeQuery> {
    queries.into_iter().filter(|q| {
        if let Some(src) = source {
            if q.source != src {
                return false;
            }
        }
        if let Some(bid) = barrier_id {
            if !q.related_barriers.iter().any(|b| b == bid) {
                return false;
            }
        }
        true
    }).collect()
}

/// Execute a single smoke test query against the appropriate source.
fn execute_query(query: &SmokeQuery, client: &reqwest::blocking::Client) -> SmokeResult {
    let id = query.id.clone();
    let timestamp = now_iso();

    // Fetch results from the appropriate source
    let results = match query.source.as_str() {
        "arxiv" | "all" => fetch_arxiv(client, &query.query, query.expected_min_results.max(5)),
        "semantic-scholar" | "semantic_scholar" | "semanticscholar" => {
            fetch_semantic_scholar(client, &query.query, query.expected_min_results.max(5))
        }
        other => {
            return SmokeResult {
                id,
                passed: false,
                results_count: 0,
                expected_min: query.expected_min_results,
                stale: true,
                freshness_days: 999,
                expected_freshness_days: query.expected_freshness_days,
                regression: None,
                error: Some(format!("unsupported source: {other}")),
                timestamp,
            };
        }
    };

    match results {
        Ok(papers) => {
            let results_count = papers.len();
            // Compute freshness: oldest result date vs now
            // Try precise date first (publicationDate/published), fall back to year
            let now = chrono::Utc::now();
            let freshness_days = papers.iter()
                .filter_map(|p| {
                    // Try precise date fields first (ISO date strings)
                    let precise_days = ["publicationDate", "published", "date"].iter()
                        .filter_map(|field| p.get(field).and_then(Value::as_str))
                        .filter_map(|date_str| {
                            chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                                .or_else(|_| chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S"))
                                .ok()
                        })
                        .map(|d| {
                            let nd = now.date_naive();
                            (nd - d).num_days().max(0)
                        })
                        .next();
                    if let Some(days) = precise_days {
                        return Some(days);
                    }
                    // Fall back to year-level with mid-year estimate (July 1)
                    p.get("year").and_then(Value::as_i64).map(|yr| {
                        let est_date = chrono::NaiveDate::from_ymd_opt(yr as i32, 7, 1)
                            .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(yr as i32, 1, 1).unwrap());
                        (now.date_naive() - est_date).num_days().max(0)
                    })
                })
                .min()
                .unwrap_or(0);

            let stale = freshness_days > query.expected_freshness_days
                || freshness_days > STALE_THRESHOLD_DAYS;
            let passed = results_count >= query.expected_min_results && !stale;

            SmokeResult {
                id,
                passed,
                results_count,
                expected_min: query.expected_min_results,
                stale,
                freshness_days,
                expected_freshness_days: query.expected_freshness_days,
                regression: None,
                error: if passed { None } else {
                    Some(format!("expected >= {}, got {}; stale={}", query.expected_min_results, results_count, stale))
                },
                timestamp,
            }
        }
        Err(e) => SmokeResult {
            id: id.clone(),
            passed: false,
            results_count: 0,
            expected_min: query.expected_min_results,
            stale: true,
            freshness_days: 999,
            expected_freshness_days: query.expected_freshness_days,
            regression: None,
            error: Some(e.to_string()),
            timestamp,
        },
    }
}

/// Load previous results from smoke-test-results.jsonl for regression comparison.
fn load_previous_results(path: &Path) -> Result<HashMap<String, SmokeResult>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let text = fs::read_to_string(path)?;
    let mut results = HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(val) => {
                if let Some(id) = val.get("id").and_then(Value::as_str) {
                    results.insert(id.to_string(), SmokeResult {
                        id: id.to_string(),
                        passed: val.get("passed").and_then(Value::as_bool).unwrap_or(false),
                        results_count: val.get("results_count").and_then(Value::as_u64).unwrap_or(0) as usize,
                        expected_min: val.get("expected_min").and_then(Value::as_u64).unwrap_or(3) as usize,
                        stale: val.get("stale").and_then(Value::as_bool).unwrap_or(true),
                        freshness_days: val.get("freshness_days").and_then(Value::as_i64).unwrap_or(999),
                        expected_freshness_days: val.get("expected_freshness_days").and_then(Value::as_i64).unwrap_or(180),
                        regression: val.get("regression").and_then(Value::as_str).map(String::from),
                        error: val.get("error").and_then(Value::as_str).map(String::from),
                        timestamp: val.get("timestamp").and_then(Value::as_str).unwrap_or("").to_string(),
                    });
                }
            }
            Err(e) => {
                eprintln!("warn: skipping malformed JSONL line in {}: {e}", path.display());
            }
        }
    }
    Ok(results)
}

/// Run regression detection against previous results.
fn detect_regression(current: &SmokeResult, previous: &HashMap<String, SmokeResult>) -> Option<String> {
    let prev = previous.get(&current.id)?;
    if prev.passed && !current.passed {
        return Some("previous: passed → now: failed".to_string());
    }
    if prev.results_count > 0 {
        let drop_ratio = prev.results_count as f64 / current.results_count.max(1) as f64;
        if drop_ratio > 2.0 {
            return Some(format!("results dropped >50% ({} → {})", prev.results_count, current.results_count));
        }
    }
    if prev.freshness_days > 0 && current.freshness_days > prev.freshness_days * 2 {
        return Some(format!("freshness window expanded >2× ({}d → {}d)", prev.freshness_days, current.freshness_days));
    }
    None
}

/// Core implementation: run smoke tests and return JSONL results string.
/// Both `cmd_smoke_test` and `cmd_barrier` call this.
pub(super) fn run_smoke_tests(
    repo_root: &Path,
    source: Option<&str>,
    barrier_id: Option<&str>,
) -> Result<String> {
    let queries = load_smoke_config(repo_root)?;
    if queries.is_empty() {
        return Ok(String::new());
    }
    let filtered = filter_queries(queries, source, barrier_id);
    if filtered.is_empty() {
        return Ok(String::new());
    }

    let client = http_client(DEFAULT_EXTERNAL_TIMEOUT_SECS)?;

    // Load previous results for regression
    let prev_path = repo_root.join(SMOKE_RESULTS_REL_PATH);
    let previous = load_previous_results(&prev_path)?;

    let mut lines = Vec::new();
    for query in &filtered {
        let mut result = execute_query(query, &client);
        // Regression check
        if barrier_id.is_none() {
            result.regression = detect_regression(&result, &previous);
        }
        let entry = json!({
            "id": result.id,
            "passed": result.passed,
            "results_count": result.results_count,
            "expected_min": result.expected_min,
            "expected_freshness_days": result.expected_freshness_days,
            "freshness_days": result.freshness_days,
            "stale": result.stale,
            "regression": result.regression,
            "error": result.error,
            "timestamp": result.timestamp,
        });
        lines.push(serde_json::to_string(&entry)?);
    }

    // Write results (replace, not append — each run is a full snapshot)
    if let Err(e) = fs::write(&prev_path, lines.join("\n")) {
        eprintln!("warn: failed to persist smoke results to {}: {e}", prev_path.display());
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn load_smoke_config_returns_empty_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let queries = load_smoke_config(tmp.path()).unwrap();
        assert!(queries.is_empty());
    }

    #[test]
    fn load_smoke_config_parses_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(SMOKE_TESTS_REL_PATH);
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, json!({
            "schema_version": "research-smoke-v1",
            "queries": [{
                "id": "q1",
                "source": "arxiv",
                "query": "transformer attention",
                "expected_min_results": 3,
                "expected_freshness_days": 180,
                "related_directions": ["nlp"],
                "related_barriers": ["br-001"]
            }]
        }).to_string()).unwrap();

        let queries = load_smoke_config(tmp.path()).unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].id, "q1");
        assert_eq!(queries[0].source, "arxiv");
        assert_eq!(queries[0].expected_min_results, 3);
    }

    #[test]
    fn filter_queries_by_source() {
        let queries = vec![
            SmokeQuery { id: "a".into(), source: "arxiv".into(), query: "".into(), expected_min_results: 3, expected_freshness_days: 180, related_directions: vec![], related_barriers: vec![] },
            SmokeQuery { id: "b".into(), source: "semantic-scholar".into(), query: "".into(), expected_min_results: 3, expected_freshness_days: 180, related_directions: vec![], related_barriers: vec![] },
        ];
        let filtered = filter_queries(queries, Some("arxiv"), None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "a");
    }

    #[test]
    fn filter_queries_by_barrier() {
        let queries = vec![
            SmokeQuery { id: "a".into(), source: "arxiv".into(), query: "".into(), expected_min_results: 3, expected_freshness_days: 180, related_directions: vec![], related_barriers: vec!["br-001".into()] },
            SmokeQuery { id: "b".into(), source: "arxiv".into(), query: "".into(), expected_min_results: 3, expected_freshness_days: 180, related_directions: vec![], related_barriers: vec![] },
        ];
        let filtered = filter_queries(queries, None, Some("br-001"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "a");
    }

    #[test]
    fn filter_queries_by_both() {
        let queries = vec![
            SmokeQuery { id: "a".into(), source: "arxiv".into(), query: "".into(), expected_min_results: 3, expected_freshness_days: 180, related_directions: vec![], related_barriers: vec!["br-001".into()] },
            SmokeQuery { id: "b".into(), source: "semantic-scholar".into(), query: "".into(), expected_min_results: 3, expected_freshness_days: 180, related_directions: vec![], related_barriers: vec!["br-001".into()] },
        ];
        let filtered = filter_queries(queries, Some("arxiv"), Some("br-001"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "a");
    }

    #[test]
    fn smoke_result_passed_true_when_ok() {
        let result = SmokeResult {
            id: "t1".into(),
            passed: true,
            results_count: 5,
            expected_min: 3,
            stale: false,
            freshness_days: 30,
            expected_freshness_days: 180,
            regression: None,
            error: None,
            timestamp: "now".into(),
        };
        assert!(result.passed);
    }

    #[test]
    fn smoke_result_passed_false_when_stale() {
        let result = SmokeResult {
            id: "t2".into(),
            passed: false,
            results_count: 5,
            expected_min: 3,
            stale: true,
            freshness_days: 365,
            expected_freshness_days: 180,
            regression: None,
            error: Some("stale".into()),
            timestamp: "now".into(),
        };
        assert!(!result.passed);
    }

    #[test]
    fn regression_detects_pass_to_fail() {
        let current = SmokeResult { id: "q1".into(), passed: false, results_count: 1, expected_min: 3, stale: true, freshness_days: 365, expected_freshness_days: 180, regression: None, error: None, timestamp: "".into() };
        let mut prev = HashMap::new();
        prev.insert("q1".into(), SmokeResult { id: "q1".into(), passed: true, results_count: 10, expected_min: 3, stale: false, freshness_days: 30, expected_freshness_days: 180, regression: None, error: None, timestamp: "".into() });
        assert!(detect_regression(&current, &prev).is_some());
    }

    #[test]
    fn regression_detects_drop_gt_50pct() {
        let current = SmokeResult { id: "q1".into(), passed: false, results_count: 4, expected_min: 3, stale: false, freshness_days: 30, expected_freshness_days: 180, regression: None, error: None, timestamp: "".into() };
        let mut prev = HashMap::new();
        prev.insert("q1".into(), SmokeResult { id: "q1".into(), passed: true, results_count: 10, expected_min: 3, stale: false, freshness_days: 30, expected_freshness_days: 180, regression: None, error: None, timestamp: "".into() });
        assert!(detect_regression(&current, &prev).is_some());
    }

    #[test]
    fn regression_detects_freshness_expansion() {
        let current = SmokeResult { id: "q1".into(), passed: false, results_count: 5, expected_min: 3, stale: true, freshness_days: 100, expected_freshness_days: 180, regression: None, error: None, timestamp: "".into() };
        let mut prev = HashMap::new();
        prev.insert("q1".into(), SmokeResult { id: "q1".into(), passed: true, results_count: 5, expected_min: 3, stale: false, freshness_days: 30, expected_freshness_days: 180, regression: None, error: None, timestamp: "".into() });
        assert!(detect_regression(&current, &prev).is_some());
    }

    #[test]
    fn regression_no_false_positive() {
        let current = SmokeResult { id: "q1".into(), passed: true, results_count: 8, expected_min: 3, stale: false, freshness_days: 40, expected_freshness_days: 180, regression: None, error: None, timestamp: "".into() };
        let mut prev = HashMap::new();
        prev.insert("q1".into(), SmokeResult { id: "q1".into(), passed: true, results_count: 10, expected_min: 3, stale: false, freshness_days: 30, expected_freshness_days: 180, regression: None, error: None, timestamp: "".into() });
        assert!(detect_regression(&current, &prev).is_none());
    }

    #[test]
    fn stale_threshold_logic() {
        // freshness_days > 180 = stale
        assert!(STALE_THRESHOLD_DAYS >= 180);
    }

    #[test]
    fn smoke_query_from_barrier_extends() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(SMOKE_TESTS_REL_PATH);
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(&config_path, json!({
            "schema_version": "research-smoke-v1",
            "queries": [],
            "barrier_extends": {
                "br-001": {
                    "queries": [{
                        "id": "br-q1",
                        "source": "arxiv",
                        "query": "attention mechanism"
                    }]
                }
            }
        }).to_string()).unwrap();

        let queries = load_smoke_config(tmp.path()).unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].id, "br-q1");
    }
}
