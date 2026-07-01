//! Python math backend bridge — real SymPy/Z3 subprocess integration.
//!
//! Calls `uv run -m math_backend --stdin-json` with PYTHONPATH set to the
//! project's `python/` directory. Falls back gracefully when Python or
//! the math_backend module is unavailable.
//!
//! # Caching
//!
//! Backend availability is cached for 30 seconds to avoid spawning a
//! subprocess on every tool call. The cache is invalidated by timeout only.
//!
//! # Layer boundary
//!
//! FEATURE layer only. Tool dispatch belongs in `mcp_tools.rs`.

use core_errors::FrameworkError;
use serde_json::{Value, json};
use std::sync::OnceLock;
use std::time::Instant;

/// Default timeout for Python backend calls in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Cache TTL for backend availability status in seconds.
const STATUS_CACHE_TTL: u64 = 30;

// ===========================================================================
// Cached backend status
// ===========================================================================

struct StatusCache {
    result: Value,
    expires_at: Instant,
}

static BACKEND_STATUS: OnceLock<std::sync::Mutex<Option<StatusCache>>> = OnceLock::new();

fn status_cache() -> &'static std::sync::Mutex<Option<StatusCache>> {
    BACKEND_STATUS.get_or_init(|| std::sync::Mutex::new(None))
}

/// Get the Python project root (parent of the `python/` directory).
fn python_root() -> Result<String, FrameworkError> {
    // Find the manifest dir at compile time using env!("CARGO_MANIFEST_DIR")
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // The python/ directory is at <manifest>/python/
    let python_path = std::path::PathBuf::from(manifest_dir).join("python");
    if !python_path.exists() {
        return Err(FrameworkError::validation(format!(
            "python backend not found at: {}",
            python_path.display()
        )));
    }
    Ok(python_path.to_string_lossy().to_string())
}

// ===========================================================================
// Core API
// ===========================================================================

/// Call the Python math backend with an operation and parameters.
///
/// Returns the JSON response on success, or an error on failure.
pub fn call_math_backend(op: &str, params: Value) -> Result<Value, FrameworkError> {
    call_math_backend_with_timeout(op, params, DEFAULT_TIMEOUT_MS)
}

/// Like `call_math_backend` but with a configurable timeout.
pub fn call_math_backend_with_timeout(
    op: &str,
    params: Value,
    timeout_ms: u64,
) -> Result<Value, FrameworkError> {
    call_math_backend_inner(op, params, timeout_ms, &["uv", "run", "-m", "math_backend", "--stdin-json"])
}

