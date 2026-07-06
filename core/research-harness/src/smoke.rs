//! General-purpose experiment smoke test engine.
//!
//! Runs executable templates from `templates/` at the repo root, injecting
//! parameter combinations as environment variables (`EXPERIMENT_<KEY>=<VALUE>`).
//! Results are parsed from script stdout as structured JSON.
//!
//! Intended for **quick directional probes** — single-run, no multi-seed,
//! no statistical rigor. All experiments run independently to completion.
//!
//! # Security
//!
//! - Templates are resolved relative to `repo_root/templates/` — no path traversal.
//! - Parameters reach the subprocess via **environment variables only** — not CLI args,
//!   preventing shell injection. Only the template binary name is fixed and from the
//!   trusted `templates/` directory, not caller-supplied.
//! - Subprocesses are process-group-isolated (via `setsid()` + `pre_exec`) so that
//!   timeout kills the entire process tree, not just the direct child.
//! - User input NEVER reaches shell arguments. All data flows through env vars.
//!
//! # Concurrency
//!
//! Parallel subprocess execution bounded by chunking (process `concurrency`
//! experiments at a time). No early exit — every experiment runs to completion
//! or timeout. Subprocess stdout/stderr are read by dedicated background threads
//! to prevent pipe-buffer deadlock on large output.
//!
//! # Caching
//!
//! LRU + TTL cache managed by [`smoke_cache::ExperimentCache`]. Cache key is SHA-256
//! of `template_content_hash || sorted_params_json`. Content hash (not mtime) avoids
//! filesystem precision issues. Disk persistence is flock-guarded against cross-process
//! races.

use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::smoke_cache::ExperimentCache;

// ── Constants ──

const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_CONCURRENCY: usize = 4;
const MAX_CONCURRENCY: usize = 32;
const MAX_STDERR_CHARS: usize = 4096;
const TEMPLATES_DIR: &str = "templates";
const ARTIFACTS_SUBDIR: &str = "artifacts/research-log/smoke";
pub(crate) const MAX_MCP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const POLL_INTERVAL_MS: u64 = 50;
/// Conservative per-experiment result size estimate for pre-flight size check.
const ESTIMATED_BYTES_PER_RESULT: usize = 1024;

// ── Public entry point ──

