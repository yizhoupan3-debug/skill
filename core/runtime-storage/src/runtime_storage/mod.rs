//! Runtime persistence: filesystem, sqlite (thread-local connection cache), and memory backends.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

mod backend;
mod filesystem;
mod operation;
pub mod paths;
mod sqlite;

pub use backend::{
    runtime_backend_capabilities, runtime_backend_family_catalog_payload,
    runtime_backend_family_parity_payload,
};
pub use filesystem::acquire_runtime_path_lock;
pub use operation::{
    build_checkpoint_control_plane_compiler_payload, resolve_storage_backend,
    runtime_storage_operation, storage_artifact_exists, storage_read_text,
};
pub(crate) use sqlite::sqlite_connection;

/// Single source of truth for the durable background-state service identity strings
/// and the SQLite payload table name. `background_state` re-exports these via
/// `use crate::runtime_storage::{...}` so renames cannot drift between the two files.
pub const DEFAULT_STATE_SERVICE_AUTHORITY: &str = "rust-runtime-control-plane";
pub const DEFAULT_STATE_SERVICE_ROLE: &str = "durable-background-state";
pub const DEFAULT_STATE_SERVICE_PROJECTION: &str = "rust-native-projection";
pub const SQLITE_TABLE_NAME: &str = "runtime_storage_payloads";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStorageRequestPayload {
    pub operation: String,
    pub path: String,
    pub backend_family: String,
    pub sqlite_db_path: Option<String>,
    pub storage_root: Option<String>,
    pub payload_text: Option<String>,
    pub expected_sha256: Option<String>,
    pub max_bytes: Option<usize>,
    pub tail_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStorageResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub operation: String,
    pub path: String,
    pub backend_family: String,
    pub sqlite_db_path: Option<String>,
    pub storage_root: Option<String>,
    pub backend_capabilities: Value,
    pub exists: bool,
    pub payload_text: Option<String>,
    pub bytes_written: Option<usize>,
    pub bytes_returned: Option<usize>,
    pub payload_sha256: Option<String>,
    pub verified: Option<bool>,
    pub truncated: Option<bool>,
}

#[derive(Debug, Clone)]
pub enum ResolvedStorageBackend {
    Filesystem,
    Memory,
    Sqlite {
        db_path: PathBuf,
        storage_root: PathBuf,
    },
}

#[cfg(test)]
mod tests;
