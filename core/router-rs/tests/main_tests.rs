use super::*;

use crate::route::RouteDecision;
use crate::route::{
    ROUTE_POLICY_SCHEMA_VERSION, evaluate_routing_cases,
    load_records_cached_for_stdio_with_default_runtime_path, load_routing_eval_cases, read_json,
    value_to_string,
};

#[path = "main_tests/alias_snapshot_tests.rs"]
mod alias_snapshot_tests;
#[path = "main_tests/artifact_write_tests.rs"]
mod artifact_write_tests;
#[path = "main_tests/background_state_tests.rs"]
mod background_state_tests;
#[path = "main_tests/closeout_tests.rs"]
mod closeout_tests;
#[path = "main_tests/common.rs"]
pub mod common;
#[path = "main_tests/evidence_tests.rs"]
mod evidence_tests;
#[path = "main_tests/execution_tests.rs"]
mod execution_tests;
#[path = "main_tests/framework_runtime_tests.rs"]
mod framework_runtime_tests;
#[path = "main_tests/routing_tests.rs"]
mod routing_tests;
#[path = "main_tests/storage_tests.rs"]
mod storage_tests;
#[path = "main_tests/trace_tests.rs"]
mod trace_tests;
