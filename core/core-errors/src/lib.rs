#![deny(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

/// Top-level error type for framework operations.
/// Each variant represents a distinct error domain (I/O, config, hook,
/// session, etc.) with a human-readable message via `Display`.
#[derive(Debug, thiserror::Error)]
pub enum FrameworkError {
    /// Wraps a standard I/O error (file read/write, network, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Wraps a JSON serialization or deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A framework configuration error (invalid setting, missing key, etc.).
    #[error("Config error: {message}")]
    Config { message: String },

    /// A provider-registry error (invalid entry, lookup failure, etc.).
    #[error("Registry error: {message}")]
    Registry { message: String },

    /// A host-hook execution error (registration, dispatch, or runtime failure).
    #[error("Hook error: {message}")]
    Hook { message: String },

    /// A session-management error (launch, termination, or state error).
    #[error("Session error: {message}")]
    Session { message: String },

    /// A path-related error (missing, invalid, or unresolvable path).
    #[error("Path error: {path}")]
    Path { path: PathBuf },

    /// A validation error (invalid input, constraint violation, etc.).
    #[error("Validation error: {message}")]
    Validation { message: String },

    /// The requested resource or entity was not found.
    #[error("Not found: {what}")]
    NotFound { what: String },

    /// An unsupported operation, feature, or value was requested.
    #[error("Unsupported: {what}")]
    Unsupported { what: String },

    /// A lock acquisition error (mutex poison, file lock contention, etc.).
    #[error("Lock error: {message}")]
    Lock { message: String },
}

impl FrameworkError {
    /// Create a `Config` error from a message string.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a `Registry` error from a message string.
    pub fn registry(message: impl Into<String>) -> Self {
        Self::Registry {
            message: message.into(),
        }
    }

    /// Create a `Hook` error from a message string.
    pub fn hook(message: impl Into<String>) -> Self {
        Self::Hook {
            message: message.into(),
        }
    }

    /// Create a `Session` error from a message string.
    pub fn session(message: impl Into<String>) -> Self {
        Self::Session {
            message: message.into(),
        }
    }

    /// Create a `Path` error from a path-like value.
    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path { path: path.into() }
    }

    /// Create a `Validation` error from a message string.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Create a `NotFound` error describing what was not found.
    pub fn not_found(what: impl Into<String>) -> Self {
        Self::NotFound { what: what.into() }
    }

    /// Create an `Unsupported` error describing what is unsupported.
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported { what: what.into() }
    }

    /// Create a `Lock` error from a message string.
    pub fn lock(message: impl Into<String>) -> Self {
        Self::Lock {
            message: message.into(),
        }
    }
}

impl From<FrameworkError> for String {
    fn from(e: FrameworkError) -> Self {
        e.to_string()
    }
}

#[allow(useless_deprecated)]
#[deprecated(note = "prefer explicit FrameworkError variant over From<String>")]
impl From<String> for FrameworkError {
    fn from(message: String) -> Self {
        Self::Validation { message }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    #[test]
    fn smoke() {
        assert!(true);
    }
}
