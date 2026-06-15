//! CLI I/O 助手、并行线程池与带锁追加写（manifest 回退已下沉至 `framework_runtime`）。
pub use crate::framework_runtime::{
    manifest_fallback_path, resolve_runtime_declared_manifest_fallback,
    route_task_with_manifest_fallback,
};
// Functions moved to framework_runtime for cycle-breaking (re-exported for backward compat).
pub use crate::framework_runtime::io_utils::{
    append_text_with_process_lock, validate_write_path,
};
pub use crate::framework_runtime::json_io::{parse_json_input, print_json_value};
use crate::runtime_envelope_ids::MAX_COMPUTE_THREADS;
use rayon::ThreadPoolBuilder;

include!("common.inc");