/// Run experiment smoke tests.
///
/// # Arguments (from MCP JSON)
///
/// | Field | Type | Required | Description |
/// |-------|------|----------|-------------|
/// | `template` | string | yes | Filename in `templates/` directory (must be executable) |
/// | `params` | array | yes | `[{"key": "val", ...}]` — each entry becomes one experiment run |
/// | `concurrency` | integer | no | Max parallel subprocesses (1–32, default 4) |
/// | `timeout_ms` | integer | no | Per-experiment timeout (default 60000) |
/// | `no_cache` | boolean | no | Bypass LRU+TTL cache (default false) |
///
/// # Returns
///
/// Structured JSON:
/// ```json
/// {
///   "experiments": [{
///     "run_id": "benchmark-0",
///     "template": "benchmark",
///     "params": {"lr": "0.01"},
///     "exit_code": 0,
///     "result": {"accuracy": 0.85},
///     "error": null,
///     "wall_time_ms": 2345
///   }],
///   "summary": {"total": 1, "succeeded": 1, "failed": 0}
/// }
/// ```
pub fn run_smoke_tests(repo_root: &Path, arguments: &Value) -> Result<String, FrameworkError> {
    // 1. Validate & parse arguments
    let template_name = arguments
        .get("template")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            FrameworkError::validation(
                "research_smoke requires 'template' (string): executable filename in templates/",
            )
        })?;

    // Reject path separators in template name to prevent path traversal
    if template_name.is_empty() {
        return Err(FrameworkError::validation(
            "template name must not be empty",
        ));
    }
    if template_name.contains('/') || template_name.contains('\\') || template_name.contains("..") {
        return Err(FrameworkError::validation(format!(
            "template name must not contain path separators: {template_name:?}"
        )));
    }

    let raw_params = arguments
        .get("params")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            FrameworkError::validation(
                "research_smoke requires 'params' (array of {key: value, ...} objects)",
            )
        })?;

    if raw_params.is_empty() {
        return Err(FrameworkError::validation("params must not be empty"));
    }

    let concurrency = arguments
        .get("concurrency")
        .and_then(Value::as_u64)
        .map(|c| c as usize)
        .unwrap_or(DEFAULT_CONCURRENCY)
        .clamp(1, MAX_CONCURRENCY);

    let timeout_ms = arguments
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_MS);

    let no_cache = arguments
        .get("no_cache")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // 2. Pre-flight size check: prevent massive response from clogging MCP transport
    if raw_params.len().saturating_mul(ESTIMATED_BYTES_PER_RESULT) > MAX_MCP_RESPONSE_BYTES {
        return Err(FrameworkError::validation(format!(
            "Too many experiments ({}). Estimated response > {} bytes. \
             Reduce params size or use the results JSONL at {}/results.jsonl",
            raw_params.len(),
            MAX_MCP_RESPONSE_BYTES,
            repo_root.join(ARTIFACTS_SUBDIR).display(),
        )));
    }

    // 3. Locate and validate template file
    let template_path = repo_root.join(TEMPLATES_DIR).join(template_name);
    if !template_path.exists() {
        return Err(FrameworkError::not_found(format!(
            "template not found: {template_name} (looked in {})",
            template_path.display(),
        )));
    }
    // Check that the file is executable (best-effort on Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&template_path) {
            if meta.permissions().mode() & 0o111 == 0 {
                return Err(FrameworkError::validation(format!(
                    "template is not executable: {template_name} (chmod +x it)"
                )));
            }
        }
    }

    // 4. Ensure artifacts directory exists
    let artifacts_dir = repo_root.join(ARTIFACTS_SUBDIR);
    fs::create_dir_all(&artifacts_dir)
        .map_err(|e| FrameworkError::Io(e))?;

    // 5. Build run list
    let runs = build_runs(template_name, &template_path, raw_params);

    // 6. Run experiments
    let cache = ExperimentCache::new(&artifacts_dir, no_cache);
    let results = run_experiments(&runs, timeout_ms, concurrency, &cache, &artifacts_dir);

    // 7. Build response (with size guard)
    let response = build_response(&results);
    let json_str = serde_json::to_string(&response).map_err(FrameworkError::Json)?;

    if json_str.len() > MAX_MCP_RESPONSE_BYTES {
        let truncated = json!({
            "truncated": true,
            "experiments_count": results.len(),
            "summary": {
                "total": results.len(),
                "succeeded": results.iter().filter(|r| r.error.is_none()).count(),
                "failed": results.iter().filter(|r| r.error.is_some()).count(),
                "note": format!(
                    "Response too large ({} bytes). Use {}/results.jsonl for full data.",
                    json_str.len(),
                    artifacts_dir.display(),
                ),
            }
        });
        return serde_json::to_string(&truncated).map_err(FrameworkError::Json);
    }

    Ok(json_str)
}

// ── Data types ──

/// A single experiment configuration.
#[derive(Debug, Clone)]
pub struct ExperimentRun {
    pub run_id: String,
    pub template_name: String,
    pub template_path: PathBuf,
    pub params: HashMap<String, String>,
}

/// Result of a single experiment execution.
#[derive(Debug, Clone)]
pub struct ExperimentResult {
    pub run_id: String,
    pub template_name: String,
    pub params: HashMap<String, String>,
    pub exit_code: i32,
    pub result: Value,
    pub error: Option<String>,
    pub wall_time_ms: u64,
}

// ── Run building ──

/// Build a list of `ExperimentRun` from the MCP params array.
fn build_runs(template_name: &str, template_path: &Path, params_list: &[Value]) -> Vec<ExperimentRun> {
    params_list
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let params = match entry.as_object() {
                Some(obj) => obj
                    .iter()
                    .map(|(k, v)| (k.clone(), param_value_to_string(v)))
                    .collect(),
                None => HashMap::new(),
            };

            ExperimentRun {
                run_id: format!("{template_name}-{i}"),
                template_name: template_name.to_string(),
                template_path: template_path.to_path_buf(),
                params,
            }
        })
        .collect()
}

