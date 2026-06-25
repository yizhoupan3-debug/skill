#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write;

/// Fuzz JSONL maintenance with random bytes.
///
/// Writes random data as a JSONL file and runs compaction to verify
/// the truncation and dedup logic handles arbitrary bytes without panicking.
fuzz_target!(|data: &[u8]| {
    let dir = std::env::temp_dir().join("fuzz-jsonl");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("fuzz.jsonl");
    // Write random bytes as the JSONL file content
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = f.write_all(data);
        let _ = f.flush();
        drop(f);
        // Exercise compaction with a low threshold
        let _ = core_state_utils::jsonl_maintenance::compact_jsonl_if_needed(&path, 5);
        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
});
