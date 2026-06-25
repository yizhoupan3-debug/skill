#[cfg(test)]
mod routing_integration_tests {
    use mcp_tool_registry::McpToolRecord;

    fn test_record(slug: &str, display_name: &str, keywords: &[&str]) -> McpToolRecord {
        McpToolRecord {
            slug: slug.to_string(),
            display_name: display_name.to_string(),
            description: format!("Tool: {display_name}"),
            layer: "builtin".to_string(),
            dispatch_domain: "composite".to_string(),
            owner: "framework".to_string(),
            gate: "none".to_string(),
            trigger_hints: keywords.iter().map(|s| s.to_string()).collect(),
            host_platforms: vec!["claude".to_string()],
            mcp_server: "router-rs".to_string(),
            tool_flags: vec![],
            input_schema_json: None,
        }
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
        let decision = tool_routing_engine::routing::route_tool_from_records("pdf-read", &records, None);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "pdf-read");
    }

    #[test]
    fn route_chinese_keyword() {
        let records = make_records();
        let decision = tool_routing_engine::routing::route_tool_from_records("帮我截图", &records, None);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "browser-screenshot");
    }

    #[test]
    fn route_browser_action() {
        let records = make_records();
        let decision = tool_routing_engine::routing::route_tool_from_records("click 按钮", &records, None);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "browser-click");
    }

    #[test]
    fn route_web_fetch() {
        let records = make_records();
        let decision = tool_routing_engine::routing::route_tool_from_records("抓取网页内容", &records, None);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "web-fetch");
    }

    #[test]
    fn route_code_search() {
        let records = make_records();
        let decision = tool_routing_engine::routing::route_tool_from_records("搜索代码", &records, None);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "codegraph-search");
    }

    #[test]
    fn search_returns_ranked() {
        let records = make_records();
        let results = tool_routing_engine::search::search_tools("PDF 文档", &records, 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].selected_tool, "pdf-read");
    }

    #[test]
    fn route_empty_returns_none() {
        let records = make_records();
        assert!(tool_routing_engine::routing::route_tool_from_records("", &records, None).is_none());
    }

    #[test]
    fn route_host_filter() {
        let records = make_records();
        let decision = tool_routing_engine::routing::route_tool_from_records("PDF 文档", &records, Some("cursor"));
        assert!(decision.is_none());
    }

    #[test]
    fn route_fuzzy_typo() {
        let records = make_records();
        let decision = tool_routing_engine::routing::route_tool_from_records("screeenshot", &records, None);
        assert!(decision.is_some(), "typo should fuzzy-match");
        let d = decision.unwrap();
        assert!(d.fuzzy_match, "should be flagged as fuzzy match");
        assert_eq!(d.selected_tool, "browser-screenshot");
    }

    #[test]
    fn load_real_tool_registry() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let registry_path = manifest_dir.join("../../configs/framework/MCP_TOOL_REGISTRY.json");
        let records = mcp_tool_registry::load_tool_records(&registry_path)
            .expect("should load real MCP_TOOL_REGISTRY.json");
        assert!(!records.is_empty(), "registry should contain tools");
        assert!(records.len() >= 54, "should have at least 54 tools");

        for record in &records {
            assert!(!record.slug.is_empty(), "every tool must have a slug");
            assert!(!record.display_name.is_empty(), "every tool must have a display_name");
        }
    }

    #[test]
    fn real_registry_routes_by_exact_slug() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let registry_path = manifest_dir.join("../../configs/framework/MCP_TOOL_REGISTRY.json");
        let records = mcp_tool_registry::load_tool_records(&registry_path)
            .expect("should load real registry");

        for record in &records {
            let decision = tool_routing_engine::routing::route_tool_from_records(
                &record.slug,
                &records,
                None,
            );
            assert!(
                decision.is_some(),
                "tool '{}' should be reachable by exact slug match",
                record.slug,
            );
            if let Some(d) = decision {
                assert_eq!(d.selected_tool, record.slug);
            }
        }
    }
}
