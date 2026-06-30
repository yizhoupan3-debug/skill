//! L1 observability infrastructure.
//!
//! A thin configuration layer over `tracing` + `tracing-subscriber`:
//!
//! - **Unified subscriber init** — stderr and/or rolling file output,
//!   controlled by `RUST_LOG` or a sensible default.
//! - **Worker log management** — path resolution and TTL-based cleanup
//!   for sub‑process worker logs (extracted from `session-supervisor`).
//! - **Panic hook** — captures panics via `tracing::error!` so they
//!   appear in the structured log output.

mod hooks;
mod rotation;
mod subscriber;

pub use hooks::install_panic_hook;
pub use rotation::{
    cleanup_stale_logs, sanitize, worker_log_path, WorkerLogManager,
};
pub use subscriber::{init, ObservabilityConfig};

/// Minimal error type for init failures (I/O errors creating log directories, etc.).
#[derive(Debug, thiserror::Error)]
pub enum ObservabilityError {
    /// I/O error (e.g. cannot create log directory).
    #[error("observability I/O error: {0}")]
    Io(#[from] std::io::Error),
}
