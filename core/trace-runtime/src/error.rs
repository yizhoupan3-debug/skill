/// Error type for trace-runtime operations.
#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Poisoned lock: {0}")]
    Poisoned(String),

    #[error("Validation error: {message}")]
    Validation { message: String },
}

impl TraceError {
    /// Construct a validation error with a descriptive message.
    pub fn validation(message: impl Into<String>) -> Self {
        TraceError::Validation {
            message: message.into(),
        }
    }
}

impl From<String> for TraceError {
    fn from(message: String) -> Self {
        TraceError::Validation { message }
    }
}

impl From<&str> for TraceError {
    fn from(message: &str) -> Self {
        TraceError::Validation {
            message: message.to_string(),
        }
    }
}
