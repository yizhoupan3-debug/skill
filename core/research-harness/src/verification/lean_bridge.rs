//! Lean theorem prover bridge — status check and verification.
//!
//! FEATURE layer only. MCP dispatch belongs in `mcp_tools.rs`.

use crate::types::{VerificationResult, VerificationStatus};
use core_errors::FrameworkError;
use serde::{Deserialize, Serialize};

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

    #[test]
    fn test_lean_probe_no_panic() {
        let _ = check_lean_status();
    }

    #[test]
    fn test_lean_repo_no_panic() {
        let _ = find_lean_repo();
    }
}
