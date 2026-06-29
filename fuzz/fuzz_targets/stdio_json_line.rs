#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz the StdioJsonRequestPayload deserialization.
///
/// The internal protocol between framework components uses a JSON envelope:
/// `{ id, op, payload, concurrency }`. This fuzz target inputs random
/// UTF-8 strings directly to serde deserialization, and verifies no
/// panics occur regardless of input.
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _: Result<framework_runtime::types::StdioJsonRequestPayload, _> = serde_json::from_str(s);
    }
});
