//! 路由 manifest 回退、并行线程池与带锁追加写。
use crate::route::{
    literal_framework_alias_decision, load_records_from_manifest, read_json, route_task,
    should_accept_manifest_fallback, should_retry_with_manifest, RouteDecision, SkillRecord,
};
use crate::runtime_envelope_ids::MAX_COMPUTE_THREADS;
use rayon::ThreadPoolBuilder;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

include!("common.inc");
