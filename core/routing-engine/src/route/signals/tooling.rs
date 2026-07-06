use super::has_signal_by_name;

/// Known tool slugs that users/agents may reference in NL queries.
/// Top-30 most commonly referenced MCP tools — not exhaustive (full registry
/// has 113 tools), but enough to catch the high-signal patterns without
/// introducing a full registry dependency at signal-evaluation time.
const KNOWN_TOOL_SLUGS: &[&str] = &[
    // Meta-routing (note: route_tool MCP was removed in Phase 3)
    "search_tools", "tool_registry_status",
    // PDF / documents
    "pdf_read", "pdf_info", "ooxml_parse", "pptx_parse",
    // Browser
    "browser_screenshot", "browser_open", "browser_click",
    "browser_fill", "browser_get_state",
    // Web / research
    "web_fetch", "financial_data",
    "search_research", "fetch_paper", "find_paper_by_title",
    // Math tools (most frequently invoked)
    "math_sympy_solve", "math_sympy_integrate", "math_sympy_simplify",
    "math_z3_prove", "math_prove_inequality",
    "math_asymptotic_estimate",
    // Goal / task / lifecycle
    "goal_state_manage", "goal_state_read",
    "record_evidence", "closeout_gate",
    // GitHub / code
    "gh_source_gate",
    "codegraph_search", "codegraph_impact",
];

/// 查询含 MCP 工具 FQN 前缀或已知工具 slug，表明明确的工具使用意图。
pub fn has_mcp_tool_invocation_intent(query_text: &str) -> bool {
    // Check 1: MCP FQN prefix (Claude SDK convention: mcp__server__tool)
    if query_text.contains("mcp__") {
        return true;
    }
    // Check 2: Known tool slug appears verbatim in the query (NL tool invocation)
    if KNOWN_TOOL_SLUGS.iter().any(|slug| query_text.contains(slug)) {
        return true;
    }
    false
}

pub fn has_codegraph_index_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("codegraph_index_ready", query_text, query_token_list)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn mcp_fqn_prefix_detected() {
        assert!(has_mcp_tool_invocation_intent("mcp__browser_mcp__browser_screenshot"));
        assert!(has_mcp_tool_invocation_intent("调用 mcp__server__tool 来操作"));
    }

    #[test]
    fn known_tool_slug_meta_routing() {
        assert!(has_mcp_tool_invocation_intent("调用 search_tools 搜索工具"));
        assert!(has_mcp_tool_invocation_intent("查 tool_registry_status"));
    }

    #[test]
    fn known_tool_slug_pdf_docs() {
        assert!(has_mcp_tool_invocation_intent("调用 pdf_read 解析文件"));
        assert!(has_mcp_tool_invocation_intent("用 pdf_info 看元数据"));
    }

    #[test]
    fn known_tool_slug_browser() {
        assert!(has_mcp_tool_invocation_intent("帮我截图 browser_screenshot"));
        assert!(has_mcp_tool_invocation_intent("用 browser_open 打开页面"));
    }

    #[test]
    fn known_tool_slug_web_research() {
        assert!(has_mcp_tool_invocation_intent("调用 web_fetch 抓取"));
        assert!(has_mcp_tool_invocation_intent("用 financial_data 查股票"));
    }

    #[test]
    fn known_tool_slug_math() {
        assert!(has_mcp_tool_invocation_intent("用 math_sympy_solve 解方程"));
        assert!(has_mcp_tool_invocation_intent("调用 math_z3_prove 验证"));
        assert!(has_mcp_tool_invocation_intent("帮我 math_prove_inequality"));
    }

    #[test]
    fn known_tool_slug_goal_task() {
        assert!(has_mcp_tool_invocation_intent("goal_state_manage 创建 goal"));
        assert!(has_mcp_tool_invocation_intent("record_evidence 记录证据"));
    }

    #[test]
    fn no_match_for_skill_query() {
        assert!(!has_mcp_tool_invocation_intent("帮我改论文"));
        assert!(!has_mcp_tool_invocation_intent("这篇代码需要优化"));
        assert!(!has_mcp_tool_invocation_intent("画一个架构图"));
    }

    #[test]
    fn empty_string_returns_false() {
        assert!(!has_mcp_tool_invocation_intent(""));
        assert!(!has_mcp_tool_invocation_intent(" "));
    }

    #[test]
    fn slug_as_substring_does_not_false_positive() {
        // "codegraph" is not a full slug match; only "codegraph_search" is
        assert!(!has_mcp_tool_invocation_intent("帮我画一个架构图 codegraph"));
        // "goal" is not "goal_state_manage" or "goal_state_read"
        assert!(!has_mcp_tool_invocation_intent("我的 goal 是什么"));
    }

    #[test]
    fn contains_matches_embedded_slug() {
        // Slug embedded within query still matched by contains()
        assert!(has_mcp_tool_invocation_intent("请调用search_tools功能"));
        assert!(has_mcp_tool_invocation_intent("帮我browser_screenshot一下"));
    }
}
