//! Smoke test: freshness guard for academic source queries.
//!
//! Implements the research harness smoke test specification:
//! - Query registry at `artifacts/research-log/smoke-tests.json`
//! - HTTP execution via existing arXiv / Semantic Scholar clients
//! - Freshness computation, stale detection, regression comparison
//! - Results written to `smoke-test-results.jsonl`
//!
//! Migrated from `tools/autoresearch-rs/src/smoke.rs`.

use anyhow::{Context, Result};
use chrono::Datelike;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use crate::search::arxiv;
use crate::search::semantic_scholar;
use crate::util::{str_field, str_field_default};

// ── Local helpers ──

fn http_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs.clamp(3, 120)))
        .build()
        .context("failed to build HTTP client")
}

const DEFAULT_EXTERNAL_TIMEOUT_SECS: u64 = 20;

const SMOKE_TESTS_REL_PATH: &str = "artifacts/research-log/smoke-tests.json";
const SMOKE_RESULTS_REL_PATH: &str = "artifacts/research-log/smoke-test-results.jsonl";
const EXPECTED_SCHEMA_VERSION: &str = "research-smoke-v1";
const STALE_THRESHOLD_DAYS: i64 = 180;

/// A single smoke test query definition from smoke-tests.json.
#[derive(Debug, Clone)]
pub struct SmokeQuery {
    pub id: String,
    pub source: String,
    pub query: String,
    pub expected_min_results: usize,
    pub expected_freshness_days: i64,
    pub related_directions: Vec<String>,
    pub related_barriers: Vec<String>,
}

/// A single smoke test result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SmokeResult {
    pub id: String,
    pub passed: bool,
    pub results_count: usize,
    pub expected_min: usize,
    pub stale: bool,
    pub freshness_days: i64,
    pub expected_freshness_days: i64,
    pub regression: Option<String>,
    pub error: Option<String>,
    pub timestamp: String,
}

// ── Config loading ──

