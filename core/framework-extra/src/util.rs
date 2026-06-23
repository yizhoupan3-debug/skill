//! Shared utility functions — re-exports from `fr_utils::util`.
//!
//! This is the single source of truth for these utility functions.
//! All implementations live in `framework_runtime/src/util.rs`.
//! This module re-exports them so that `framework_extra::util::*` callers
//! continue to work unchanged.

// ── Re-exports (canonical implementations in framework-runtime) ──

pub use fr_utils::util::{
    count_evidence_rows,
    current_local_timestamp,
    defaulted_payload_text,
    hash_file_for_test,
    is_terminal,
    normalize_task_registry_rows,
    parse_session_summary,
    registry_rows_from_payload,
    required_payload_text,
    supervisor_contract,
    truncate_utf8_chars,
    write_json_if_changed_unlocked,
    write_text_if_changed_unlocked,
};
