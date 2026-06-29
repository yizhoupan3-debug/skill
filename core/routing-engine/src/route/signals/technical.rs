use super::has_signal_by_name;
use crate::text::text_matches_phrase;
use crate::types::SkillRecord;

pub fn is_meta_routing_task(query_text: &str) -> bool {
    let anchor_hit = super::meta_routing_anchors()
        .iter()
        .any(|a| query_text.contains(a.as_str()));
    if !anchor_hit {
        return false;
    }
    super::meta_routing_markers()
        .iter()
        .any(|m| query_text.contains(m.as_str()))
}

pub fn has_skill_creator_context(query_text: &str, query_token_list: &[String]) -> bool {
    (query_text.contains("skill") || query_text.contains("skill.md"))
        && [
            "创建",
            "新建",
            "写一个",
            "写个",
            "做一个",
            "做个",
            "create",
            "author",
            "scaffold",
            "update",
            "revise",
        ]
        .iter()
        .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

pub fn has_skill_installer_context(query_text: &str, query_token_list: &[String]) -> bool {
    query_text.contains("skill")
        && [
            "安装",
            "装一下",
            "装一个",
            "装个",
            "导入",
            "引入",
            "install",
            "installed",
            "curated",
            "github",
        ]
        .iter()
        .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

pub fn has_skill_framework_maintenance_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    (query_text.contains("skill")
        || query_text.contains("skill.md")
        || query_text.contains("runtime")
        || query_text.contains("框架")
        || query_text.contains(".supervisor_state"))
        && [
            "不好用",
            "持续优化",
            "外部调研",
            "路由没触发",
            "触发不准",
            "优化 skill",
            "framework",
            "routing",
            "skill 系统",
            "skill系统",
            // NOTE: "轻量化", "兼容层", "胶水层", "沉到 runtime", "沉到runtime",
            // "减少入口", "减入口", "不损害功能", "加重负担", "没有用" were removed
            // here to avoid implicit double-counting with SIGNAL_DEFS
            // `runtime_lightweighting` markers. Those markers are now scored via
            // `has_signal_by_name("runtime_lightweighting", …)` / NL route
            // adjustments — no framework keyword gate needed.
            "核查",
            "合并",
            "精简",
            "清理",
            "历史文件",
            "旧文件",
            "口径",
            "contract",
            "治理任务",
        ]
        .iter()
        .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

pub fn has_runtime_lightweighting_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("runtime_lightweighting", query_text, query_token_list)
}

pub fn has_systematic_debug_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("systematic_debug", query_text, query_token_list)
}

pub fn has_copywriting_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("copywriting", query_text, query_token_list)
}

pub fn has_bounded_subagent_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("bounded_subagent", query_text, query_token_list)
}

pub fn has_token_budget_pressure(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("token_budget_pressure", query_text, query_token_list)
}

pub fn has_workflow_negation_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("workflow_negation", query_text, query_token_list)
}

pub fn has_workflow_orchestration_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("workflow_orchestration", query_text, query_token_list)
}

pub fn has_parallel_execution_context(query_text: &str, query_token_list: &[String]) -> bool {
    let explicit_parallel = [
        "并行",
        "同时",
        "分头",
        "分路",
        "分三路",
        "多路",
        "多线",
        "多方向",
        "多个方向",
        "独立方向",
        "独立维度",
        "parallel",
        "concurrent",
        "in parallel",
        "split lanes",
        "split work",
    ]
    .iter()
    .any(|marker| query_text.contains(*marker) || text_matches_phrase(query_token_list, marker));
    if !explicit_parallel {
        return false;
    }

    let split_shape = [
        "三个方向",
        "三方向",
        "三个模块",
        "三模块",
        "多个模块",
        "多个假设",
        "多个独立",
        "前端",
        "后端",
        "测试",
        "api",
        "数据库",
        // "ui" must use token-level matching only in the filter below:
        // substring match causes false positives (e.g. "guide", "quick", "build").
        // We keep it in the list but the filter logic handles it separately
        // (see the `if marker == "ui"` guard below).
        "ui",
        "安全",
        "性能",
        "架构",
        "实现",
        "策略",
        "验证",
        "frontend",
        "backend",
        "testing",
        "tests",
        "database",
        "security",
        "performance",
        "architecture",
        "implementation",
        "verification",
    ]
    .iter()
    .filter(|marker| {
        // "ui" is token-level only to avoid false positives from
        // substring match (e.g. "guide", "quick", "build").
        if **marker == "ui" {
            return text_matches_phrase(query_token_list, marker);
        }
        query_text.contains(*marker) || text_matches_phrase(query_token_list, marker)
    })
    .count();

    split_shape >= 2
}

