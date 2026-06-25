//! Error types for the core-state crate.

/// Error type for state management operations.
///
/// All variants implement [`From`] via `#[from]` for automatic conversion
/// from the underlying error types where applicable.
#[derive(Debug, thiserror::Error)]
#[must_use]
pub enum StateError {
    /// IO error during state file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Requested task was not found.
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// State invariant violated.
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Lock contention or lock acquisition failure.
    #[error("Lock error: {0}")]
    Lock(String),
}
