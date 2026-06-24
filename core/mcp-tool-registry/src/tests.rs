#[cfg(test)]
mod routing_tests {
    use crate::tool_types::McpToolRecord;
    use std::collections::HashSet;

    fn test_record(slug: &str, display: &str, keywords: &[&str]) -> McpToolRecord {
        let mut record = McpToolRecord {
            slug: slug.to_string(),
            display_name: display.to_string(),
            description: format!("Tool: {display}"),
            layer: "builtin".to_string(),
            dispatch_domain: "composite".to_string(),
            owner: "framework".to_string(),
            gate: "none".to_string(),
            trigger_hints: keywords.iter().map(|s| s.to_string()).collect(),
            name_tokens: HashSet::new(),
            keyword_tokens: HashSet::new(),
            desc_tokens: HashSet::new(),
            host_platforms: vec!["claude".to_string()],
            mcp_server: "router-rs".to_string(),
            tool_flags: vec![],
        };
        McpToolRecord::derive_tokens(&mut record);
        record
    }

    fn make_records() -> Vec<McpToolRecord> {
        vec![
            test_record("pdf-read", "PDF 文本提取", &["pdf", "PDF", "文档", "论文"]),
            test_record("browser-screenshot", "浏览器截图", &["截图", "浏览器", "screenshot"]),
            test_record("browser-click", "浏览器点击", &["点击", "click"]),
            test_record("web-fetch", "网页抓取", &["网页", "fetch", "url", "抓取"]),
            test_record("codegraph-search", "代码图搜索", &["代码", "搜索", "codegraph"]),
            test_record("task-create", "任务创建", &["任务", "task", "创建"]),
            test_record("knowledge-search", "知识搜索", &["知识", "knowledge", "学术"]),
        ]
    }

    #[test]
    fn route_exact_match() {
        let records = make_records();
        let decision = crate::tool_routing::route_tool_from_records("pdf-read", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "pdf-read");
    }

    #[test]
    fn route_chinese_keyword() {
        let records = make_records();
        let decision = crate::tool_routing::route_tool_from_records("帮我截图", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "browser-screenshot");
    }

    #[test]
    fn route_browser_action() {
        let records = make_records();
        // "点击浏览器按钮" — both browser-click and browser-screenshot match "browser" name token
        // and both have CJK trigger hints. browser-screenshot matches "浏览器" (3 keyword hits)
        // while browser-click matches "点击" (2 keyword hits). The result depends on scoring weights.
        // Use "click 按钮" to clearly target browser-click via English trigger hint.
        let decision = crate::tool_routing::route_tool_from_records("click 按钮", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "browser-click");
    }

    #[test]
    fn route_web_fetch() {
        let records = make_records();
        let decision = crate::tool_routing::route_tool_from_records("抓取网页内容", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "web-fetch");
    }

    #[test]
    fn route_code_search() {
        let records = make_records();
        let decision = crate::tool_routing::route_tool_from_records("搜索代码", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "codegraph-search");
    }

    #[test]
    fn search_returns_ranked() {
        let records = make_records();
        let results = crate::tool_search::search_tools("PDF 文档", &records, 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].slug, "pdf-read");
    }

    #[test]
    fn route_empty_returns_none() {
        let records = make_records();
        assert!(crate::tool_routing::route_tool_from_records("", &records).is_none());
    }
}
