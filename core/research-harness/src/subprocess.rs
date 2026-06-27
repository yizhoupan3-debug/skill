//! Shared subprocess utilities for Python bridge calls.
//!
//! Provides a timeout-guarded `uv run -m <module>` runner with stderr capture.
//! All Python bridge calls in the verification modules funnel through here.
//!
//! # Security
//!
//! User input NEVER reaches shell arguments. All data arrives via JSON on stdin.
//! The only shell-adjacent call is `uv run -m <module>` where `<module>` is a
//! compile-time constant (inequality_solver, asymptotic_solver), never user input.

use core_errors::FrameworkError;
use serde_json::Value;
use std::io::Write;
use std::sync::mpsc;
use std::time::Duration;

/// Default timeout for all Python subprocess calls (15 seconds).
const DEFAULT_TIMEOUT_MS: u64 = 15_000;

/// Poll interval for `try_wait` loop. 50ms is negligible vs 15s default timeout.
const POLL_INTERVAL_MS: u64 = 50;

/// Run `uv run -m <module> --stdin-json` with JSON input, default timeout.
pub fn run_uv_module(module: &str, input: &Value) -> Result<Value, FrameworkError> {
    run_uv_module_with_timeout(module, input, DEFAULT_TIMEOUT_MS)
}

/// Run `uv run -m <module> --stdin-json` with JSON input and custom timeout.
///
/// Writes `input` as JSON to the subprocess stdin, reads a JSON object from stdout,
/// and captures stderr for error diagnostics.
///
/// # Safety
///
/// No `unsafe` code. Uses `std::process::Child::kill()` (safe libstd wrapper)
/// instead of raw `libc::kill()` to avoid PID-reuse races on timeout.
pub fn run_uv_module_with_timeout(
    module: &str,
    input: &Value,
    timeout_ms: u64,
) -> Result<Value, FrameworkError> {
    let input_str = serde_json::to_string(input)?;

    let mut child = std::process::Command::new("uv")
        .args(["run", "-m", module, "--stdin-json"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let pid = child.id();

    // Write stdin and close the pipe (drop stdin handle) to signal EOF to Python
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_str.as_bytes())?;
    }

    // Background thread: fires a signal once the deadline passes.
    // Used instead of polling `Instant::now()` to avoid time-warp issues and
    // keep the hot path simple (just check channel emptiness).
    let (deadline_tx, deadline_rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(timeout_ms));
        let _ = deadline_tx.send(());
    });

    // Poll loop using `try_wait` (non-blocking, no unsafe).
    // On process exit: collect buffered stdout/stderr via `wait_with_output`.
    // On timeout: `child.kill()` is the safe libstd cross-platform wrapper.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited — collect remaining pipe data.
                let output = child
                    .wait_with_output()?;

                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if !status.success() {
                    // Non-zero exit: check if stdout has an {"error": ...} JSON.
                    if let Ok(val) = serde_json::from_slice::<Value>(&output.stdout) {
                        if val.get("error").is_some() {
                            return Ok(val);
                        }
                    }
                    let detail = truncate(&stderr, 500);
                    return Err(FrameworkError::validation(format!(
                        "uv {} failed (exit={:?}): {}",
                        module,
                        output.status.code(),
                        detail
                    )));
                }

                return serde_json::from_slice(&output.stdout).map_err(|e| {
                    FrameworkError::validation(format!(
                        "parse {} output: {e}, stderr: {}",
                        module,
                        truncate(&stderr, 200)
                    ))
                });
            }
            Ok(None) => {
                // Child still running — check deadline.
                if deadline_rx.try_recv().is_ok() {
                    // Timeout: kill safely via libstd (cross-platform, no unsafe).
                    let _ = child.kill();
                    // Reap zombie.
                    let _ = child.wait();
                    return Err(FrameworkError::validation(format!(
                        "subprocess timed out after {timeout_ms}ms (pid={pid})"
                    )));
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(e) => return Err(FrameworkError::Io(e)),
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        let mut t = s[..max].to_string();
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
}
