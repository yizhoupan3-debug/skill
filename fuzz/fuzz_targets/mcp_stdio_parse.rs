#![no_main]

use std::io::BufReader;

use libfuzzer_sys::fuzz_target;

/// Fuzz the MCP stdio message parser.
///
/// The `read_mcp_message` function handles two transport modes:
/// - Content-Length: header+body protocol (RFC-like)
/// - Newline-delimited JSON
///
/// This fuzz target feeds random bytes as stdin and verifies the parser
/// never crashes (panics) regardless of input.
fuzz_target!(|data: &[u8]| {
    let mut input = BufReader::new(data);
    let mut transport = None;
    // read_mcp_message_test_helper is #[cfg(feature = "test-support")]
    let _ = host_projection::hosts::mcp_stdio_harness::read_mcp_message_test_helper(
        &mut input,
        &mut transport,
    );
});
