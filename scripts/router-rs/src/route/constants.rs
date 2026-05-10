pub(crate) const ROUTE_DECISION_SCHEMA_VERSION: &str = "router-rs-route-decision-v1";
pub(crate) const SEARCH_RESULTS_SCHEMA_VERSION: &str = "router-rs-search-results-v1";
pub(crate) const ROUTE_POLICY_SCHEMA_VERSION: &str = "router-rs-route-policy-v1";
pub(crate) const ROUTE_SNAPSHOT_SCHEMA_VERSION: &str = "router-rs-route-snapshot-v1";
pub(crate) const ROUTE_REPORT_SCHEMA_VERSION: &str = "router-rs-route-report-v2";
pub(crate) const ROUTE_RESOLUTION_SCHEMA_VERSION: &str = "router-rs-route-resolution-v1";
pub(crate) const ROUTE_AUTHORITY: &str = "rust-route-core";
pub(crate) const PROFILE_COMPILE_AUTHORITY: &str = "rust-route-compiler";

pub(super) const NO_SKILL_SELECTED: &str = "none";

pub(super) const ARTIFACT_GATE_PHRASES: [&str; 16] = [
    "pdf",
    "docx",
    "xlsx",
    "ppt",
    "pptx",
    "excel",
    "spreadsheet",
    "word 文档",
    "word 文件",
    "表格",
    "工作簿",
    "幻灯片",
    "演示文稿",
    "presentation",
    "deck",
    "slide deck",
];

pub(super) const PARALLEL_RECORD_SCAN_MIN: usize = 48;

/// Max distinct `(runtime_path, manifest_path)` entries kept for `load_records_cached_for_stdio`.
/// Long-lived stdio routers otherwise grow without bound when callers rotate paths.
/// Test builds use a tiny cap so eviction is covered without allocating dozens of fixtures.
pub(super) const RECORDS_CACHE_MAX_KEYS: usize = if cfg!(test) { 4 } else { 64 };

#[cfg(test)]
pub(super) const PARALLEL_EVAL_CASE_MIN: usize = 8;