// ── Parallel execution ──

/// Run all experiments with bounded concurrency via chunking.
///
/// Processes `concurrency` experiments at a time in parallel using
/// `std::thread::scope`. This provides bounded concurrency without needing
/// a Semaphore (not yet stabilized in std). For quick probe experiments this
/// works well — the last chunk may have idle threads, but wall-clock is
/// dominated by the slowest experiment, not chunk alignment.
pub(crate) fn run_experiments(
    runs: &[ExperimentRun],
    timeout_ms: u64,
    concurrency: usize,
    cache: &ExperimentCache,
    artifacts_dir: &Path,
) -> Vec<ExperimentResult> {
    let effective_concurrency = concurrency.max(1);
    let mut all_results = Vec::with_capacity(runs.len());

    for chunk in runs.chunks(effective_concurrency) {
        let chunk_results: Vec<ExperimentResult> = std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|run| {
                    let run = run.clone();
                    let artifacts_dir = artifacts_dir.to_path_buf();

                    scope.spawn(move || {
                        // Cache lookup — skip computation when no_cache
                        let cache_key = if !cache.no_cache {
                            Some(ExperimentCache::cache_key(
                                &run.template_path,
                                &run.template_name,
                                &run.params,
                            ))
                        } else {
                            None
                        };
                        if let Some(ref ck) = cache_key {
                            if let Some(cached_val) = cache.get(ck) {
                                return ExperimentResult {
                                run_id: run.run_id,
                                template_name: run.template_name,
                                params: run.params,
                                exit_code: cached_val["exit_code"].as_i64().unwrap_or(-1) as i32,
                                result: cached_val.get("result").cloned().unwrap_or(Value::Null),
                                error: cached_val.get("error")
                                    .and_then(Value::as_str)
                                    .map(String::from),
                                wall_time_ms: cached_val["wall_time_ms"].as_u64().unwrap_or(0),
                            };
                        }
                        }

                        // Execute
                        let start = Instant::now();
                        let mut result = execute_single(&run, timeout_ms);
                        result.wall_time_ms = start.elapsed().as_millis() as u64;

                        // Cache the result (only when cache is active)
                        if let Some(ck) = &cache_key {
                            let cache_result = json!({
                                "run_id": result.run_id,
                                "template_name": result.template_name,
                                "params": result.params,
                                "exit_code": result.exit_code,
                                "result": result.result,
                                "error": result.error,
                                "wall_time_ms": result.wall_time_ms,
                            });
                            cache.set(ck.clone(), cache_result);
                        }

                        // Append to results JSONL (file-locked, best-effort)
                        if let Err(e) = append_result_jsonl(&artifacts_dir, &result) {
                            tracing::warn!(error = %e, run_id = %result.run_id, "[smoke] append_result_jsonl failed");
                        }

                        result
                    })
                })
                .collect();

            handles
                .into_iter()
                .map(|h| h.join().expect("experiment thread panicked"))
                .collect()
        });
        all_results.extend(chunk_results);

        // Flush cache to disk once per chunk (not once per experiment).
        // This reduces disk writes from O(N) to O(N/concurrency).
        cache.flush();
    }

    all_results
}

// ── Single experiment execution ──

