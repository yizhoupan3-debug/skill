//! CLI I/O 助手、并行线程池与带锁追加写（manifest 回退已下沉至 `framework_runtime`）。
pub use crate::framework_runtime::{
    manifest_fallback_path, resolve_runtime_declared_manifest_fallback,
    route_task_with_manifest_fallback,
};
use crate::runtime_envelope_ids::MAX_COMPUTE_THREADS;
use rayon::ThreadPoolBuilder;
use serde::Serialize;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

include!("common.inc");
