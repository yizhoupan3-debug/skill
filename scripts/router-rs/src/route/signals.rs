//! Query heuristics and route context classification.
use super::aliases::framework_alias_requires_explicit_call;
use super::constants::ARTIFACT_GATE_PHRASES;
use super::text::{normalize_text, text_matches_phrase};
use super::types::{RouteContextPayload, SkillRecord};
use regex::Regex;
use std::sync::OnceLock;

fn github_pr_standalone_token_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bpr\b").expect("static github pr token regex"))
}

pub(crate) fn is_meta_routing_task(query_text: &str) -> bool {
    (query_text.contains("skill")
        || query_text.contains("skill.md")
        || query_text.contains("runtime")
        || query_text.contains("framework")
        || query_text.contains("框架"))
        && [
            "路由",
            "触发",
            "routing",
            "router",
            "route",
            "系统",
            "入口",
            "抽象",
            "行为驱动",
            "第一性原理",
            "减法",
            "轻量化",
            "兼容层",
            "胶水层",
            "核查",
            "合并",
            "精简",
            "清理",
            "历史文件",
            "旧文件",
            "口径",
            "contract",
            "沉到 runtime",
            "沉到runtime",
            "减少入口",
            "减入口",
            "不损害功能",
            "加重负担",
            "没有用",
            "runtime 轻量化",
            "讨论-规划-执行-验证",
        ]
        .iter()
        .any(|marker| query_text.contains(marker))
}

pub(crate) fn has_checklist_execution_context(query_text: &str) -> bool {
    query_text.contains("checklist")
        && ![
            "规范",
            "规范化",
            "normalize",
            "normalise",
            "serial",
            "parallel",
            "并行",
            "串行",
        ]
        .iter()
        .any(|marker| query_text.contains(marker))
        && [
            "执行",
            "一口气",
            "彻底",
            "落实",
            "按",
            "fix",
            "implement",
            "run",
            "do it",
        ]
        .iter()
        .any(|marker| query_text.contains(marker))
}

pub(crate) fn has_skill_creator_context(query_text: &str, query_token_list: &[String]) -> bool {
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

pub(crate) fn has_skill_installer_context(query_text: &str, query_token_list: &[String]) -> bool {
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

pub(crate) fn has_skill_framework_maintenance_context(
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
            "轻量化",
            "兼容层",
            "胶水层",
            "核查",
            "合并",
            "精简",
            "清理",
            "历史文件",
            "旧文件",
            "口径",
            "contract",
            "沉到 runtime",
            "沉到runtime",
            "减少入口",
            "减入口",
            "不损害功能",
            "加重负担",
            "没有用",
            "治理任务",
        ]
        .iter()
        .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

pub(crate) fn has_runtime_lightweighting_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "runtime 轻量化",
        "轻量化",
        "兼容层",
        "胶水层",
        "沉到 runtime",
        "沉到runtime",
        "runtime 下沉",
        "下沉 runtime",
        "沉到运行时",
        "减少入口",
        "减入口",
        "不损害功能",
        "加重负担",
        "没有用",
    ]
    .iter()
    .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

pub(crate) fn has_systematic_debug_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "root-cause analysis",
        "root cause analysis",
        "root-cause",
        "root cause",
        "根因",
        "找根因",
        "bug",
        "报错",
        "失败",
        "崩了",
        "不工作",
        "哪里错了",
        "flaky",
        "flake",
        "traceback",
        "error",
        "tdd workflow",
    ]
    .iter()
    .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

pub(crate) fn has_scientific_figure_plotting_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "scientific figures",
        "scientific figure",
        "publication chart",
        "publication figure",
        "journal style",
        "科研出图",
        "论文图",
        "期刊风格",
        "matplotlib",
        "seaborn",
        "plotnine",
        "raincloud",
        "ridge plot",
        "statistical annotations",
        "colorblind-safe",
        "cjk font",
    ]
    .iter()
    .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