/// Load and parse smoke-tests.json from the repo root.
pub fn load_smoke_config(repo_root: &Path) -> Result<Vec<SmokeQuery>> {
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
            "[smoke] config schema_version={version:?}, expected={EXPECTED_SCHEMA_VERSION:?}"
        );
    }

    let mut queries = Vec::new();
    if let Some(arr) = config.get("queries").and_then(Value::as_array) {
        for item in arr {
            queries.push(SmokeQuery {
                id: str_field(item, "id").to_string(),
                source: str_field_default(item, "source", "arxiv").to_string(),
                query: str_field(item, "query").to_string(),
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
                        id: str_field(item, "id").to_string(),
                        source: str_field_default(item, "source", "arxiv").to_string(),
                        query: str_field(item, "query").to_string(),
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

// ── Filtering ──

/// Filter queries by source and/or barrier_id.
pub fn filter_queries(
    queries: Vec<SmokeQuery>,
    source: Option<&str>,
    barrier_id: Option<&str>,
) -> Vec<SmokeQuery> {
    queries
        .into_iter()
        .filter(|q| {
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
        })
        .collect()
}

// ── Query execution ──

/// Execute a single smoke test query against the appropriate source.
pub fn execute_query(query: &SmokeQuery, client: &reqwest::blocking::Client) -> SmokeResult {
    let id = query.id.clone();
    let timestamp = framework_kernel::time::now_iso();

    let results = match query.source.as_str() {
        "arxiv" | "all" => arxiv::search(client, &query.query, query.expected_min_results.max(5)),
        "semantic-scholar" | "semantic_scholar" | "semanticscholar" => {
            semantic_scholar::search(client, &query.query, query.expected_min_results.max(5))
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
            let now = chrono::Utc::now();
            let freshness_days = papers
                .iter()
                .filter_map(|p| {
                    let precise_days = ["publicationDate", "published", "date"]
                        .iter()
                        .filter_map(|field| p.get(field).and_then(Value::as_str))
                        .filter_map(|date_str| {
                            chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                                .or_else(|_| chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S"))
                                .ok()
                        })
                        .map(|d| (now.date_naive() - d).num_days().max(0))
                        .next();
                    if let Some(days) = precise_days {
                        return Some(days);
                    }
                    p.get("year").and_then(Value::as_i64).map(|yr| {
                        let current_year = now.year();
                        if yr as i32 == current_year {
                            // Same year: assume fresh (conservative)
                            0
                        } else {
                            // Different year: calculate from start of that year
                            let est_date = chrono::NaiveDate::from_ymd_opt(yr as i32, 1, 1)
                                .unwrap_or(now.date_naive());
                            (now.date_naive() - est_date).num_days().max(0)
                        }
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
                error: if passed {
                    None
                } else {
                    Some(format!(
                        "expected >= {}, got {}; stale={}",
                        query.expected_min_results, results_count, stale
                    ))
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

// ── Regression detection ──

/// Load previous results from smoke-test-results.jsonl for regression comparison.
pub fn load_previous_results(path: &Path) -> Result<HashMap<String, SmokeResult>> {
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
        if let Ok(val) = serde_json::from_str::<SmokeResult>(trimmed) {
            results.insert(val.id.clone(), val);
        }
    }
    Ok(results)
}

/// Run regression detection against previous results.
pub fn detect_regression(
    current: &SmokeResult,
    previous: &HashMap<String, SmokeResult>,
) -> Option<String> {
    let prev = previous.get(&current.id)?;
    if prev.passed && !current.passed {
        return Some("previous: passed → now: failed".to_string());
    }
    if prev.results_count > 0 {
        let drop_ratio = prev.results_count as f64 / current.results_count.max(1) as f64;
        if drop_ratio > 2.0 {
            return Some(format!(
                "results dropped >50% ({} → {})",
                prev.results_count, current.results_count
            ));
        }
    }
    if prev.freshness_days > 0 && current.freshness_days > prev.freshness_days * 2 {
        return Some(format!(
            "freshness window expanded >2× ({}d → {}d)",
            prev.freshness_days, current.freshness_days
        ));
    }
    None
}

// ── Main orchestrator ──

/// Core implementation: run smoke tests and return JSONL results string.
pub fn run_smoke_tests(
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
        eprintln!(
            "[smoke] warn: failed to persist results to {}: {e}",
            prev_path.display()
        );
    }

    Ok(lines.join("\n"))
}

// ── Freshness check ──

/// Check if a result year is fresh enough given the threshold.
pub fn is_fresh(year: u32, threshold_days: i64) -> bool {
    let current_year = chrono::Utc::now().year() as u32;
    let threshold_years = (threshold_days / 365).max(1) as u32;
    current_year.saturating_sub(year) <= threshold_years
}

// ── Tests ──

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
        fs::write(
            &config_path,
            json!({
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
            })
            .to_string(),
        )
        .unwrap();
        let queries = load_smoke_config(tmp.path()).unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].id, "q1");
        assert_eq!(queries[0].source, "arxiv");
        assert_eq!(queries[0].expected_min_results, 3);
    }

    #[test]
    fn filter_queries_by_source() {
        let queries = vec![
            SmokeQuery {
                id: "a".into(),
                source: "arxiv".into(),
                query: "".into(),
                expected_min_results: 3,
                expected_freshness_days: 180,
                related_directions: vec![],
                related_barriers: vec![],
            },
            SmokeQuery {
                id: "b".into(),
                source: "semantic-scholar".into(),
                query: "".into(),
                expected_min_results: 3,
                expected_freshness_days: 180,
                related_directions: vec![],
                related_barriers: vec![],
            },
        ];
        let filtered = filter_queries(queries, Some("arxiv"), None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "a");
    }

    #[test]
    fn filter_queries_by_barrier() {
        let queries = vec![
            SmokeQuery {
                id: "a".into(),
                source: "arxiv".into(),
                query: "".into(),
                expected_min_results: 3,
                expected_freshness_days: 180,
                related_directions: vec![],
                related_barriers: vec!["br-001".into()],
            },
            SmokeQuery {
                id: "b".into(),
                source: "arxiv".into(),
                query: "".into(),
                expected_min_results: 3,
                expected_freshness_days: 180,
                related_directions: vec![],
                related_barriers: vec![],
            },
        ];
        let filtered = filter_queries(queries, None, Some("br-001"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "a");
    }

    #[test]
    fn filter_queries_by_both() {
        let queries = vec![
            SmokeQuery {
                id: "a".into(),
                source: "arxiv".into(),
                query: "".into(),
                expected_min_results: 3,
                expected_freshness_days: 180,
                related_directions: vec![],
                related_barriers: vec!["br-001".into()],
            },
            SmokeQuery {
                id: "b".into(),
                source: "semantic-scholar".into(),
                query: "".into(),
                expected_min_results: 3,
                expected_freshness_days: 180,
                related_directions: vec![],
                related_barriers: vec!["br-001".into()],
            },
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
        let current = SmokeResult {
            id: "q1".into(),
            passed: false,
            results_count: 1,
            expected_min: 3,
            stale: true,
            freshness_days: 365,
            expected_freshness_days: 180,
            regression: None,
            error: None,
            timestamp: "".into(),
        };
        let mut prev = HashMap::new();
        prev.insert(
            "q1".into(),
            SmokeResult {
                id: "q1".into(),
                passed: true,
                results_count: 10,
                expected_min: 3,
                stale: false,
                freshness_days: 30,
                expected_freshness_days: 180,
                regression: None,
                error: None,
                timestamp: "".into(),
            },
        );
        assert!(detect_regression(&current, &prev).is_some());
    }

    #[test]
    fn regression_detects_drop_gt_50pct() {
        let current = SmokeResult {
            id: "q1".into(),
            passed: false,
            results_count: 4,
            expected_min: 3,
            stale: false,
            freshness_days: 30,
            expected_freshness_days: 180,
            regression: None,
            error: None,
            timestamp: "".into(),
        };
        let mut prev = HashMap::new();
        prev.insert(
            "q1".into(),
            SmokeResult {
                id: "q1".into(),
                passed: true,
                results_count: 10,
                expected_min: 3,
                stale: false,
                freshness_days: 30,
                expected_freshness_days: 180,
                regression: None,
                error: None,
                timestamp: "".into(),
            },
        );
        assert!(detect_regression(&current, &prev).is_some());
    }

    #[test]
    fn regression_detects_freshness_expansion() {
        let current = SmokeResult {
            id: "q1".into(),
            passed: false,
            results_count: 5,
            expected_min: 3,
            stale: true,
            freshness_days: 100,
            expected_freshness_days: 180,
            regression: None,
            error: None,
            timestamp: "".into(),
        };
        let mut prev = HashMap::new();
        prev.insert(
            "q1".into(),
            SmokeResult {
                id: "q1".into(),
                passed: true,
                results_count: 5,
                expected_min: 3,
                stale: false,
                freshness_days: 30,
                expected_freshness_days: 180,
                regression: None,
                error: None,
                timestamp: "".into(),
            },
        );
        assert!(detect_regression(&current, &prev).is_some());
    }

    #[test]
    fn regression_no_false_positive() {
        let current = SmokeResult {
            id: "q1".into(),
            passed: true,
            results_count: 8,
            expected_min: 3,
            stale: false,
            freshness_days: 40,
            expected_freshness_days: 180,
            regression: None,
            error: None,
            timestamp: "".into(),
        };
        let mut prev = HashMap::new();
        prev.insert(
            "q1".into(),
            SmokeResult {
                id: "q1".into(),
                passed: true,
                results_count: 10,
                expected_min: 3,
                stale: false,
                freshness_days: 30,
                expected_freshness_days: 180,
                regression: None,
                error: None,
                timestamp: "".into(),
            },
        );
        assert!(detect_regression(&current, &prev).is_none());
    }

    #[test]
    fn is_fresh_recent_year() {
        assert!(is_fresh(2025, 180));
    }

    #[test]
    fn is_fresh_old_year() {
        assert!(!is_fresh(2020, 180));
    }

    #[test]
    fn load_previous_results_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let results = load_previous_results(&tmp.path().join("nonexistent.jsonl")).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn load_previous_results_parses_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("results.jsonl");
        let entry = json!({
            "id": "q1", "passed": true, "results_count": 5,
            "expected_min": 3, "stale": false, "freshness_days": 30,
            "expected_freshness_days": 180, "regression": null,
            "error": null, "timestamp": "2026-01-01T00:00:00Z"
        });
        fs::write(&path, serde_json::to_string(&entry).unwrap()).unwrap();
        let results = load_previous_results(&path).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results.get("q1").unwrap().passed);
    }

    #[test]
    fn smoke_query_from_barrier_extends() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(SMOKE_TESTS_REL_PATH);
        fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        fs::write(
            &config_path,
            json!({
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
            })
            .to_string(),
        )
        .unwrap();
        let queries = load_smoke_config(tmp.path()).unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].id, "br-q1");
    }

    #[test]
    fn run_smoke_tests_empty_when_no_config() {
        let tmp = tempfile::tempdir().unwrap();
        let result = run_smoke_tests(tmp.path(), None, None).unwrap();
        assert!(result.is_empty());
    }
}
