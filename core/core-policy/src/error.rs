use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum FrameworkError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config error: {message}")]
    Config { message: String },

    #[error("Registry error: {message}")]
    Registry { message: String },

    #[error("Hook error: {message}")]
    Hook { message: String },

    #[error("MCP error: {message}")]
    Mcp { message: String },

    #[error("Session error: {message}")]
    Session { message: String },

    #[error("Path error: {path}")]
    Path { path: PathBuf },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Not found: {what}")]
    NotFound { what: String },

    #[error("Unsupported: {what}")]
    Unsupported { what: String },
}

impl FrameworkError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config { message: message.into() }
    }

    pub fn registry(message: impl Into<String>) -> Self {
        Self::Registry { message: message.into() }
    }

    pub fn hook(message: impl Into<String>) -> Self {
        Self::Hook { message: message.into() }
    }

    pub fn mcp(message: impl Into<String>) -> Self {
        Self::Mcp { message: message.into() }
    }

    pub fn session(message: impl Into<String>) -> Self {
        Self::Session { message: message.into() }
    }

    pub fn path(path: impl Into<PathBuf>) -> Self {
        Self::Path { path: path.into() }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation { message: message.into() }
    }

    pub fn not_found(what: impl Into<String>) -> Self {
        Self::NotFound { what: what.into() }
    }

    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported { what: what.into() }
    }
}

impl From<FrameworkError> for String {
    fn from(e: FrameworkError) -> Self {
        e.to_string()
    }
}

impl From<String> for FrameworkError {
    fn from(message: String) -> Self {
        Self::Validation { message }
    }
}
