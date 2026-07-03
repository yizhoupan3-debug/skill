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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecordEventResponsePayload {
    pub schema_version: String,
    pub authority: String,
    pub path: Option<String>,
    pub event: Value,
    pub sink_line: String,
    pub bytes_written: usize,
}
