use router_rs::framework_error::HookExitExt;
use router_rs::hook_common::{parse_stdin_json_trimmed, read_limited_stdin};
use serde_json::Value;
use std::io::Read;

pub fn read_stdin_json_from_reader<R: Read>(reader: &mut R) -> Result<Value, String> {
    read_stdin_json_from_reader_inner(reader).map_hook_exit()
}

fn read_stdin_json_from_reader_inner<R: Read>(
    reader: &mut R,
) -> framework_core::error::Result<Value> {
    let buf = read_limited_stdin(reader)?;
    let value = parse_stdin_json_trimmed(buf.trim())?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(framework_core::error::FrameworkError::validation(
            "stdin_json_not_object",
        ))
    }
}

pub fn read_cursor_hook_stdin_json() -> Result<Value, String> {
    let mut stdin = std::io::stdin();
    read_stdin_json_from_reader(&mut stdin)
}
