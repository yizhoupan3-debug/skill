use super::*;
use crate::integration_test_prelude::*;
use clap::Parser;

use serde_json::{Map, Value, json};

use crate::route::RouteDecision;
use crate::route::{
    ROUTE_POLICY_SCHEMA_VERSION, evaluate_routing_cases,
    load_records_cached_for_stdio_with_default_runtime_path, load_records_from_manifest,
    load_routing_eval_cases, read_json, value_to_string,
};
use std::collections::HashSet;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{sleep, spawn};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[path = "main_tests/common.rs"]
pub mod common;
#[path = "main_tests/background_state_tests.rs"]
mod background_state_tests;
#[path = "main_tests/cli_hooks_tests.rs"]
mod cli_hooks_tests;
#[path = "main_tests/closeout_tests.rs"]
mod closeout_tests;
#[path = "main_tests/execution_tests.rs"]
mod execution_tests;
#[path = "main_tests/framework_runtime_tests.rs"]
mod framework_runtime_tests;
#[path = "main_tests/routing_tests.rs"]
mod routing_tests;
#[path = "main_tests/sandbox_tests.rs"]
mod sandbox_tests;
#[path = "main_tests/storage_tests.rs"]
mod storage_tests;
#[path = "main_tests/trace_tests.rs"]
mod trace_tests;
