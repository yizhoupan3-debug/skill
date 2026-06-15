//! runtime-storage: extracted storage modules from runtime-core.
//!
//! Provides runtime envelope IDs and the runtime storage backend (filesystem + SQLite).

pub mod runtime_envelope_ids;
pub mod runtime_storage;

pub mod background_state;

// Re-export runtime_storage items at the crate root for background_state convenience.
pub use runtime_storage::{
    DEFAULT_STATE_SERVICE_AUTHORITY, DEFAULT_STATE_SERVICE_PROJECTION, DEFAULT_STATE_SERVICE_ROLE,
    SQLITE_TABLE_NAME, acquire_runtime_path_lock, runtime_backend_capabilities,
};
