#[cfg(test)]
mod tool_eval_harness_contracts {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::path::Path;

    /// Resolve project root from `CARGO_MANIFEST_DIR` (core/tool-routing-engine).
    /// Going up two parents yields the workspace root.
    fn project_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
    }

    #[test]
    fn tool_routing_accuracy_meets_baseline() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let registry_path = manifest_dir.join("../../configs/framework/MCP_TOOL_REGISTRY.json");
        let records = mcp_tool_registry::load_tool_records(&registry_path)
            .expect("should load real MCP_TOOL_REGISTRY.json");

        let cases_path = project_root().join("tests/tool_routing_eval_cases.json");
        let cases = tool_routing_engine::eval::load_tool_routing_eval_cases(&cases_path)
            .expect("load tool routing eval cases");
        let report = tool_routing_engine::eval::evaluate_tool_routing_cases(&records, cases)
            .expect("evaluate tool routing cases");

        let m = &report.metrics;
        let trigger_total = m.trigger_hit + m.trigger_miss;
        let trigger_rate = if trigger_total > 0 {
            (m.trigger_hit as f64) / (trigger_total as f64) * 100.0
        } else {
            100.0
        };
        let overtrigger_rate = if m.case_count > 0 {
            (m.overtrigger as f64) / (m.case_count as f64) * 100.0
        } else {
            0.0
        };
        let tool_accuracy = if m.case_count > 0 {
            (m.tool_correct as f64) / (m.case_count as f64) * 100.0
        } else {
            100.0
        };

        eprintln!("=== Tool Routing Eval Report ===");
        eprintln!("  Cases:          {}", m.case_count);
        eprintln!("  Trigger hit:    {} ({:.1}%)", m.trigger_hit, trigger_rate);
        eprintln!("  Trigger miss:   {}", m.trigger_miss);
        eprintln!(
            "  Overtrigger:    {} ({:.1}%)",
            m.overtrigger, overtrigger_rate
        );
        eprintln!(
            "  Tool correct:   {} ({:.1}%)",
            m.tool_correct, tool_accuracy
        );

        // Print miss/overtrigger details for diagnosis
        for result in &report.results {
            let relevant = matches!(result.category.as_str(), "should-trigger" | "fuzzy-rescue");
            if relevant && !result.trigger_hit {
                eprintln!(
                    "  MISS id={:?} expected_tool={:?} selected={:?} task={:?}",
                    result.id, result.expected_tool, result.selected_tool, result.task
                );
            }
            if result.overtrigger {
                eprintln!(
                    "  OVER id={:?} expected_tool={:?} selected={:?} task={:?}",
                    result.id, result.expected_tool, result.selected_tool, result.task
                );
            }
        }

        // Baselines (adjusted for deprecated tool exclusion and TRE-1 eval fix):
        //   trigger_hit >= 84% (was 85% before deprecated exclusion)
        //   overtrigger <= 10%
        //   tool_accuracy >= 74% (was 80% before TRE-1: should-not-trigger
        //     with selected tool is now correctly counted as tool_correct=false)
        assert!(
            trigger_rate >= 84.0,
            "trigger_hit rate too low: {trigger_rate:.1}% (want >= 84%)"
        );
        assert!(
            overtrigger_rate <= 10.0,
            "overtrigger rate too high: {overtrigger_rate:.1}% (want <= 10%)"
        );
        assert!(
            tool_accuracy >= 74.0,
            "tool_accuracy too low: {tool_accuracy:.1}% (want >= 74%)"
        );
    }
}