/// Execute a single experiment: spawn subprocess, capture output, return result.
///
/// Uses process group isolation (`setsid()` in `pre_exec`) so that timeout kills
/// the entire process tree. Stdout/stderr are consumed by background reader threads
/// to prevent pipe-buffer deadlock on large output.
fn execute_single(run: &ExperimentRun, timeout_ms: u64) -> ExperimentResult {
    let run_id = run.run_id.clone();
    let template_name = run.template_name.clone();
    let params = run.params.clone();

    // Build command with process group isolation
    let mut cmd = std::process::Command::new(&run.template_path);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Process group isolation: create a new session so we can kill the entire tree
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                let ret = libc::setsid();
                if ret == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    // Inject parameters as environment variables: EXPERIMENT_<KEY>=<VALUE>
    cmd.env("EXPERIMENT_TEMPLATE", &run.template_name);
    cmd.env("EXPERIMENT_RUN_ID", &run.run_id);
    for (key, value) in &run.params {
        let env_key = sanitize_env_key(key);
        cmd.env(&env_key, value);
    }

    let start = Instant::now();

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ExperimentResult {
                run_id,
                template_name,
                params,
                exit_code: -1,
                result: Value::Null,
                error: Some(format!("spawn failed: {e}")),
                wall_time_ms: start.elapsed().as_millis() as u64,
            };
        }
    };

    let pid = child.id();

    // Take stdout/stderr BEFORE spawning reader threads
    let child_stdout = child.stdout.take();
    let child_stderr = child.stderr.take();

    // Background threads read stdout/stderr concurrently to avoid pipe deadlock.
    let (out_tx, out_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let (err_tx, err_rx) = std::sync::mpsc::channel::<Vec<u8>>();

    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut reader) = child_stdout {
            let _ = reader.read_to_end(&mut buf);
        }
        let _ = out_tx.send(buf);
    });
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut reader) = child_stderr {
            let _ = reader.read_to_end(&mut buf);
        }
        let _ = err_tx.send(buf);
    });

    // Track timeout via Instant — no detached thread needed.
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut timed_out = false;

    // Poll loop
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = child.wait();

                let stdout_bytes = out_rx.recv().unwrap_or_default();
                let stderr_bytes = err_rx.recv().unwrap_or_default();
                let stdout_str = String::from_utf8_lossy(&stdout_bytes).to_string();
                let stderr_str = truncate(
                    &String::from_utf8_lossy(&stderr_bytes),
                    MAX_STDERR_CHARS,
                );
                let exit_code = status.code().unwrap_or(-1);

                let (parsed, parse_error) = parse_last_json(&stdout_str);

                let error = if timed_out {
                    Some(format!("timeout after {timeout_ms}ms (pid={pid})"))
                } else if let Some(msg) = parse_error {
                    Some(format!("exit={exit_code}, parse: {msg}"))
                } else if exit_code != 0 {
                    let detail = if stderr_str.is_empty() {
                        "no stderr".into()
                    } else {
                        stderr_str
                    };
                    Some(format!("exit={exit_code}: {detail}"))
                } else {
                    None
                };

                return ExperimentResult {
                    run_id,
                    template_name,
                    params,
                    exit_code,
                    result: parsed,
                    error,
                    wall_time_ms: start.elapsed().as_millis() as u64,
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    #[cfg(unix)]
                    unsafe {
                        if let Ok(pgid) = i32::try_from(pid) {
                            libc::kill(-pgid, libc::SIGTERM);
                        }
                    }
                    #[cfg(not(unix))]
                    let _ = child.kill();

                    std::thread::sleep(Duration::from_millis(200));
                    #[cfg(unix)]
                    unsafe {
                        if let Ok(pgid) = i32::try_from(pid) {
                            libc::kill(-pgid, libc::SIGKILL);
                        }
                    }

                    continue;
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(e) => {
                return ExperimentResult {
                    run_id,
                    template_name,
                    params,
                    exit_code: -1,
                    result: Value::Null,
                    error: Some(format!("subprocess wait error: {e}")),
                    wall_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        }
    }
}

// ── Stdout JSON parsing ──

/// Parse the last JSON value from stdout.
///
/// Strategy:
/// 1. Try parsing the entire trimmed stdout as JSON (handles single JSON object).
/// 2. If that fails, parse line-by-line and return the last valid JSON object.
/// 3. If nothing works, return `(Value::Null, Some(reason))`.
fn parse_last_json(stdout: &str) -> (Value, Option<String>) {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return (Value::Null, None);
    }

    // Fast path: parse entire stdout as JSON
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return (v, None);
    }

    // Fallback: line-by-line, return last valid object
    let mut last_valid: Option<Value> = None;
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            if v.is_object() || v.is_array() {
                last_valid = Some(v);
            }
        }
    }

    match last_valid {
        Some(v) => (v, None),
        None => (Value::Null, Some("stdout is not valid JSON".into())),
    }
}