/// Internal variant that accepts a command path and args (for testability).
fn call_math_backend_inner(
    op: &str,
    params: Value,
    timeout_ms: u64,
    cmd_and_args: &[&str],
) -> Result<Value, FrameworkError> {
    let python_dir = python_root()?;

    let request = json!({
        "op": op,
        "params": params,
    });

    let cmd = cmd_and_args.first().ok_or_else(|| {
        FrameworkError::validation("empty command for math backend".to_string())
    })?;
    let args = &cmd_and_args[1..];

    // Need to set PYTHONPATH so `uv run -m math_backend` can find the module
    // We use a helper script that sets PYTHONPATH and then invokes the module.
    let result = std::process::Command::new(cmd)
        .args(args)
        .env("PYTHONPATH", &python_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match result {
        Ok(c) => c,
        Err(e) => {
            // uv not available or other system error
            tracing::warn!("[python_bridge] failed to spawn {cmd}: {e}");
            return Err(FrameworkError::validation(format!(
                "Python backend unavailable: {e}"
            )));
        }
    };

    // Write stdin
    let input = serde_json::to_string(&request)?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        if let Err(e) = stdin.write_all(input.as_bytes()) {
            tracing::warn!("[python_bridge] stdin write error: {e}");
        }
    }

    // Wait with timeout
    let start = Instant::now();
    let max_duration = std::time::Duration::from_millis(timeout_ms);
    let poll_interval = std::time::Duration::from_millis(50);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(FrameworkError::Io)?;
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if !status.success() {
                    // Check stdout for error JSON
                    if let Ok(val) = serde_json::from_str::<Value>(&stdout) {
                        if val.get("status").and_then(|v| v.as_str()) == Some("error") {
                            return Err(FrameworkError::validation(format!(
                                "math_backend {} failed: {}",
                                op,
                                val.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error")
                            )));
                        }
                    }
                    let detail = truncate(&stderr, 500);
                    return Err(FrameworkError::validation(format!(
                        "math_backend {} failed (exit={:?}): {}",
                        op,
                        output.status.code(),
                        detail
                    )));
                }

                // Parse JSON response
                let response: Value = serde_json::from_str(&stdout).map_err(|e| {
                    FrameworkError::validation(format!(
                        "parse math_backend output: {e}, stdout: {}, stderr: {}",
                        truncate(&stdout, 200),
                        truncate(&stderr, 200),
                    ))
                })?;

                // Check for error status
                if response.get("status").and_then(|v| v.as_str()) == Some("error") {
                    return Err(FrameworkError::validation(format!(
                        "math_backend {} error: {}",
                        op,
                        response.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
                    )));
                }

                // Return result field
                return Ok(response.get("result").cloned().unwrap_or(response))
            }
            Ok(None) => {
                if start.elapsed() > max_duration {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(FrameworkError::validation(format!(
                        "math backend timed out after {timeout_ms}ms (op={op})"
                    )));
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => return Err(FrameworkError::Io(e)),
        }
    }
}

// ===========================================================================
// Backend availability probes
// ===========================================================================

/// Check if the Python math backend is available (sympy + z3 installed).
pub fn backend_available() -> bool {
    match fetch_status() {
        Ok(status) => {
            let sympy_ok = status
                .pointer("/sympy/available")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let z3_ok = status
                .pointer("/z3/available")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            sympy_ok && z3_ok
        }
        Err(_) => false,
    }
}

