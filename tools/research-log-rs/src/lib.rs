pub mod cli;
pub mod db;
pub mod models;
pub mod text_layer;

use anyhow::Result;
use std::path::Path;

pub const ARTIFACTS_LOG_DIR: &str = "artifacts/research-log";

/// Initialize the log workspace structure: create directory tree and DB.
pub fn init_log_workspace(log_root: &Path) -> Result<()> {
    text_layer::ensure_log_dirs(log_root)?;
    let db_path = log_root.join("research-log.db");
    db::init_database(&db_path)?;
    Ok(())
}
