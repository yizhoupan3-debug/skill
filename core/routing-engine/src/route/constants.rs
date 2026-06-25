pub const ROUTE_DECISION_SCHEMA_VERSION: &str = "router-rs-route-decision-v1";
pub const SEARCH_RESULTS_SCHEMA_VERSION: &str = "router-rs-search-results-v1";
pub const ROUTE_POLICY_SCHEMA_VERSION: &str = "router-rs-route-policy-v1";
pub const ROUTE_SNAPSHOT_SCHEMA_VERSION: &str = "router-rs-route-snapshot-v1";
pub const ROUTE_REPORT_SCHEMA_VERSION: &str = "router-rs-route-report-v2";
pub const ROUTE_RESOLUTION_SCHEMA_VERSION: &str = "router-rs-route-resolution-v1";
pub const ROUTE_AUTHORITY: &str = "rust-route-core";
pub const PROFILE_COMPILE_AUTHORITY: &str = "rust-route-compiler";

pub(super) const NO_SKILL_SELECTED: &str = "none";

pub(super) const PARALLEL_RECORD_SCAN_MIN: usize = 48;

/// Max distinct `runtime_path` entries kept for `load_records_cached_for_stdio`.
/// Long-lived stdio routers otherwise grow without bound when callers rotate paths.
/// Test builds use a tiny cap so eviction is covered without allocating dozens of fixtures.
pub(super) const RECORDS_CACHE_MAX_KEYS: usize = if cfg!(test) { 4 } else { 64 };

pub(super) const PARALLEL_EVAL_CASE_MIN: usize = 8;

/// Const string matching `skill_layer::frontmatter::RecordKind::FrameworkCommand.as_str()`.
///
/// Used in routing decisions (e.g. `filter_records_by_host`) instead of a
/// hardcoded string literal. A `#[cfg(test)]` test below verifies compile-time
/// consistency with the enum definition.
pub(super) const FRAMEWORK_COMMAND_KIND: &str = "framework_command";

#[cfg(test)]
mod tests {
    use super::FRAMEWORK_COMMAND_KIND;

    #[test]
    fn framework_command_kind_matches_record_kind_enum() {
        let expected = skill_layer::frontmatter::RecordKind::FrameworkCommand.as_str();
        assert_eq!(
            FRAMEWORK_COMMAND_KIND, expected,
            "FRAMEWORK_COMMAND_KIND must stay in sync with \
             skill_layer::frontmatter::RecordKind::FrameworkCommand.as_str()"
        );
    }
}
