use std::path::PathBuf;

/// Error type for trace-runtime operations.
#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Lock error: {0}")]
    Lock(String),

    #[error("Poisoned lock: {0}")]
    Poisoned(String),

    #[error("Path error: {path}")]
    Path { path: PathBuf },

    #[error("Validation error: {message}")]
    Validation { message: String },
}
