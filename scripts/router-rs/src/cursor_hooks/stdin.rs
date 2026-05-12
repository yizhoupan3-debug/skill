use serde_json::{json, Value};
use std::io::Read;

pub(crate) fn read_stdin_json_from_reader<R: Read>(reader: &mut R) -> Result<Value, String> {
    const MAX_STDIN_BYTES: u64 = 4 * 1024 * 1024;
    let mut buf = String::new();
    reader
        .by_ref()
        .take(MAX_STDIN_BYTES)
        .read_to_string(&mut buf)
        .map_err(|e| e.to_string())?;
    let mut probe = [0_u8; 1];
    let overflow = reader.read(&mut probe).map_err(|e| e.to_string())?;
    if overflow > 0 {
        return Err("stdin_too_large".to_string());
    }
    if buf.trim().is_empty() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_str(&buf).map_err(|_| "stdin_json_invalid".to_string())?;
    if value.is_object() {
        Ok(value)
    } else {
        Ok(json!({}))
    }
}

pub(crate) fn read_cursor_hook_stdin_json() -> Result<Value, String> {
    let mut stdin = std::io::stdin();
    read_stdin_json_from_reader(&mut stdin)
}
