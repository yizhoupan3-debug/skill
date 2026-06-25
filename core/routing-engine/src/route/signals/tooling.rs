use super::has_signal_by_name;

pub fn has_codegraph_index_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("codegraph_index_ready", query_text, query_token_list)
}

/// 查询含 MCP 工具 FQN 前缀，表明明确的工具使用意图。
///
/// 用于 NL 路由 suppress 规则，防止工具调用意图被误路由到 skill。
pub fn has_mcp_tool_invocation_intent(query_text: &str) -> bool {
    query_text.contains("mcp__")
}
