/// Error type for MCP tool registry operations.
#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum McpToolRegistryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid record: {0}")]
    InvalidRecord(String),

    #[error("Lookup error: {0}")]
    Lookup(String),
}
