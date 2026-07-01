//! Lean theorem prover bridge — status check and verification.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.

use crate::types::{VerificationResult, VerificationStatus};
use core_errors::FrameworkError;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Status of the Lean toolchain availability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeanStatus {
    /// Lean 4 is installed and usable.
    Available,
    /// Lean is not found or broken, with diagnostic info.
    NotFound {
        reason: String,
        install_guide: String,
    },
}

impl LeanStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, LeanStatus::Available)
    }
}

// ===========================================================================
// Z3 status checks (via Python bridge)
// ===========================================================================

/// Check if Z3 is available via the Python math backend.
pub fn check_z3_available() -> bool {
    crate::verification::python_bridge::z3_available()
}

/// Check if SymPy is available via the Python math backend.
pub fn check_sympy_available() -> bool {
    crate::verification::python_bridge::sympy_available()
}

/// Get a comprehensive backend status report.
pub fn check_all_backends() -> serde_json::Value {
    // Get Python backend status
    let python_status = crate::verification::python_bridge::get_full_status_report();

    // Get Lean status separately (not via Python)
    let lean_status = check_lean_status();
    let (lean_available, lean_detail) = match &lean_status {
        LeanStatus::Available => (true, "Lean 4 installed".to_string()),
        LeanStatus::NotFound { reason, install_guide } => (false, format!("{reason}. {install_guide}")),
    };

    json!({
        "lean": {
            "available": lean_available,
            "detail": lean_detail,
            "probe_type": "which lean",
        },
        "sympy": python_status.get("sympy"),
        "z3": python_status.get("z3"),
        "python_backend": python_status.get("python_backend"),
    })
}

/// Return a unified string describing all backends' status.
pub fn format_all_backends_status() -> String {
    let status = check_all_backends();

    let sympy = status.pointer("/sympy/available").and_then(|v| v.as_bool()).unwrap_or(false);
    let sympy_ver = status.pointer("/sympy/version").and_then(|v| v.as_str()).unwrap_or("?");
    let z3 = status.pointer("/z3/available").and_then(|v| v.as_bool()).unwrap_or(false);
    let z3_ver = status.pointer("/z3/version").and_then(|v| v.as_str()).unwrap_or("?");
    let lean = status.pointer("/lean/available").and_then(|v| v.as_bool()).unwrap_or(false);
    let py_backend = status.pointer("/python_backend").and_then(|v| v.as_bool()).unwrap_or(false);

    format!(
        "SymPy: {} (v{}), Z3: {} (v{}), Lean: {}, Python backend: {}",
        if sympy { "✅" } else { "❌" },
        sympy_ver,
        if z3 { "✅" } else { "❌" },
        z3_ver,
        if lean { "✅" } else { "❌" },
        if py_backend { "✅" } else { "❌" },
    )
}

/// Check if Lean 4 is available on the system PATH.
///
/// Probes `which lean` and `lean --version`. No caching — per-invocation probe.
pub fn check_lean_status() -> LeanStatus {
    let which = std::process::Command::new("which")
        .arg("lean")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    match which {
        Ok(output) if output.status.success() => {
            // Also check version
            let version = std::process::Command::new("lean")
                .arg("--version")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output();
            match version {
                Ok(v) if v.status.success() => {
                    LeanStatus::Available
                }
                _ => LeanStatus::NotFound {
                    reason: "lean binary found but `lean --version` failed".into(),
                    install_guide: "Run: elan install lean4".into(),
                },
            }
        }
        _ => LeanStatus::NotFound {
            reason: "lean not found on system PATH".into(),
            install_guide: concat!(
                "Install elan (Lean 4 version manager):\n",
                "  curl -L https://github.com/leanprover/elan/releases/download/v4.0.3/elan-x86_64-unknown-linux-gnu.tar.gz | tar xz\n",
                "  ./elan-init\n",
                "Then: elan install lean4\n",
                "Or use the VS Code extension 'lean4'."
            ).into(),
        },
    }
}

