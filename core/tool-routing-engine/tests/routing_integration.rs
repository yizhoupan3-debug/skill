#[cfg(test)]
mod routing_integration_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use mcp_tool_registry::{DispatchDomain, McpToolRecord, ToolLayer, ToolOwner};

    fn test_record(slug: &str, display_name: &str, keywords: &[&str]) -> McpToolRecord {
        McpToolRecord {
            slug: slug.to_string(),
            display_name: display_name.to_string(),
            description: format!("Tool: {display_name}"),
            layer: ToolLayer::Builtin,
            dispatch_domain: DispatchDomain::DomainFramework,
            owner: ToolOwner::Framework,
            trigger_hints: keywords.iter().map(|s| s.to_string()).collect(),
            mcp_server: "router-rs".to_string(),
            tool_flags: vec![],
            input_schema_json: None,
        }
    }

    fn make_records() -> Vec<McpToolRecord> {
        vec![
            test_record("pdf-read", "PDF 文本提取", &["pdf", "PDF", "文档", "论文"]),
            test_record(
                "browser-screenshot",
                "浏览器截图",
                &["截图", "浏览器", "screenshot"],
            ),
            test_record("browser-click", "浏览器点击", &["点击", "click"]),
            test_record("web-fetch", "网页抓取", &["网页", "fetch", "url", "抓取"]),
            test_record(
                "codegraph-search",
                "代码图搜索",
                &["代码", "搜索", "codegraph"],
            ),
            test_record("task-create", "任务创建", &["任务", "task", "创建"]),
            test_record(
                "knowledge-search",
                "知识搜索",
                &["知识", "knowledge", "学术"],
            ),
        ]
    }

    #[test]
    fn route_exact_match() {
        let records = make_records();
        let decision =
            tool_routing_engine::routing::route_tool_from_records("pdf-read", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "pdf-read");
    }

    #[test]
    fn route_chinese_keyword() {
        let records = make_records();
        let decision =
            tool_routing_engine::routing::route_tool_from_records("帮我截图", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "browser-screenshot");
    }

    #[test]
    fn route_browser_action() {
        let records = make_records();
        let decision =
            tool_routing_engine::routing::route_tool_from_records("click 按钮", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "browser-click");
    }

    #[test]
    fn route_web_fetch() {
        let records = make_records();
        let decision =
            tool_routing_engine::routing::route_tool_from_records("抓取网页内容", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "web-fetch");
    }

    #[test]
    fn route_code_search() {
        let records = make_records();
        let decision =
            tool_routing_engine::routing::route_tool_from_records("搜索代码", &records);
        assert!(decision.is_some());
        assert_eq!(decision.unwrap().selected_tool, "codegraph-search");
    }

    #[test]
    fn search_returns_ranked() {
        let records = make_records();
        let results = tool_routing_engine::search::search_tools("PDF 文档", &records, 3, None);
        assert!(!results.is_empty());
        assert_eq!(results[0].selected_tool, "pdf-read");
    }

    #[test]
    fn route_empty_returns_none() {
        let records = make_records();
        assert!(
            tool_routing_engine::routing::route_tool_from_records("", &records).is_none()
        );
    }

    #[test]
    fn route_fuzzy_typo() {
        let records = make_records();
        let decision =
            tool_routing_engine::routing::route_tool_from_records("screeenshot", &records);
        assert!(decision.is_some(), "typo should fuzzy-match");
        let d = decision.unwrap();
        assert!(d.fuzzy_match, "should be flagged as fuzzy match");
        assert_eq!(d.selected_tool, "browser-screenshot");
    }

    #[test]
    fn route_tie_breaking() {
        let records = vec![
            test_record("tool-alpha", "Tool Alpha", &["search"]),
            test_record("tool-beta", "Tool Beta", &["search"]),
        ];
        let decision =
            tool_routing_engine::routing::route_tool_from_records("search", &records);
        assert!(decision.is_some(), "tie should still return a result");
    }

    #[test]
    fn route_emoji_or_punctuation_query() {
        let records = make_records();
        let decision = tool_routing_engine::routing::route_tool_from_records("😊", &records);
        assert!(decision.is_none(), "emoji-only query should not match");
        let decision2 =
            tool_routing_engine::routing::route_tool_from_records("!@#$%^&*", &records);
        assert!(
            decision2.is_none(),
            "punctuation-only query should not match"
        );
    }

    #[test]
    fn route_empty_trigger_hint_string_does_not_match_all() {
        let records = vec![test_record("empty-hints", "Empty Hints", &[""])];
        let decision = tool_routing_engine::routing::route_tool_from_records(
            "something totally unrelated",
            &records,
        );
        assert!(
            decision.is_none(),
            "empty hint string should not match anything"
        );
    }

    #[test]
    fn search_top_k_overflow() {
        let records = make_records();
        let results = tool_routing_engine::search::search_tools("pdf", &records, 9999, None);
        assert!(!results.is_empty(), "should still return results");
        assert!(results.len() <= 100, "top_k should be clamped to MAX_TOP_K");
        assert!(
            results.len() <= records.len(),
            "results should not exceed total record count"
        );
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
            assert!(
                !record.display_name.is_empty(),
                "every tool must have a display_name"
            );
        }
    }

    #[test]
    fn real_registry_routes_by_exact_slug() {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let registry_path = manifest_dir.join("../../configs/framework/MCP_TOOL_REGISTRY.json");
        let records = mcp_tool_registry::load_tool_records(&registry_path)
            .expect("should load real registry");

        for record in &records {
            // Skip tools excluded from routing (deprecated or no_routing)
            if record
                .tool_flags
                .iter()
                .any(|f| f == "deprecated" || f == "no_routing")
            {
                continue;
            }
            let decision =
                tool_routing_engine::routing::route_tool_from_records(&record.slug, &records);
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
