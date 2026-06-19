use super::has_signal_by_name;
use std::sync::OnceLock;
use regex::Regex;

pub fn has_codegraph_index_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("codegraph_index_ready", query_text, query_token_list)
}

/// 查询含 MCP 工具 FQN 或明确工具使用意图。
///
/// 检测：`mcp__` 前缀、"use/invoke/call/run + mcp/tool/browser/paperplain/codegraph" 模式。
/// 用于 NL 路由 suppress 规则，防止工具调用意图被误路由到 skill。
pub fn has_mcp_tool_invocation_intent(query_text: &str, _query_token_list: &[String]) -> bool {
    if query_text.contains("mcp__") {
        return true;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(use|invoke|call|run|打开|调用|使用)\b.*\b(mcp|tool|browser|paperplain|codegraph|浏览器)\b")
            .expect("invalid mcp tool intent regex")
    })
    .is_match(query_text)
}