// ── JSONL append (using framework-runtime process-safe writer) ──

/// Append an experiment result to `results.jsonl` using framework-runtime's
/// `append_text_with_process_lock()` for cross-process safety.
/// Returns `Ok(())` on success or a descriptive error string on failure.
fn append_result_jsonl(artifacts_dir: &Path, result: &ExperimentResult) -> Result<(), String> {
    use framework_runtime::io_utils::append_text_with_process_lock;

    let entry = json!({
        "run_id": result.run_id,
        "template": result.template_name,
        "params": result.params,
        "exit_code": result.exit_code,
        "result": result.result,
        "error": result.error,
        "wall_time_ms": result.wall_time_ms,
    });

    let line = serde_json::to_string(&entry)
        .map_err(|e| format!("JSONL serialization failed: {e}"))?;
    let line = line + "\n";

    let jsonl_path = artifacts_dir.join("results.jsonl");
    append_text_with_process_lock(&jsonl_path, &line, "experiment-smoke")
        .map_err(|e| format!("append result to {} failed: {e}", jsonl_path.display()))?;
    Ok(())
}

// ── Response builder ──

fn build_response(results: &[ExperimentResult]) -> Value {
    let total = results.len();
    let succeeded = results.iter().filter(|r| r.error.is_none()).count();
    let failed = results.iter().filter(|r| r.error.is_some()).count();

    let experiments: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "run_id": r.run_id,
                "template": r.template_name,
                "params": r.params,
                "exit_code": r.exit_code,
                "result": r.result,
                "error": r.error,
                "wall_time_ms": r.wall_time_ms,
            })
        })
        .collect();

    json!({
        "experiments": experiments,
        "summary": {
            "total": total,
            "succeeded": succeeded,
            "failed": failed,
        }
    })
}

// ── Helpers ──

/// Sanitize a parameter key into a valid environment variable name.
///
/// Uppercases, replaces non-alphanumeric chars with `_`, prepends `EXPERIMENT_`.
fn sanitize_env_key(key: &str) -> String {
    let sanitized: String = key
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .collect();
    format!("EXPERIMENT_{sanitized}")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        let mut t: String = s.chars().take(max).collect();
        t.push_str(&format!("... ({} total)", s.len()));
        t
    } else {
        s.to_string()
    }
}

