//! cross-host closeout + evidence write consistency (registry-driven).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::closeout_enforcement::{
    CLOSEOUT_ENFORCEMENT_AUTHORITY, CLOSEOUT_RECORD_SCHEMA_VERSION, closeout_enforcement_contract,
    evaluate_closeout_record_value,
};
use crate::framework_host_targets::host_targets_supported_host_ids;
use runtime_core::framework_runtime::stdio_dispatch::dispatch_stdio_json_request;
use fr_utils::constants::FRAMEWORK_SESSION_ARTIFACT_WRITE_AUTHORITY;
use framework_extra::evidence::framework_hook_evidence_append;
use framework_extra::session_artifacts::write_framework_session_artifacts;
use crate::hook_event_routing::{
    HOOK_EVENT_ROUTING_AUTHORITY, HOOK_EVENT_ROUTING_SCHEMA_VERSION, canonical_hook_event,
    hook_event_routing_contract, routable_lifecycle_events,
};
use crate::hosts::host_provider_for_id;
use crate::runtime_registry::load_runtime_registry_json;