/// Attempt to verify a Lean theorem by running `lean` on a script file.
pub fn verify_lean_theorem(script: &str) -> VerificationResult {
    // Check availability first
    if !check_lean_status().is_available() {
        return VerificationResult {
            check_name: "math_lean_verify".into(),
            status: VerificationStatus::Warn,
            details: "Lean not available — install via elan".into(),
            evidence_path: None,
        };
    }

    // Write script to temp file and run lean
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("lean_verify_{nanos:016x}"));
    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
        tracing::warn!("[lean_bridge] failed to create temp dir: {e}");
    }
    let script_path = temp_dir.join("verify.lean");

    // Drop guard ensures cleanup even if the process panics mid-execution.
    struct CleanupGuard(std::path::PathBuf);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _guard = CleanupGuard(temp_dir.clone());

    let result = (|| -> Result<std::process::Output, FrameworkError> {
        core_state_utils::atomic_write::write_atomic_text(&script_path, script)?;
        std::process::Command::new("lean")
            .arg(&script_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .map_err(FrameworkError::Io)
    })();

    // Clean up both file and directory (Drop guard also handles this, but eager
    // cleanup is better — shorter lifetime for temp resources).
    let _ = std::fs::remove_dir_all(&temp_dir);

    let output = match result {
        Ok(o) => o,
        Err(e) => {
            return VerificationResult {
                check_name: "math_lean_verify".into(),
                status: VerificationStatus::Warn,
                details: e.to_string(),
                evidence_path: None,
            };
        }
    };

    if output.status.success() {
        VerificationResult {
            check_name: "math_lean_verify".into(),
            status: VerificationStatus::Pass,
            details: "Lean verified the theorem".into(),
            evidence_path: None,
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        VerificationResult {
            check_name: "math_lean_verify".into(),
            status: VerificationStatus::Fail,
            details: format!("Lean verification failed:\n{stderr}"),
            evidence_path: None,
        }
    }
}

/// Find a Lean 4 repository (presence of `lakefile.lean` or `lean-toolchain`).
pub fn find_lean_repo() -> Option<std::path::PathBuf> {
    // Search from cwd upward for lakefile.lean or lean-toolchain
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            if d.join("lakefile.lean").exists() || d.join("lean-toolchain").exists() {
                return Some(d.to_path_buf());
            }
            dir = d.parent();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: true when Lean 4 is on the system PATH.
    fn lean_is_available() -> bool {
        check_lean_status().is_available()
    }

    #[test]
    fn test_lean_probe_no_panic() {
        let _ = check_lean_status();
    }

    #[test]
    fn test_lean_repo_no_panic() {
        let _ = find_lean_repo();
    }

    #[test]
    fn test_verify_lean_theorem_available() {
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }
        // A valid Lean theorem that relies only on the built-in Prelude.
        let script = "theorem reflexive (a : Nat) : a = a := rfl";
        let result = verify_lean_theorem(script);
        assert_eq!(
            result.status,
            VerificationStatus::Pass,
            "expected Pass for a valid theorem, got {:?}: {}",
            result.status,
            result.details,
        );
        assert_eq!(result.check_name, "math_lean_verify");
    }

    #[test]
    fn test_verify_lean_theorem_failure() {
        if !lean_is_available() {
            eprintln!("Skipping: Lean 4 not available on PATH");
            return;
        }
        // A theorem that type-checks but is not true by rfl -- Lean fails.
        let script = "theorem broken : 1 = 2 := rfl";
        let result = verify_lean_theorem(script);
        assert_eq!(
            result.status,
            VerificationStatus::Fail,
            "expected Fail for an invalid theorem, got {:?}: {}",
            result.status,
            result.details,
        );
        assert_eq!(result.check_name, "math_lean_verify");
        assert!(
            result.details.contains("Lean verification failed"),
            "details should indicate failure, got: {}",
            result.details,
        );
    }

    #[test]
    fn test_verify_lean_theorem_unavailable() {
        if lean_is_available() {
            eprintln!(
                "Skipping: Lean is available on PATH \
                 (cannot exercise the unavailable code path)"
            );
            return;
        }
        let result = verify_lean_theorem("(any content)");
        assert_eq!(
            result.status,
            VerificationStatus::Warn,
            "expected Warn when Lean is unavailable, got {:?}: {}",
            result.status,
            result.details,
        );
        assert_eq!(result.check_name, "math_lean_verify");
        assert!(
            result.details.contains("not available")
                || result.details.contains("install"),
            "details should mention Lean unavailability, got: {}",
            result.details,
        );
    }
}
