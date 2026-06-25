//! Verification pipelines: literature, statistical, prose QC, structure, formal.

pub mod asymptotic;
pub mod formal;
pub mod inequality;
pub mod lean_bridge;
pub mod literature;
pub mod prose_qc;
pub mod statistical;
pub mod structure;
pub mod sympy_bridge;

/// Canonical SymPy availability probe.
/// All modules should call this rather than duplicating the probe logic.
pub fn sympy_available() -> bool {
    std::process::Command::new("uv")
        .args(["run", "python", "-c", "import sympy; print('ok')"])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
        .map(|s| s.success()).unwrap_or(false)
}