/// Check if SymPy is available via the Python backend.
pub fn sympy_available() -> bool {
    match fetch_status() {
        Ok(status) => status
            .pointer("/sympy/available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Check if Z3 is available via the Python backend.
pub fn z3_available() -> bool {
    match fetch_status() {
        Ok(status) => status
            .pointer("/z3/available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// Fetch backend status from the Python process (with caching).
fn fetch_status() -> Result<Value, FrameworkError> {
    let now = Instant::now();

    // Check cache
    if let Ok(cache_guard) = status_cache().lock() {
        if let Some(cache) = cache_guard.as_ref() {
            if now < cache.expires_at {
                return Ok(cache.result.clone());
            }
        }
    }

    // Call Python backend
    let result = call_math_backend_with_timeout("backend_status", json!({}), 10_000)?;

    // Update cache
    if let Ok(mut cache_guard) = status_cache().lock() {
        cache_guard.replace(StatusCache {
            result: result.clone(),
            expires_at: now + std::time::Duration::from_secs(STATUS_CACHE_TTL),
        });
    }

    Ok(result)
}

/// Get a full backend status report (for `tool_math_backend_available`).
pub fn get_full_status_report() -> Value {
    match fetch_status() {
        Ok(status) => {
            let default_obj = json!({"available": false, "version": "unknown"});
            let sympy = status.get("sympy").cloned().unwrap_or(default_obj.clone());
            let z3 = status.get("z3").cloned().unwrap_or(default_obj.clone());
            let lean_raw = status.get("lean").cloned().unwrap_or(json!({"available": false}));

            json!({
                "sympy": {
                    "available": sympy.get("available").and_then(|v| v.as_bool()).unwrap_or(false),
                    "version": sympy.get("version").and_then(|v| v.as_str()).unwrap_or(""),
                    "description": sympy.get("description").and_then(|v| v.as_str()).unwrap_or("SymPy CAS"),
                },
                "z3": {
                    "available": z3.get("available").and_then(|v| v.as_bool()).unwrap_or(false),
                    "version": z3.get("version").and_then(|v| v.as_str()).unwrap_or(""),
                    "description": z3.get("description").and_then(|v| v.as_str()).unwrap_or("Z3 SMT solver"),
                },
                "lean": {
                    "available": lean_raw.get("available").and_then(|v| v.as_bool()).unwrap_or(false),
                    "description": "Lean theorem prover (system PATH)",
                },
                "python_backend": true,
            })
        }
        Err(_) => {
            json!({
                "sympy": {"available": false, "version": "", "description": "SymPy CAS"},
                "z3": {"available": false, "version": "", "description": "Z3 SMT solver"},
                "lean": {"available": false, "description": "Lean theorem prover"},
                "python_backend": false,
            })
        }
    }
}

// ===========================================================================
// Utility
// ===========================================================================

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        let end = s
            .char_indices()
            .take_while(|(i, _)| *i < max)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let mut t = s[..end].to_string();
        t.push_str(&format!("... ({} total)", s.len()));
        t
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short() {
        let s = truncate("hello", 100);
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_truncate_long() {
        let s = truncate("hello world", 5);
        assert!(s.starts_with("hello"));
        assert!(s.contains("..."));
    }

    #[test]
    fn test_python_root_exists() {
        let root = python_root().unwrap();
        assert!(!root.is_empty());
        let path = std::path::PathBuf::from(&root);
        assert!(path.exists(), "python dir should exist: {root}");
    }

    #[test]
    fn test_backend_available_probe() {
        // This probe may fail if uv or deps are not installed — not a hard fail
        let result = fetch_status();
        tracing::info!("backend status: {result:?}");
        // Should not panic even on failure
    }

    #[test]
    fn test_backend_status_cache_hit_and_expiry() {
        use std::time::Duration;
        let _lock = match cache_test_lock().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        // --- 1. Cache hit: pre-populate with known value ---
        let canned = json!({"cached": true, "value": 42});
        {
            let mut guard = status_cache().lock().unwrap();
            *guard = Some(StatusCache {
                result: canned.clone(),
                expires_at: Instant::now() + Duration::from_secs(60),
            });
        }

        let hit = fetch_status().expect("cache hit should return Ok");
        assert_eq!(hit, canned, "should return cached value (cache hit)");

        // --- 2. Cache expired: set stale value, verify reprobe ---
        let stale = json!({"stale": true});
        {
            let mut guard = status_cache().lock().unwrap();
            *guard = Some(StatusCache {
                result: stale.clone(),
                expires_at: Instant::now() - Duration::from_secs(1),
            });
        }

        match fetch_status() {
            Ok(refreshed) => {
                assert_ne!(refreshed, stale, "cache should be refreshed on expiry");
                let guard = status_cache().lock().unwrap();
                let cache = guard.as_ref().expect("cache should exist after refresh");
                assert!(
                    cache.expires_at > Instant::now(),
                    "cache expiry should be updated"
                );
            }
            Err(_) => {
                // Backend unavailable — stale value should persist
                let guard = status_cache().lock().unwrap();
                let cache = guard.as_ref().expect("stale value should persist");
                assert_eq!(cache.result, stale, "stale cache should persist on error");
            }
        }

        // --- 3. Cache empty: no cache, verify reprobe from scratch ---
        {
            let mut guard = status_cache().lock().unwrap();
            *guard = None;
        }

        match fetch_status() {
            Ok(_) => {
                let guard = status_cache().lock().unwrap();
                assert!(guard.is_some(), "cache should be populated after successful fetch");
            }
            Err(_) => {
                let guard = status_cache().lock().unwrap();
                assert!(guard.is_none(), "cache should remain empty on fetch error");
            }
        }
    }

    // =========================================================================
    // call_math_backend_inner — mock subprocess tests
    // =========================================================================

    #[test]
    fn test_inner_success_path() {
        let result = call_math_backend_inner(
            "sympy_verify",
            json!({"lhs": "x", "rhs": "x"}),
            5000,
            &[
                "/bin/sh",
                "-c",
                "cat > /dev/null; echo '{\"status\":\"ok\",\"result\":{\"equal\":true,\"difference\":\"0\"}}'",
            ],
        );
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
        let val = result.unwrap();
        assert_eq!(val["equal"], true, "result should contain equal=true");
    }

    #[test]
    fn test_inner_failed_spawn() {
        let result = call_math_backend_inner(
            "test_op",
            json!({}),
            1000,
            &["/nonexistent/nonexistent_binary_xyz789"],
        );
        assert!(result.is_err(), "expected Err for non-existent command");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Python backend unavailable"),
            "error should mention 'Python backend unavailable', got: {err}"
        );
    }

    #[test]
    fn test_inner_non_zero_exit_with_stderr() {
        let result = call_math_backend_inner(
            "test_op",
            json!({}),
            5000,
            &["/bin/sh", "-c", "cat > /dev/null; echo 'stderr msg' >&2; exit 1"],
        );
        assert!(result.is_err(), "expected Err for non-zero exit");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exit="), "error should mention exit=, got: {err}");
    }

    #[test]
    fn test_inner_json_error_stdout_exit_zero() {
        let result = call_math_backend_inner(
            "test_op",
            json!({}),
            5000,
            &[
                "/bin/sh",
                "-c",
                "cat > /dev/null; echo '{\"status\":\"error\",\"error\":\"test operation failed\"}'",
            ],
        );
        assert!(result.is_err(), "expected Err for error status in JSON");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("test operation failed"),
            "error should contain the JSON error message, got: {err}"
        );
    }

    #[test]
    fn test_inner_json_error_with_nonzero_exit() {
        let result = call_math_backend_inner(
            "test_op",
            json!({}),
            5000,
            &[
                "/bin/sh",
                "-c",
                "cat > /dev/null; echo '{\"status\":\"error\",\"error\":\"json error message\"}'; echo 'stderr' >&2; exit 1",
            ],
        );
        assert!(result.is_err(), "expected Err");
        let err = result.unwrap_err().to_string();
        // Should hit the JSON error path (checked before stderr)
        assert!(
            err.contains("json error message"),
            "error should contain the JSON error, got: {err}"
        );
    }

    #[test]
    fn test_inner_malformed_json() {
        let result = call_math_backend_inner(
            "test_op",
            json!({}),
            5000,
            &["/bin/sh", "-c", "cat > /dev/null; echo 'this is not valid json at all'"],
        );
        assert!(result.is_err(), "expected Err for malformed JSON");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("parse"), "error should mention parse, got: {err}");
    }

    #[test]
    fn test_inner_timeout() {
        let result = call_math_backend_inner(
            "test_op",
            json!({}),
            100,
            &["/bin/sh", "-c", "cat > /dev/null; sleep 1; echo '{\"status\":\"ok\"}'"],
        );
        let start = Instant::now();
        assert!(result.is_err(), "expected Err for timeout");
        let elapsed = start.elapsed();
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("timed out"),
            "error should mention timed out, got: {err}"
        );
        // Timeout should be fast (well under 1 second with 100ms timeout)
        assert!(
            elapsed.as_millis() < 1000,
            "timeout took too long: {elapsed:?}"
        );
    }

    #[test]
    fn test_inner_empty_cmd_and_args() {
        let result = call_math_backend_inner("test_op", json!({}), 1000, &[]);
        assert!(result.is_err(), "expected Err for empty command list");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("empty command"),
            "error should mention empty command, got: {err}"
        );
    }

    // =========================================================================
    // Cache layer tests (backend_available / sympy_available / z3_available)
    //
    // These tests modify the global BACKEND_STATUS cache.  A per-test mutex
    // prevents interference from other cache-modifying tests running in
    // parallel threads.
    // =========================================================================

    static CACHE_TEST_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> =
        std::sync::OnceLock::new();

    fn cache_test_lock() -> &'static std::sync::Mutex<()> {
        CACHE_TEST_LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    #[test]
    fn test_backend_available_true_with_prepopulated_cache() {
        let _lock = match cache_test_lock().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let canned = json!({
            "sympy": {"available": true, "version": "1.13"},
            "z3": {"available": true, "version": "4.13"},
        });
        {
            let mut guard = status_cache().lock().unwrap();
            *guard = Some(StatusCache {
                result: canned,
                expires_at: Instant::now() + std::time::Duration::from_secs(60),
            });
        }
        assert!(backend_available(), "backend_available should be true");
        assert!(sympy_available(), "sympy_available should be true");
        assert!(z3_available(), "z3_available should be true");
        status_cache().lock().unwrap().take();
    }

    #[test]
    fn test_backend_available_false_with_prepopulated_cache() {
        let _lock = match cache_test_lock().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let canned = json!({
            "sympy": {"available": false, "version": ""},
            "z3": {"available": false, "version": ""},
        });
        {
            let mut guard = status_cache().lock().unwrap();
            *guard = Some(StatusCache {
                result: canned,
                expires_at: Instant::now() + std::time::Duration::from_secs(60),
            });
        }
        assert!(!backend_available(), "backend_available should be false");
        assert!(!sympy_available(), "sympy_available should be false");
        assert!(!z3_available(), "z3_available should be false");
        status_cache().lock().unwrap().take();
    }

    #[test]
    fn test_backend_available_sympy_only() {
        let _lock = match cache_test_lock().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let canned = json!({
            "sympy": {"available": true, "version": "1.13"},
            "z3": {"available": false, "version": ""},
        });
        {
            let mut guard = status_cache().lock().unwrap();
            *guard = Some(StatusCache {
                result: canned,
                expires_at: Instant::now() + std::time::Duration::from_secs(60),
            });
        }
        // backend_available requires BOTH, so it should be false
        assert!(
            !backend_available(),
            "backend_available requires both backends"
        );
        assert!(sympy_available(), "sympy_available should be true");
        assert!(!z3_available(), "z3_available should be false");
        status_cache().lock().unwrap().take();
    }

    #[test]
    fn test_get_full_status_report_structure() {
        let _lock = match cache_test_lock().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let canned = json!({
            "sympy": {"available": true, "version": "1.13", "description": "SymPy CAS"},
            "z3": {"available": false, "version": "", "description": "Z3 SMT solver"},
        });
        {
            let mut guard = status_cache().lock().unwrap();
            *guard = Some(StatusCache {
                result: canned,
                expires_at: Instant::now() + std::time::Duration::from_secs(60),
            });
        }
        let report = get_full_status_report();
        assert_eq!(
            report["sympy"]["available"].as_bool(),
            Some(true),
            "sympy available should be true"
        );
        assert_eq!(
            report["z3"]["available"].as_bool(),
            Some(false),
            "z3 available should be false"
        );
        assert!(report["python_backend"].as_bool().unwrap_or(false));
        assert!(report.get("lean").is_some(), "report should have lean key");
        assert!(report.get("sympy").is_some(), "report should have sympy key");
        assert!(report.get("z3").is_some(), "report should have z3 key");
        status_cache().lock().unwrap().take();
    }

    #[test]
    fn test_get_full_status_report_cache_miss() {
        status_cache().lock().unwrap().take();
        let report = get_full_status_report();
        // On cache miss, it probes the real backend; if unavailable,
        // returns python_backend=false
        assert!(report.get("python_backend").is_some());
        assert!(report.get("sympy").is_some());
        assert!(report.get("z3").is_some());
        assert!(report.get("lean").is_some());
    }
}
