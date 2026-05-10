//! 运行时节点的 schema_version / authority 等标识集中于此，避免 main.rs 入口文件继续膨胀。
//! 行为与字符串取值必须与历史版本保持一致（契约测试依赖）。

use std::sync::atomic::AtomicU64;

pub static WRITE_TEXT_PAYLOAD_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub const RUNTIME_CONTROL_PLANE_SCHEMA_VERSION: &str = "router-rs-runtime-control-plane-v1";
pub const RUNTIME_CONTROL_PLANE_AUTHORITY: &str = "rust-runtime-control-plane";
pub const RUNTIME_INTEGRATOR_SCHEMA_VERSION: &str = "router-rs-runtime-integrator-v1";
pub const RUNTIME_INTEGRATOR_AUTHORITY: &str = "rust-runtime-integrator";
pub const SANDBOX_CONTROL_SCHEMA_VERSION: &str = "router-rs-sandbox-control-v1";
pub const SANDBOX_CONTROL_AUTHORITY: &str = "rust-sandbox-control";
pub const SANDBOX_EVENT_SCHEMA_VERSION: &str = "runtime-sandbox-event-v1";
pub const BACKGROUND_CONTROL_SCHEMA_VERSION: &str = "router-rs-background-control-v1";
pub const BACKGROUND_CONTROL_AUTHORITY: &str = "rust-background-control";
pub const TRACE_DESCRIPTOR_SCHEMA_VERSION: &str = "router-rs-trace-descriptor-v1";
pub const TRACE_DESCRIPTOR_AUTHORITY: &str = "rust-runtime-trace-descriptor";
pub const CHECKPOINT_RESUME_MANIFEST_SCHEMA_VERSION: &str =
    "router-rs-checkpoint-resume-manifest-v1";
pub const CHECKPOINT_RESUME_MANIFEST_AUTHORITY: &str = "rust-runtime-checkpoint-manifest";
pub const TRANSPORT_BINDING_WRITE_SCHEMA_VERSION: &str = "router-rs-transport-binding-write-v1";
pub const TRANSPORT_BINDING_WRITE_AUTHORITY: &str = "rust-runtime-transport-binding-writer";
pub const CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION: &str = "router-rs-checkpoint-manifest-write-v1";
pub const CHECKPOINT_MANIFEST_WRITE_AUTHORITY: &str = "rust-runtime-checkpoint-manifest-writer";
pub const RUNTIME_STORAGE_SCHEMA_VERSION: &str = "router-rs-runtime-storage-v1";
pub const RUNTIME_STORAGE_AUTHORITY: &str = "rust-runtime-storage";
pub const ATTACHED_RUNTIME_EVENT_ATTACH_AUTHORITY: &str = "rust-runtime-attached-event-transport";
pub const TRACE_STREAM_REPLAY_SCHEMA_VERSION: &str = "router-rs-trace-stream-replay-v1";
pub const TRACE_STREAM_INSPECT_SCHEMA_VERSION: &str = "router-rs-trace-stream-inspect-v1";
pub const TRACE_COMPACTION_DELTA_WRITE_SCHEMA_VERSION: &str =
    "router-rs-trace-compaction-delta-write-v1";
pub const TRACE_METADATA_WRITE_SCHEMA_VERSION: &str = "router-rs-trace-metadata-write-v1";
pub const TRACE_STREAM_IO_AUTHORITY: &str = "rust-runtime-trace-io";
pub const TRACE_METADATA_WRITE_AUTHORITY: &str = "rust-runtime-trace-metadata-writer";
pub const RUNTIME_OBSERVABILITY_EXPORTER_SCHEMA_VERSION: &str = "runtime-observability-exporter-v1";
pub const RUNTIME_OBSERVABILITY_METRIC_RECORD_SCHEMA_VERSION: &str =
    "runtime-observability-metric-record-v1";
pub const RUNTIME_OBSERVABILITY_METRIC_CATALOG_SCHEMA_VERSION: &str =
    "runtime-observability-metric-catalog-v1";
pub const RUNTIME_OBSERVABILITY_METRIC_CATALOG_VERSION: &str = "runtime-observability-metrics-v1";
pub const RUNTIME_OBSERVABILITY_DASHBOARD_SCHEMA_VERSION: &str =
    "runtime-observability-dashboard-v1";
pub const RUNTIME_OBSERVABILITY_HEALTH_SNAPSHOT_SCHEMA_VERSION: &str =
    "runtime-observability-health-snapshot-v1";
pub const RUNTIME_OBSERVABILITY_SIGNAL_VOCABULARY: &str = "shared-runtime-v1";
pub const DEFAULT_MAX_CONCURRENT_SUBAGENTS: usize = 8;
pub const MAX_CONCURRENT_SUBAGENTS_LIMIT: usize = 24;
pub const DEFAULT_SUBAGENT_TIMEOUT_SECONDS: u64 = 900;
pub const DEFAULT_MAX_BACKGROUND_JOBS: usize = 16;
pub const MAX_BACKGROUND_JOBS_LIMIT: usize = 64;
pub const DEFAULT_BACKGROUND_JOB_TIMEOUT_SECONDS: u64 = 600;
pub const DEFAULT_COMPUTE_THREADS: usize = 0;
pub const MAX_COMPUTE_THREADS: usize = 64;