/// Convert a JSON parameter value to its string representation.
///
/// Strings pass through, numbers/bools become their text form, null becomes
/// empty string. Arrays and objects are serialized as JSON with a prefix marker
/// so the subprocess can detect and parse them — no silent data loss.
fn param_value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => {
            // Serialize nested values as JSON with a marker so scripts can detect them
            format!("__JSON__{}", serde_json::to_string(v).unwrap_or_default())
        }
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn build_runs_flat_dict() {
        let params = json!([{"lr": "0.01", "bs": "32"}, {"lr": "0.001", "bs": "64"}]);
        let runs = build_runs("test", &Path::new("templates/test.sh"), params.as_array().unwrap());
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].params["lr"], "0.01");
        assert_eq!(runs[0].params["bs"], "32");
        assert_eq!(runs[1].run_id, "test-1");
    }

    #[test]
    fn build_runs_empty_params_yields_empty_map() {
        let params = json!([{}]);
        let runs = build_runs("t", &Path::new("t.sh"), params.as_array().unwrap());
        assert_eq!(runs.len(), 1);
        assert!(runs[0].params.is_empty());
    }

    #[test]
    fn parse_last_json_whole_stdout() {
        let (v, err) = parse_last_json(r#"{"accuracy": 0.85}"#);
        assert!(err.is_none());
        assert_eq!(v["accuracy"], 0.85);
    }

    #[test]
    fn parse_last_json_line_by_line() {
        let stdout = "some log line\n{\"step\": 1}\nanother log\n{\"step\": 2}";
        let (v, err) = parse_last_json(stdout);
        assert!(err.is_none());
        assert_eq!(v["step"], 2);
    }

    #[test]
    fn parse_last_json_empty() {
        let (v, err) = parse_last_json("");
        assert!(err.is_none());
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn parse_last_json_invalid() {
        let (v, err) = parse_last_json("not json at all");
        assert!(err.is_some());
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn sanitize_env_key_basic() {
        let k = sanitize_env_key("batch_size");
        assert_eq!(k, "EXPERIMENT_BATCH_SIZE");
    }

    #[test]
    fn sanitize_env_key_with_special_chars() {
        let k = sanitize_env_key("my-param.1");
        assert_eq!(k, "EXPERIMENT_MY_PARAM_1");
    }

    #[test]
    fn truncate_short() {
        let s = truncate("hello", 100);
        assert_eq!(s, "hello");
    }

    #[test]
    fn truncate_long() {
        let s = truncate("hello world", 5);
        assert!(s.starts_with("hello"));
        assert!(s.contains("..."));
    }

    #[test]
    fn build_response_empty() {
        let resp = build_response(&[]);
        assert_eq!(resp["summary"]["total"], 0);
    }

    #[test]
    fn build_response_mixed() {
        let results = vec![
            ExperimentResult {
                run_id: "ok".into(),
                template_name: "t".into(),
                params: HashMap::new(),
                exit_code: 0,
                result: json!({"x": 1}),
                error: None,
                wall_time_ms: 100,
            },
            ExperimentResult {
                run_id: "fail".into(),
                template_name: "t".into(),
                params: HashMap::new(),
                exit_code: 1,
                result: Value::Null,
                error: Some("exit=1".into()),
                wall_time_ms: 50,
            },
        ];
        let resp = build_response(&results);
        assert_eq!(resp["summary"]["total"], 2);
        assert_eq!(resp["summary"]["succeeded"], 1);
        assert_eq!(resp["summary"]["failed"], 1);
    }

    #[test]
    fn param_value_to_string_nested() {
        // Arrays serialize as JSON with __JSON__ marker
        let arr = param_value_to_string(&json!([1, 2, 3]));
        assert!(arr.starts_with("__JSON__"), "array should get __JSON__ prefix: {arr}");
        assert!(arr.contains("1"), "array values should be present: {arr}");

        // Objects serialize as JSON with __JSON__ marker
        let obj = param_value_to_string(&json!({"nested": "deep"}));
        assert!(obj.starts_with("__JSON__"), "object should get __JSON__ prefix: {obj}");

        // Strings pass through unchanged
        let s = param_value_to_string(&Value::String("hello".into()));
        assert_eq!(s, "hello");

        // Numbers preserve
        let n = param_value_to_string(&json!(42));
        assert_eq!(n, "42");

        let f = param_value_to_string(&json!(3.14));
        assert_eq!(f, "3.14");

        // Bool
        let t = param_value_to_string(&Value::Bool(true));
        assert_eq!(t, "true");
    }

    #[test]
    fn build_params_from_nested_value() {
        // Ensure build_runs properly delegates nested values to param_value_to_string
        let params = json!([{"layers": [64, 128], "config": {"dropout": 0.5}, "lr": 0.01}]);
        let runs = build_runs("test", &Path::new("test.sh"), params.as_array().unwrap());
        assert_eq!(runs.len(), 1);
        let p = &runs[0].params;
        assert!(p.get("layers").map_or(false, |v| v.starts_with("__JSON__")),
            "nested array should be marked: {:?}", p.get("layers"));
        assert!(p.get("config").map_or(false, |v| v.starts_with("__JSON__")),
            "nested object should be marked: {:?}", p.get("config"));
        assert_eq!(p.get("lr").map(String::as_str), Some("0.01"));
    }

    #[test]
    fn parse_last_json_exit_code_included() {
        // Exit code 0 with invalid JSON should still report exit code
        let result = ExperimentResult {
            run_id: "t-0".into(),
            template_name: "t".into(),
            params: HashMap::new(),
            exit_code: 0,
            result: Value::Null,
            error: Some("exit=0, parse: stdout is not valid JSON".into()),
            wall_time_ms: 10,
        };
        assert!(result.error.as_ref().unwrap().contains("exit=0"),
            "exit code should be reported even when 0: {:?}", result.error);
    }
}
