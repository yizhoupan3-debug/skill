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

use serde_json::Value;
use std::io::Write;
use std::sync::mpsc;
use std::time::Duration;

/// Default timeout for all Python subprocess calls (15 seconds).
const DEFAULT_TIMEOUT_MS: u64 = 15_000;

/// Run `uv run -m <module> --stdin-json` with JSON input, default timeout.
pub fn run_uv_module(module: &str, input: &Value) -> Result<Value, String> {
    run_uv_module_with_timeout(module, input, DEFAULT_TIMEOUT_MS)
}

/// Run `uv run -m <module> --stdin-json` with JSON input and custom timeout.
///
/// Writes `input` as JSON to the subprocess stdin, reads a JSON object from stdout,
/// and captures stderr for error diagnostics.
pub fn run_uv_module_with_timeout(
    module: &str,
    input: &Value,
    timeout_ms: u64,
) -> Result<Value, String> {
    let input_str = serde_json::to_string(input).map_err(|e| format!("serialize: {e}"))?;

    let mut child = std::process::Command::new("uv")
        .args(["run", "-m", module, "--stdin-json"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| format!("failed to spawn uv '{module}' — is uv installed?"))?;

    let pid = child.id();

    // Write stdin and close the pipe (drop stdin handle) to signal EOF to Python
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_str.as_bytes())
            .map_err(|e| format!("stdin write: {e}"))?;
    }

    // Thread-based timeout guard
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = child.wait_with_output();
        tx.send(result).ok();
    });

    let timeout = Duration::from_millis(timeout_ms);
    let output = match rx.recv_timeout(timeout) {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(format!("subprocess wait: {e}")),
        Err(_) => {
            // Timeout — best-effort kill via OS command
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
            return Err(format!("subprocess timed out after {timeout_ms}ms (pid={pid})"));
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        // The Python script may print an error JSON on stdout even on exit code 0
        // (error field in the response). On non-zero exit, try parsing stdout first.
        if let Ok(val) = serde_json::from_slice::<Value>(&output.stdout) {
            if val.get("error").is_some() {
                return Ok(val); // Let caller handle the {"error": ...} response
            }
        }
        let detail = truncate(&stderr, 500);
        return Err(format!(
            "uv {} failed (exit={:?}): {}",
            module,
            output.status.code(),
            detail
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|e| {
        format!(
            "parse {} output: {e}, stderr: {}",
            module,
            truncate(&stderr, 200)
        )
    })
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
