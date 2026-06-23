//! Shared test helpers for core-state crate.

use std::path::PathBuf;

/// Create a unique temporary directory path for tests.
pub fn unique_repo(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "router-rs-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}