pub(crate) fn has_rendered_visual_evidence_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    let direct_evidence = [
        "截图",
        "看图",
        "这张图",
        "这张界面图",
        "screenshot",
        "rendered",
        "already-rendered",
        "image file",
    ]
    .iter()
    .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker));
    direct_evidence || has_existing_image_file_context(query_text, query_token_list)
}

pub(crate) fn has_existing_image_file_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    let has_image_extension = [".png", ".jpg", ".jpeg"]
        .iter()
        .any(|marker| query_text.contains(marker))
        || ["png", "jpg", "jpeg"]
            .iter()
            .any(|marker| text_matches_phrase(query_token_list, marker));
    if !has_image_extension {
        return false;
    }
    [
        "attached",
        "uploaded",
        "existing",
        "already-rendered",
        "image file",
        "png file",
        "jpg file",
        "jpeg file",
        "这张",
        "附件",
        "已渲染",
        "已有",
    ]
    .iter()
    .any(|marker| query_text.contains(marker) || text_matches_phrase(query_token_list, marker))
}

pub(crate) fn has_prose_naturalization_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "润色",
        "润色得自然",
        "自然一点",
        "改自然",
        "自然化",
        "文本精修",
        "表达优化",
        "去模板腔",
        "像人写的",
        "humanize",
        "aigc",
        "ai 味",
        "ai味",
        "ai 感",
        "逐句评估",
        "哪些句子",
        "普通说明",
        "说明文字",
        "普通写作",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_copywriting_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "ux 微文案",
        "ux",
        "微文案",
        "空状态",
        "cta",
        "转化",
        "转化率",
        "点击创建",
        "创建项目",
        "广告词",
        "产品卖点",
        "落地页",
        "品牌故事",
        "copywriting",
        "in-app microcopy",
        "tagline",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn is_overlay_record(record: &SkillRecord) -> bool {
    record.owner_lower == "overlay"
}

pub(crate) fn can_be_primary_owner(record: &SkillRecord) -> bool {
    if !record.primary_allowed {
        return false;
    }
    record.gate_lower == "none"
        && !framework_alias_requires_explicit_call(record)
        && !matches!(record.owner_lower.as_str(), "gate" | "overlay")
}

pub(crate) fn can_be_fallback_owner(record: &SkillRecord) -> bool {
    can_be_primary_owner(record)
        && !matches!(
            record.fallback_policy_mode.as_str(),
            "never" | "explicit-only"
        )
}

/// High-precision Cursor Plan / 策划闸门 intent (aligned with `skills/plan-mode/SKILL.md` trigger_hints).
/// Used to keep delegation-gate admission from overriding the `plan-mode` owner on first-turn routing.
pub(crate) fn has_cursor_plan_mode_owner_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    query_text.contains("cursor plan")
        || query_text.contains("plan 模式")
        || query_text.contains("策划文档闸门")
        || text_matches_phrase(query_token_list, "plan revision round")
        || text_matches_phrase(query_token_list, "可验收 todo")
        || text_matches_phrase(query_token_list, "subagent 审 plan")
        || text_matches_phrase(query_token_list, "gitx plan 收口")
        || text_matches_phrase(query_token_list, "计划对照实际")
        || text_matches_phrase(query_token_list, "独立上下文 review 计划")
        || (query_text.contains("可验收") && query_text.contains("todo"))
}