pub fn has_parallel_review_candidate_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    let markers = crate::hooks::parallel_review_candidate_markers();
    let review_requested = markers.review_markers.iter().any(|marker| {
        // Avoid treating "revision" / "revisions" as a standalone "review" hit.
        if marker.as_str() == "review" {
            return text_matches_phrase(query_token_list, "review");
        }
        query_text.contains(marker.as_str()) || text_matches_phrase(query_token_list, marker)
    });
    if !review_requested {
        return false;
    }

    let broad_or_independent = markers.breadth_markers.iter().any(|marker| {
        query_text.contains(marker.as_str()) || text_matches_phrase(query_token_list, marker)
    });
    if !broad_or_independent {
        return false;
    }

    markers.scope_markers.iter().any(|marker| {
        query_text.contains(marker.as_str()) || text_matches_phrase(query_token_list, marker)
    })
}

pub fn has_github_pr_context(query_text: &str, query_token_list: &[String]) -> bool {
    query_text.contains("github")
        || text_matches_phrase(query_token_list, "github")
        || text_matches_phrase(query_token_list, "gh")
        || query_text.contains("pull request")
        || text_matches_phrase(query_token_list, "pull request")
        || super::github_pr_standalone_token_regex().is_match(query_text)
        || text_matches_phrase(query_token_list, "pr")
}

pub fn has_pr_triage_summary_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("pr_triage_summary", query_text, query_token_list)
}

pub fn has_sentry_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("sentry", query_text, query_token_list)
}

pub fn has_non_github_ci_provider_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("non_github_ci_provider", query_text, query_token_list)
}

pub fn should_defer_to_artifact_gate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if record.gate_lower != "none" || !super::artifact_gate_matches_query(query_token_list) {
        return false;
    }
    let explicit_entry = format!("${}", record.slug_lower);
    if query_text.contains(&explicit_entry) {
        return false;
    }
    if record
        .skill_flags
        .iter()
        .any(|f| f == "artifact_exception:ppt_beamer")
        && super::design::has_beamer_slide_context(query_text, query_token_list)
    {
        return false;
    }
    if record
        .skill_flags
        .iter()
        .any(|f| f == "artifact_exception:source_slide_formats")
        && super::design::has_source_slide_format_context(query_text, query_token_list)
    {
        return false;
    }
    record.session_start_lower == "n/a"
        && (record
            .name_tokens
            .iter()
            .any(|token| query_token_list.contains(token))
            || record
                .trigger_hints
                .iter()
                .any(|hint| text_matches_phrase(query_token_list, hint)))
}

pub fn should_suppress_non_target_artifact_gate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if record
        .skill_flags
        .iter()
        .any(|f| f == "artifact_exception:design_md_suppress")
        && super::design::has_design_contract_context(query_text, query_token_list)
        && !super::design::has_design_contract_negation_context(query_text, query_token_list)
    {
        return false;
    }
    record.gate_lower == "artifact"
        && !super::is_meta_routing_task(query_text)
        && super::artifact_gate_target_slug(query_token_list)
            .map(|target| record.slug != target)
            .unwrap_or(false)
}

pub fn should_prefer_design_contract_over_artifact(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    record
        .skill_flags
        .iter()
        .any(|f| f == "artifact_exception:slides_design_contract")
        && super::design::has_design_contract_context(query_text, query_token_list)
        && !super::design::has_design_contract_negation_context(query_text, query_token_list)
}
