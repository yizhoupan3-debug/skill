#![no_main]

use libfuzzer_sys::fuzz_target;

/// Fuzz the hook stdin JSON parser.
///
/// Hook calls from IDE send a JSON payload over stdin (up to 4 MB).
/// This fuzz target tests `serde_json::from_str::<Value>` with random
/// UTF-8 inputs to verify no panics in the initial parse layer.
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _: Result<serde_json::Value, _> = serde_json::from_str(s);
    }
});