pub(crate) fn has_bounded_subagent_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "sidecar",
        "sidecars",
        "subagent",
        "subagents",
        "delegation plan",
        "multiagent",
        "multi-agent",
        "多 agent",
        "多 agent 执行",
        "多 agent 路由",
        "bounded sidecar",
        "bounded sidecars",
        "bounded subagent",
        "bounded subagents",
        "subagent lane",
        "sidecar lane",
        "local-supervisor",
        "local-supervisor queue",
        "保留 sidecar 边界",
        "只切 sidecar",
        "并行 sidecar",
        "不实际 spawn",
        "stay local",
        "主线程保留",
        "保留主线程",
        "主线程集成",
        "lane-local output",
        "不创建 worker",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_token_budget_pressure(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "token budget",
        "context budget",
        "token 开销",
        "token 成本",
        "降低 token",
        "压 token",
        "省 token",
        "缩上下文",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_team_negation_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "不要 team",
        "不要进入 team",
        "不进 team",
        "不用 team",
        "无需 team",
        "not team",
        "without team",
        "不要 team orchestration",
        "只是 sidecar",
        "only sidecar",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_team_orchestration_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "team orchestration",
        "team workflow",
        "team mode",
        "team supervisor",
        "worker lifecycle",
        "worker orchestration",
        "multi-worker",
        "multi worker",
        "parallel worker",
        "parallel workers",
        "disjoint files",
        "disjoint file",
        "disjoint write",
        "disjoint writes",
        "disjoint scope",
        "disjoint scopes",
        "disjoint write scope",
        "disjoint write scopes",
        "lane-local",
        "lane local",
        "lane-local delta",
        "worker write scope",
        "worker write scopes",
        "team 协作",
        "团队编排",
        "多 worker",
        "worker 生命周期",
        "supervisor-led",
        "supervisor led",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_parallel_execution_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
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
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    });
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
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
    .count();

    split_shape >= 2
}

pub(crate) fn has_parallel_review_candidate_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    let markers = crate::review_routing_signals::parallel_review_candidate_markers();
    let review_requested = markers.review_markers.iter().any(|marker| {
        // Avoid treating "revision" / "revisions" as a standalone "review" hit.
        if marker.as_str() == "review" {
            return text_matches_phrase(query_token_list, "review");
        }
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    });
    if !review_requested {
        return false;
    }

    let broad_or_independent = markers.breadth_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    });
    if !broad_or_independent {
        return false;
    }

    markers.scope_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn paper_skill_requires_context(slug: &str) -> bool {
    matches!(
        slug,
        "paper-workbench" | "paper-reviewer" | "paper-reviser" | "paper-writing"
    )
}

pub(crate) fn has_paper_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "paper",
        "manuscript",
        "论文",
        "稿子",
        "稿件",
        "摘要",
        "引言",
        "审稿意见",
        "reviewer comments",
        "rebuttal",
        "appendix",
        "claim",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_github_pr_context(query_text: &str, query_token_list: &[String]) -> bool {
    query_text.contains(&normalize_text("github"))
        || text_matches_phrase(query_token_list, "github")
        || query_text.contains(&normalize_text("gh"))
        || text_matches_phrase(query_token_list, "gh")
        || query_text.contains(&normalize_text("pull request"))
        || text_matches_phrase(query_token_list, "pull request")
        || github_pr_standalone_token_regex().is_match(query_text)
        || text_matches_phrase(query_token_list, "pr")
}

pub(crate) fn has_pr_triage_summary_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "quick PR 状态梳理",
        "pr 状态梳理",
        "pr review summary",
        "pull request summary",
        "reviewer feedback digest",
        "changed-file digest",
        "changed files summary",
        "pr triage",
        "pr-level follow-up",
        "pr follow-up",
        "changed-file surface",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_sentry_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "sentry",
        "production error",
        "production errors",
        "线上异常",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_ci_failure_context(query_text: &str, query_token_list: &[String]) -> bool {
    let phrase_match = [
        "github actions",
        "actions failure",
        "failing check",
        "failing checks",
        "failed check",
        "failed checks",
        "check failure",
        "checks failure",
        "build failure",
        "workflow failure",
        "failing workflow",
        "ci failure",
        "ci failing",
        "fix ci",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    });
    phrase_match || query_token_list.iter().any(|token| token == "ci")
}

pub(crate) fn has_non_github_ci_provider_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "gitlab",
        "gitlab ci",
        "circleci",
        "circle ci",
        "jenkins",
        "azure pipelines",
        "buildkite",
        "travis",
        "bitbucket pipelines",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn should_route_to_gh_fix_ci(query_text: &str, query_token_list: &[String]) -> bool {
    has_ci_failure_context(query_text, query_token_list)
        && (has_github_pr_context(query_text, query_token_list)
            || !has_non_github_ci_provider_context(query_text, query_token_list))
}

