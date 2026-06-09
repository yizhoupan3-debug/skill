use router_rs::session_call_tracker::init_tracker;
use serde_json::Value;
use std::io::{BufRead, Write};
use std::path::Path;

use super::dispatch::handle_mcp_request;

pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "router-rs-framework";
pub const SERVER_VERSION: &str = "0.1.0-rust";
const MAX_MCP_CONTENT_LENGTH: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpTransportMode {
    ContentLength,
    NewlineDelimited,
}

pub fn run_mcp_stdio<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    repo_root: &Path,
    host_id: &str,
) -> Result<(), String> {
    // 初始化 session tracker（session 级别，只执行一次）
    // 注意：init_tracker 失败不会阻塞 MCP 服务，因为某些环境可能不支持 tracker 文件
    if let Err(e) = init_tracker(repo_root) {
        eprintln!(
            "[router-rs warning] init_tracker failed: session call tracking may not work. \
             Error: {}. This is non-fatal for MCP operation.",
            e
        );
    }
    let mut transport_mode = None;
    while let Some(message) = read_mcp_message(&mut input, &mut transport_mode)? {
        if let Some(response) = handle_mcp_request(&message, repo_root, host_id) {
            write_mcp_response(
                &mut output,
                transport_mode.unwrap_or(McpTransportMode::NewlineDelimited),
                &response,
            )?;
        }
    }
    Ok(())
}

fn read_mcp_message<R: BufRead>(
    input: &mut R,
    transport_mode: &mut Option<McpTransportMode>,
) -> Result<Option<String>, String> {
    let mut first_line = String::new();
    loop {
        first_line.clear();
        let bytes = input
            .read_line(&mut first_line)
            .map_err(|err| format!("read MCP request failed: {err}"))?;
        if bytes == 0 {
            return Ok(None);
        }
        if !first_line.trim().is_empty() {
            break;
        }
    }

    let lower = first_line.to_ascii_lowercase();
    // HTTP headers may have optional whitespace (OWS) before the colon per RFC 7230
    let has_content_length =
        lower.starts_with("content-length:") || lower.starts_with("content-length :");
    if has_content_length {
        let previous_mode = *transport_mode;
        *transport_mode = Some(McpTransportMode::ContentLength);

        // Log transport mode changes (only on first switch for debugging)
        if previous_mode.is_none() {
            eprintln!("[router-rs info] MCP transport mode: Content-Length");
        }

        let content_length = parse_content_length(&first_line)?;
        if content_length > MAX_MCP_CONTENT_LENGTH {
            return Err(format!(
                "MCP Content-Length {content_length} exceeds max {MAX_MCP_CONTENT_LENGTH}"
            ));
        }
        loop {
            let mut header = String::new();
            let bytes = input
                .read_line(&mut header)
                .map_err(|err| format!("read MCP header failed: {err}"))?;
            if bytes == 0 {
                return Err("MCP header ended before blank line".to_string());
            }
            if header.trim().is_empty() {
                break;
            }
        }
        let mut body = vec![0_u8; content_length];
        input
            .read_exact(&mut body)
            .map_err(|err| format!("read MCP body failed: {err}"))?;
        // Strip UTF-8 BOM if present (some clients send it)
        let body = body.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&body);
        return String::from_utf8(body.to_vec())
            .map(Some)
            .map_err(|err| format!("decode MCP body failed: {err}"));
    }

    // NOTE: 不再锁定传输模式。每次读取都重新检测 Content-Length 头，
    // 允许客户端在会话中切换传输模式（如先发 newline 探测，再切 Content-Length）。
    // NewlineDelimited mode
    let previous_mode = *transport_mode;
    if previous_mode.is_none() {
        eprintln!("[router-rs info] MCP transport mode: NewlineDelimited");
    }
    Ok(Some(first_line.trim_end().to_string()))
}

pub fn parse_content_length(line: &str) -> Result<usize, String> {
    // Handle both "Content-Length:" and "Content-Length :" (OWS)
    // Note: line may contain trailing \r\n from read_line
    let lower = line.to_ascii_lowercase();
    let value_str = if lower.starts_with("content-length :") {
        // Skip "content-length :" (16 chars)
        line[16..].trim()
    } else if lower.starts_with("content-length:") {
        // Skip "content-length:" (15 chars)
        line[15..].trim()
    } else {
        return Err(format!("invalid Content-Length header: {}", line));
    };
    value_str
        .parse::<usize>()
        .map_err(|err| format!("invalid MCP content length '{value_str}': {err}"))
}

fn write_mcp_response<W: Write>(
    output: &mut W,
    transport_mode: McpTransportMode,
    response: &Value,
) -> Result<(), String> {
    let encoded = serde_json::to_string(response)
        .map_err(|err| format!("serialize MCP response failed: {err}"))?;
    match transport_mode {
        McpTransportMode::ContentLength => {
            write!(output, "Content-Length: {}\r\n\r\n{encoded}", encoded.len())
                .map_err(|err| format!("write MCP response failed: {err}"))?;
        }
        McpTransportMode::NewlineDelimited => {
            writeln!(output, "{encoded}")
                .map_err(|err| format!("write MCP response failed: {err}"))?;
        }
    }
    Ok(())
}
