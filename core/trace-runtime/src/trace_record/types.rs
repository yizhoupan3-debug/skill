use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{TRACE_EVENT_SCHEMA_VERSION, TRACE_EVENT_SINK_SCHEMA_VERSION};

fn default_true() -> bool {
    true
}

fn default_event_sink_schema_version() -> String {
    TRACE_EVENT_SINK_SCHEMA_VERSION.to_string()
}

fn default_event_schema_version() -> String {
    TRACE_EVENT_SCHEMA_VERSION.to_string()
}

fn default_ok_status() -> String {
    "ok".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecordEventRequestPayload {
    pub path: Option<String>,
    #[serde(default = "default_true")]
    pub write_outputs: bool,
    #[serde(default = "default_event_sink_schema_version")]
    pub sink_schema_version: String,
    #[serde(default = "default_event_schema_version")]
    pub event_schema_version: String,
    pub generation: usize,
    pub seq: usize,
    pub run_id: String,
    pub job_id: Option<String>,
    pub kind: String,
    pub stage: String,
    #[serde(default = "default_ok_status")]
    pub status: String,
    #[serde(default)]
    pub payload: Map<String, Value>,
    pub compaction_manifest_path: Option<String>,
    pub compaction_manifest_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecordEventResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub path: Option<String>,
    pub event: Value,
    pub sink_line: String,
    pub bytes_written: usize,
    pub delta_path: Option<String>,
    pub delta_line: Option<String>,
    pub delta_bytes_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCompactRequestPayload {
    pub root_path: String,
    pub event_stream_path: Option<String>,
    pub output_path: Option<String>,
    pub run_id: String,
    pub job_id: Option<String>,
    pub backend_family: Option<String>,
    #[serde(default = "default_true")]
    pub supports_compaction: bool,
    #[serde(default = "default_true")]
    pub supports_snapshot_delta: bool,
    pub current_generation: usize,
    #[serde(default)]
    pub artifact_paths: Vec<String>,
    pub event_stream_text: Option<String>,
    pub output_text: Option<String>,
    pub previous_manifest_text: Option<String>,
    #[serde(default = "default_true")]
    pub write_outputs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCompactResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub applied: bool,
    pub status: String,
    pub reason: Option<String>,
    pub run_id: String,
    pub job_id: Option<String>,
    pub backend_family: Option<String>,
    pub current_generation: usize,
    pub next_generation: usize,
    pub latest_stable_snapshot: Option<Value>,
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub writes: Vec<TraceTextWrite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceTextWrite {
    pub path: String,
    pub payload_text: String,
}