pub(crate) fn has_paper_review_revision_intent(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let review_markers = [
        "review",
        "reviewer comments",
        "review comments",
        "审稿意见",
        "评审意见",
    ];
    let revise_markers = ["改论文", "修改论文", "改稿", "修改稿", "进入修改", "直接改"];
    review_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) && revise_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_paper_direct_revision_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    if [
        "该删就删",
        "藏到附录",
        "改到能投",
        "根据 reviewer comments 修改论文",
        "根据 reviewer comments 改论文",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) {
        return false;
    }
    [
        "别先给方案",
        "直接进入修改",
        "直接改稿",
        "不要再审",
        "只进改稿",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_paper_workbench_frontdoor_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    [
        "整体推进这篇论文",
        "现在该审",
        "该审",
        "该改",
        "该补实验",
        "怎么处理",
        "先审再改",
        "改到能投",
        "该删就删",
        "藏到附录",
        "根据 reviewer comments 修改论文",
        "根据 reviewer comments 改论文",
        "能不能投",
        "整篇严审",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_paper_writing_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    if has_paper_ref_first_workflow_context(query_text, query_token_list)
        || has_paper_review_judgment_context(query_text, query_token_list)
        || query_text.contains("别润色")
        || query_text.contains("不润色")
    {
        return false;
    }
    [
        "润色",
        "文字精修",
        "表达",
        "故事线",
        "重写摘要",
        "重写引言",
        "只改表达",
        "polish",
        "rewrite introduction",
        "rewrite abstract",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_paper_review_judgment_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    [
        "paper review",
        "review paper",
        "审稿",
        "审一下",
        "严审",
        "投稿前",
        "能不能投",
        "投稿判断",
        "reviewer-style",
        "reviewer style",
        "外部调研",
        "查文献后审",
        "科学性批评",
        "科学批评",
        "只要批评",
        "只批评",
        "只要科学",
        "不要改稿",
        "别改稿",
        "只审不改",
        "critique only",
        "critique-only",
        "review only",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_paper_figure_layout_review_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let visual_markers = [
        "图表", "排版", "figure", "figures", "table", "tables", "layout",
    ];
    let review_markers = ["只看", "审", "review", "检查", "别检查别的维度"];
    visual_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) && review_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_paper_logic_evidence_review_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let logic_markers = [
        "claim",
        "claims",
        "evidence",
        "证据",
        "支撑",
        "实验支撑",
        "对齐",
        "够不够",
    ];
    let review_markers = ["看", "检查", "评估", "review", "审", "别润色"];
    logic_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) && review_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_paper_ref_first_workflow_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let ref_markers = [
        "下载ref",
        "目标期刊",
        "相近ref",
        "相近 ref",
        "reference corpus",
        "target journal",
    ];
    let story_or_write_markers = [
        "讲故事",
        "故事线",
        "写作套路",
        "重写摘要",
        "重写引言",
        "再写",
        "再帮我重写",
    ];
    ref_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    }) && story_or_write_markers.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_design_reference_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "参考源",
        "verified tokens",
        "品牌 token",
        "stripe",
        "linear",
        "apple",
        "vercel",
        "liquid glass motion",
        "产品风格映射",
        "borrowable cues",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_visual_evidence_review_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "看图",
        "截图",
        "界面图",
        "视觉问题",
        "可读性审查",
        "重叠",
        "层级",
        "渲染",
        "rendered",
        "screenshot",
        "visual review",
        "ui overlap",
        "readability review",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn artifact_gate_matches_query(query_token_list: &[String]) -> bool {
    ARTIFACT_GATE_PHRASES
        .iter()
        .any(|phrase| text_matches_phrase(query_token_list, phrase))
}

