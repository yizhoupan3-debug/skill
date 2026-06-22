//! CLI I/O 助手、并行线程池与带锁追加写（manifest 回退已下沉至 `framework_runtime`）。
use runtime_core::runtime_envelope_ids::MAX_COMPUTE_THREADS;
use rayon::ThreadPoolBuilder;

include!("common.inc");
