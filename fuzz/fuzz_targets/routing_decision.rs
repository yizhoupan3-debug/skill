#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz routing engine host filter with random UTF-8 host IDs and empty records.
///
/// Exercises the host filtering codepath which processes string matching and
/// HashSet lookups — a common source of edge-case panics.
fuzz_target!(|data: &[u8]| {
    if let Ok(host_id) = std::str::from_utf8(data) {
        let records: Vec<routing_engine::route::SkillRecord> = vec![];
        let _ = routing_engine::route::filter_records_for_host(&records, Some(host_id));
    }
});