pub(crate) fn artifact_gate_target_slug(query_token_list: &[String]) -> Option<&'static str> {
    const ARTIFACT_TARGETS: [(&str, &[&str]); 4] = [
        (
            "spreadsheets",
            &[
                "xlsx",
                "excel",
                "spreadsheet",
                "xls",
                "csv",
                "tsv",
                "sheet review",
                "工作簿",
            ],
        ),
        (
            "slides",
            &[
                "ppt",
                "pptx",
                "slides",
                "powerpoint",
                "presentation",
                "deck",
                "slide deck",
                "幻灯片",
                "演示文稿",
            ],
        ),
        ("doc", &["docx", "word 文档", "word 文件"]),
        ("pdf", &["pdf"]),
    ];

    ARTIFACT_TARGETS.iter().find_map(|(slug, phrases)| {
        phrases
            .iter()
            .any(|phrase| text_matches_phrase(query_token_list, phrase))
            .then_some(*slug)
    })
}

pub(crate) fn has_design_contract_context(query_text: &str, query_token_list: &[String]) -> bool {
    const MARKERS: [&str; 18] = [
        "design.md",
        "设计规范",
        "设计系统",
        "设计 token",
        "design token",
        "design tokens",
        "视觉身份",
        "视觉规范",
        "品牌风格",
        "品牌规范",
        "house style",
        "visual identity",
        "style contract",
        "统一设计规范",
        "统一视觉",
        "统一风格",
        "风格漂移",
        "根据 design.md",
    ];
    MARKERS.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_design_contract_negation_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    const MARKERS: [&str; 10] = [
        "不需要设计系统",
        "不需要设计规范",
        "不用设计系统",
        "不用设计规范",
        "无需设计系统",
        "无需设计规范",
        "不要设计系统",
        "不要设计规范",
        "no design system",
        "without design system",
    ];
    MARKERS.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_design_output_audit_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "设计审计",
        "设计验收",
        "验收结论",
        "风格漂移",
        "ai 味",
        "反模式",
        "drift",
        "anti-pattern",
        "audit produced",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_design_workflow_protocol_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "设计工件协议",
        "设计工作流",
        "设计迭代协议",
        "design workflow",
        "design artifact protocol",
        "prompt 到 screenshot 到 verdict",
        "每轮都按这个工作流跑",
        "工作流跑",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_quick_artifact_context(query_text: &str, query_token_list: &[String]) -> bool {
    const MARKERS: [&str; 8] = [
        "快速", "普通", "简单", "临时", "quick", "simple", "draft", "utility",
    ];
    MARKERS.iter().any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn should_defer_to_artifact_gate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if record.gate_lower != "none" || !artifact_gate_matches_query(query_token_list) {
        return false;
    }
    let explicit_entry = format!("${}", record.slug_lower);
    if query_text.contains(&explicit_entry) {
        return false;
    }
    if record.slug == "ppt-beamer" && has_beamer_slide_context(query_text, query_token_list) {
        return false;
    }
    if record.slug == "source-slide-formats"
        && has_source_slide_format_context(query_text, query_token_list)
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

pub(crate) fn should_suppress_non_target_artifact_gate(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if record.slug == "design-md"
        && has_design_contract_context(query_text, query_token_list)
        && !has_design_contract_negation_context(query_text, query_token_list)
    {
        return false;
    }
    record.gate_lower == "artifact"
        && !is_meta_routing_task(query_text)
        && artifact_gate_target_slug(query_token_list)
            .map(|target| record.slug != target)
            .unwrap_or(false)
}

pub(crate) fn should_prefer_design_contract_over_artifact(
    record: &SkillRecord,
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    record.slug == "slides"
        && has_design_contract_context(query_text, query_token_list)
        && !has_design_contract_negation_context(query_text, query_token_list)
}

pub(crate) fn has_beamer_slide_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "beamer",
        "beamer slides",
        "latex beamer",
        "latex 幻灯片",
        "beamer 编译",
        "学术 ppt",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_source_slide_format_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    [
        "markdown slides",
        "slidev",
        "marp",
        "html slides",
        "source slide formats",
        "source-first slides",
        "用 markdown 做 slides",
        "根据大纲做 html slides",
        "browser-matched pdf",
        "presentation.html",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn has_diagramming_context(query_text: &str, query_token_list: &[String]) -> bool {
    [
        "mermaid",
        "graphviz",
        "dot diagram",
        "流程图",
        "研究流程图",
        "技术路线图",
        "方法图",
        "实验流程",
        "pipeline 图",
        "时序图",
        "架构图",
        "依赖图",
        "状态机",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(&normalize_text(marker))
            || text_matches_phrase(query_token_list, marker)
    })
}

pub(crate) fn completion_execution_markers() -> [&'static str; 10] {
    [
        "gsd",
        "get shit done",
        "推进到底",
        "别停",
        "直接干完",
        "一路做完",
        "持续跑到收敛",
        "给验证证据",
        "给我验证证据",
        "验证证据",
    ]
}

pub(crate) fn supervisor_execution_markers() -> [&'static str; 9] {
    [
        ".supervisor_state.json",
        "共享 continuity",
        "shared continuity",
        "多 lane 集成",
        "主线程集成",
        "integration supervisor",
        "supervisor",
        "长运行",
        "状态持久化",
    ]
}

pub(crate) fn build_route_context(
    query_text: &str,
    query_token_list: &[String],
) -> RouteContextPayload {
    let completion_requested = completion_execution_markers().iter().any(|marker| {
        query_text.contains(*marker) || text_matches_phrase(query_token_list, marker)
    });
    let supervisor_required = supervisor_execution_markers().iter().any(|marker| {
        query_text.contains(*marker) || text_matches_phrase(query_token_list, marker)
    });
    let delegation_candidate = has_bounded_subagent_context(query_text, query_token_list)
        || has_team_orchestration_context(query_text, query_token_list)
        || has_parallel_review_candidate_context(query_text, query_token_list)
        || has_parallel_execution_context(query_text, query_token_list);
    let audit_requested = [
        "核查",
        "审查",
        "审核",
        "审计",
        "评审",
        "诊断",
        "有什么问题",
        "哪里错了",
        "audit",
        "review",
        "diagnose",
    ]
    .iter()
    .any(|marker| query_text.contains(*marker) || text_matches_phrase(query_token_list, marker));
    let implementation_requested = [
        "实现",
        "修复",
        "开发",
        "落地",
        "直接做代码",
        "implement",
        "fix",
        "code",
    ]
    .iter()
    .any(|marker| query_text.contains(*marker) || text_matches_phrase(query_token_list, marker));
    let route_reason = if supervisor_required {
        "explicit_supervisor_continuity"
    } else if delegation_candidate {
        "delegation_gate_candidate"
    } else if completion_requested {
        "completion_signal_context"
    } else {
        "narrowest_domain_owner"
    };

    RouteContextPayload {
        execution_protocol: if implementation_requested && !audit_requested {
            "implementation"
        } else if audit_requested {
            "audit"
        } else {
            "four_step"
        }
        .to_string(),
        verification_required: true,
        evidence_required: audit_requested || !implementation_requested,
        supervisor_required,
        delegation_candidate,
        continue_safe_local_steps: completion_requested,
        route_reason: route_reason.to_string(),
    }
}

#[cfg(test)]
mod github_pr_context_tests {
    use super::*;
    use crate::route::tokenize_query;

    #[test]
    fn github_pr_context_does_not_match_preview_primary() {
        let q = "preview the layout before deploy";
        let tok = tokenize_query(q);
        assert!(!has_github_pr_context(q, &tok));
        let q2 = "primary owner for the module";
        let tok2 = tokenize_query(q2);
        assert!(!has_github_pr_context(q2, &tok2));
    }

    #[test]
    fn github_pr_context_matches_pr_token_and_phrase() {
        let spaced = "please triage my pr now";
        let tok = tokenize_query(spaced);
        assert!(has_github_pr_context(spaced, &tok));
        let spaced2 = "please triage pr fixes";
        let tok2 = tokenize_query(spaced2);
        assert!(has_github_pr_context(spaced2, &tok2));
    }
}
