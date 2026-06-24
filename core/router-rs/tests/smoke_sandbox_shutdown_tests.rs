//! sandbox close path drain -> cleanup -> recycled (registry contract).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use framework_extra::orchestration_controller::build_runtime_control_plane_payload;
use fr_exec::sandbox_control::build_sandbox_control_response;
use runtime_core::framework_runtime::stdio_dispatch::dispatch_stdio_json_request;
use crate::runtime_envelope_ids::SANDBOX_CONTROL_SCHEMA_VERSION;
use crate::session_supervisor::handle_session_supervisor_operation;
use crate::stdio_payload_types::SandboxControlRequestPayload;

fn temp_supervisor_state_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("router-rs-smoke-{name}-{nonce}.json"))
}
