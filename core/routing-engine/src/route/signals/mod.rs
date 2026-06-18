//! Query heuristics and route context classification.
//!
//! ## Marker sources (do not duplicate without reason)
//!
//! - **JSON**: [`ROUTING_SIGNAL_MARKERS.json`](../../../configs/framework/ROUTING_SIGNAL_MARKERS.json)
//!   — completion / supervisor strings, meta-routing anchors, and other lists loaded via
//!   `routing_signal_markers_json()` helpers at the top of this module. Prefer adding **new
//!   cross-cutting phrase lists** here when they are pure substring / token vocabulary shared
//!   across many skills.
//! - **Rust `has_*` functions**: domain-specific or scoring-coupled heuristics (often need
//!   `normalize_text` / `text_matches_phrase` / `SkillRecord` context) stay as functions in this
//!   file until a clear JSON migration path exists.
//!
//! Default rule: if the marker set is **large, stable vocabulary** reused broadly, put it in JSON;
//! if it needs **Rust-only helpers or record-aware logic**, keep a `has_*` here and cross-link in
//! `NL_ROUTE_ADJUSTMENTS.json` docs when relevant.
use super::aliases::framework_alias_requires_explicit_call;
use super::constants::ARTIFACT_GATE_PHRASES;
use super::text::text_matches_phrase;
use super::types::{RouteContextPayload, SkillRecord};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

pub mod paper;
pub mod design;
pub mod technical;
pub mod tooling;

pub use paper::*;
pub use design::*;
pub use technical::*;
pub use tooling::*;

// ---------------------------------------------------------------------------
// Data-driven signal table
// ---------------------------------------------------------------------------

/// How markers are matched against the query.
pub(crate) enum SignalMatchMode {
    /// `query_text.contains(normalize_text(marker)) || text_matches_phrase(...)`.
    NormalizeAndToken,
    /// `query_text.contains(marker) || text_matches_phrase(...)` — raw substring + token.
    ContainsOrToken,
}

pub(crate) struct SignalDef {
    name: &'static str,
    mode: SignalMatchMode,
    markers: &'static [&'static str],
}

macro_rules! sig {
    ($name:expr, normalize => $markers:expr) => {
        SignalDef {
            name: $name,
            mode: SignalMatchMode::NormalizeAndToken,
            markers: $markers,
        }
    };
    ($name:expr, contains_or_token => $markers:expr) => {
        SignalDef {
            name: $name,
            mode: SignalMatchMode::ContainsOrToken,
            markers: $markers,
        }
    };
}

/// All pure-template signal definitions. Each entry replaces a hand-written
/// `has_xxx_context` function that was just `.iter().any(…)` over a marker list.
const SIGNAL_DEFS: &[SignalDef] = &[
    // ── Contains-mode signals (raw `query_text.contains(marker)`) ──────
    sig!("runtime_lightweighting", contains_or_token => &[
        "runtime 轻量化", "轻量化", "兼容层", "胶水层",
        "沉到 runtime", "沉到runtime", "runtime 下沉", "下沉 runtime",
        "沉到运行时", "减少入口", "减入口", "不损害功能", "加重负担", "没有用",
    ]),
    sig!("systematic_debug", contains_or_token => &[
        "root-cause analysis", "root cause analysis", "root-cause", "root cause",
        "根因", "找根因", "bug", "报错", "失败", "崩了", "不工作", "哪里错了",
        "flaky", "flake", "traceback", "error", "tdd workflow", "tdd",
        "定位根因", "修这个 bug", "fix login", "login bug",
    ]),
    sig!("copywriting", normalize => &[
        "ux 微文案", "ux", "微文案", "空状态", "cta", "转化", "转化率",
        "点击创建", "创建项目", "广告词", "产品卖点", "落地页", "品牌故事",
        "copywriting", "in-app microcopy", "tagline",
    ]),
    sig!("prose_naturalization", normalize => &[
        "润色", "润色得自然", "自然一点", "改自然", "自然化", "文本精修",
        "表达优化", "去模板腔", "像人写的", "humanize", "aigc", "ai 味", "ai味",
        "ai 感", "逐句评估", "哪些句子", "普通说明", "说明文字", "普通写作",
    ]),
    sig!("paper", normalize => &[
        "paper", "manuscript", "论文", "稿子", "稿件", "摘要", "引言",
        "审稿意见", "reviewer comments", "rebuttal", "appendix", "claim",
        "投稿", "期刊",
    ]),
    sig!("scientific_figure_plotting", normalize => &[
        "scientific figures", "scientific figure", "publication chart",
        "publication figure", "journal style", "科研出图", "论文图", "期刊风格",
        "matplotlib", "seaborn", "plotnine", "raincloud", "ridge plot",
        "statistical annotations", "colorblind-safe", "cjk font",
    ]),
    // ── NormalizeAndToken-mode signals ─────────────────────────────────
    sig!("sentry", normalize => &[
        "sentry", "production error", "production errors", "线上异常",
    ]),
    sig!("pr_triage_summary", normalize => &[
        "quick PR 状态梳理", "pr 状态梳理", "pr review summary",
        "pull request summary", "reviewer feedback digest", "changed-file digest",
        "changed files summary", "pr triage", "pr-level follow-up",
        "pr follow-up", "changed-file surface",
    ]),
    sig!("non_github_ci_provider", normalize => &[
        "gitlab", "gitlab ci", "circleci", "circle ci", "jenkins",
        "azure pipelines", "buildkite", "travis", "bitbucket pipelines",
    ]),
    sig!("design_reference", normalize => &[
        "参考源", "verified tokens", "品牌 token", "stripe", "linear", "apple",
        "vercel", "liquid glass motion", "产品风格映射", "borrowable cues",
    ]),
    sig!("visual_evidence_review", normalize => &[
        "看图", "截图", "界面图", "视觉问题", "可读性审查", "重叠", "层级",
        "渲染", "rendered", "screenshot", "visual review", "ui overlap",
        "readability review",
    ]),
    sig!("design_output_audit", normalize => &[
        "设计审计", "设计验收", "验收结论", "风格漂移", "ai 味", "反模式",
        "drift", "anti-pattern", "audit produced",
    ]),
    sig!("design_workflow_protocol", normalize => &[
        "设计工件协议", "设计工作流", "设计迭代协议", "design workflow",
        "design artifact protocol", "prompt 到 screenshot 到 verdict",
        "每轮都按这个工作流跑", "工作流跑",
    ]),
    sig!("beamer_slide", normalize => &[
        "beamer", "beamer slides", "latex beamer", "latex 幻灯片",
        "beamer 编译", "学术 ppt",
    ]),
    sig!("source_slide_format", normalize => &[
        "markdown slides", "slidev", "marp", "html slides", "source slide formats",
        "source-first slides", "用 markdown 做 slides", "根据大纲做 html slides",
        "browser-matched pdf", "presentation.html",
    ]),
    sig!("diagramming", normalize => &[
        "mermaid", "graphviz", "dot diagram", "流程图", "研究流程图",
        "技术路线图", "方法图", "实验流程", "pipeline 图", "时序图", "架构图",
        "依赖图", "状态机",
    ]),
    sig!("bounded_subagent", normalize => &[
        "sidecar", "sidecars", "subagent", "subagents", "delegation plan",
        "multiagent", "multi-agent", "多 agent", "多 agent 执行", "多 agent 路由",
        "bounded sidecar", "bounded sidecars", "bounded subagent",
        "bounded subagents", "subagent lane", "sidecar lane",
        "local-supervisor", "local-supervisor queue", "保留 sidecar 边界",
        "只切 sidecar", "并行 sidecar", "不实际 spawn", "stay local",
        "主线程保留", "保留主线程", "主线程集成", "lane-local output",
        "不创建 worker",
    ]),
    sig!("token_budget_pressure", normalize => &[
        "token budget", "context budget", "token 开销", "token 成本",
        "降低 token", "压 token", "省 token", "缩上下文",
    ]),
    sig!("workflow_negation", normalize => &[
        "不要 workflow", "不要进入 workflow", "不进 workflow", "不用 workflow",
        "无需 workflow", "not workflow", "without workflow",
        "不要 workflow orchestration", "不要 workflow 编排", "只是 sidecar",
        "only sidecar", "tdd workflow", "用 tdd workflow",
        "test driven development",
    ]),
    sig!("workflow_orchestration", normalize => &[
        "workflow orchestration", "workflow supervisor", "workflow mode",
        "workflow 编排", "用 workflow", ".claude/workflows", "/workflow",
        "ultracode", "worker lifecycle", "worker orchestration",
        "multi-worker", "multi worker", "parallel worker", "parallel workers",
        "disjoint files", "disjoint file", "disjoint write", "disjoint writes",
        "disjoint scope", "disjoint scopes", "disjoint write scope",
        "disjoint write scopes", "lane-local", "lane local", "lane-local delta",
        "worker write scope", "worker write scopes", "workflow 协作",
        "团队编排", "多 worker", "worker 生命周期",
        "supervisor-led", "supervisor led",
    ]),
    sig!("explicit_prose_polish", normalize => &[
        "润色", "文字精修", "SCI润色", "SCI 润色", "英文论文润色",
        "学术润色", "只改表达", "polish", "proofread", "copyedit",
        "rewrite introduction", "rewrite abstract", "manuscript editing",
        "academic writing",
    ]),
    sig!("design_contract", normalize => &[
        "design.md", "设计规范", "设计系统", "设计 token", "design token",
        "design tokens", "视觉身份", "视觉规范", "品牌风格", "品牌规范",
        "house style", "visual identity", "style contract", "统一设计规范",
        "统一视觉", "统一风格", "风格漂移", "根据 design.md",
    ]),
    sig!("design_contract_negation", normalize => &[
        "不需要设计系统", "不需要设计规范", "不用设计系统", "不用设计规范",
        "无需设计系统", "无需设计规范", "不要设计系统", "不要设计规范",
        "no design system", "without design system",
    ]),
    sig!("research_context", normalize => &[
        "科研日志", "研究日志", "研究工作区", "研究记录", "科研记录",
        "research log", "research log entry", "experiment log",
        "实验记录", "日志记录", "科研笔记", "科研回顾",
    ]),
    sig!("quick_artifact", normalize => &[
        "快速", "普通", "简单", "临时", "quick", "simple", "draft", "utility",
    ]),
    sig!("codegraph_index_ready", normalize => &[
        "codegraph", "调用链", "影响半径", "call graph", "callers",
        "callees", "impact analysis", "死代码", "dead code",
        "重构影响", "refactor impact", "rename symbol",
        "symbol search", "代码搜索", "调用者", "被调用者",
        "影响范围", "函数调用", "符号定义", "符号引用",
    ]),
];

/// Generic engine: check whether `query_text` / `query_token_list` matches
/// any marker in `markers` using the given `mode`.
pub(crate) fn signal_matches(
    mode: &SignalMatchMode,
    query_text: &str,
    query_token_list: &[String],
    markers: &[&str],
) -> bool {
    match mode {
        // query_text is already normalized at routing entry; markers are static lowercase ASCII.
        // Skip per-marker normalize_text allocation — direct contains is sufficient.
        SignalMatchMode::NormalizeAndToken | SignalMatchMode::ContainsOrToken => {
            markers
                .iter()
                .any(|m| query_text.contains(*m) || text_matches_phrase(query_token_list, m))
        }
    }
}

/// Look up a signal definition by name and evaluate it.
pub(crate) fn has_signal_by_name(name: &str, query_text: &str, query_token_list: &[String]) -> bool {
    SIGNAL_DEFS
        .iter()
        .find(|def| def.name == name)
        .map(|def| signal_matches(&def.mode, query_text, query_token_list, def.markers))
        .unwrap_or(false)
}

const ROUTING_SIGNAL_MARKERS_EMBED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/framework/ROUTING_SIGNAL_MARKERS.json"
));

pub(crate) fn routing_signal_markers_json() -> &'static Value {
    static CELL: OnceLock<Value> = OnceLock::new();
    CELL.get_or_init(|| {
        let v: Value = serde_json::from_str(ROUTING_SIGNAL_MARKERS_EMBED)
            .expect("ROUTING_SIGNAL_MARKERS.json: embedded JSON must parse (build artifact corrupted)");
        let version = v.get("schema_version").and_then(Value::as_str);
        assert_eq!(
            version,
            Some("routing-signal-markers-v1"),
            "ROUTING_SIGNAL_MARKERS schema_version={version:?}, expected \"routing-signal-markers-v1\" — \
             configs/framework/ROUTING_SIGNAL_MARKERS.json was modified without updating this assertion"
        );
        v
    })
}

pub(crate) fn string_list_field<'a>(root: &'a Value, key: &'static str) -> &'a Vec<Value> {
    root.get(key).and_then(Value::as_array).unwrap_or_else(|| {
        panic!(
            "ROUTING_SIGNAL_MARKERS.json missing required array field `{key}` — \
             check configs/framework/ROUTING_SIGNAL_MARKERS.json schema"
        )
    })
}

pub(crate) fn meta_routing_anchors() -> &'static [String] {
    static CELL: OnceLock<Vec<String>> = OnceLock::new();
    CELL.get_or_init(|| {
        let root = routing_signal_markers_json();
        let arr = root
            .pointer("/meta_routing_task/anchor_any_of_substrings")
            .and_then(Value::as_array)
            .expect("meta_routing_task.anchor_any_of_substrings");
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    })
}

pub(crate) fn meta_routing_markers() -> &'static [String] {
    static CELL: OnceLock<Vec<String>> = OnceLock::new();
    CELL.get_or_init(|| {
        let root = routing_signal_markers_json();
        let arr = root
            .pointer("/meta_routing_task/marker_any_of_substrings")
            .and_then(Value::as_array)
            .expect("meta_routing_task.marker_any_of_substrings");
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    })
}

pub(crate) fn completion_marker_strings() -> &'static [String] {
    static CELL: OnceLock<Vec<String>> = OnceLock::new();
    CELL.get_or_init(|| {
        string_list_field(
            routing_signal_markers_json(),
            "completion_execution_markers",
        )
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
    })
}

pub(crate) fn supervisor_marker_strings() -> &'static [String] {
    static CELL: OnceLock<Vec<String>> = OnceLock::new();
    CELL.get_or_init(|| {
        string_list_field(
            routing_signal_markers_json(),
            "supervisor_execution_markers",
        )
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
    })
}

pub(crate) fn github_pr_standalone_token_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bpr\b").expect("static github pr token regex"))
}


pub fn has_checklist_execution_context(query_text: &str) -> bool {
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







pub fn has_rendered_visual_evidence_context(query_text: &str, query_token_list: &[String]) -> bool {
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

pub fn has_existing_image_file_context(query_text: &str, query_token_list: &[String]) -> bool {
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



pub fn is_overlay_record(record: &SkillRecord) -> bool {
    record.owner_lower == "overlay"
}

pub fn can_be_primary_owner(record: &SkillRecord) -> bool {
    if !record.primary_allowed {
        return false;
    }
    record.gate_lower == "none"
        && !framework_alias_requires_explicit_call(record)
        && !matches!(record.owner_lower.as_str(), "gate" | "overlay")
}

pub fn can_be_fallback_owner(record: &SkillRecord) -> bool {
    can_be_primary_owner(record)
        && !matches!(
            record.fallback_policy_mode.as_str(),
            "never" | "explicit-only"
        )
}

/// High-precision **framework plan-mode** intent (aligned with `skills/plan-mode/SKILL.md`),
/// host-neutral: not tied to Cursor product name.
/// Used to keep delegation-gate admission from overriding the `plan-mode` owner on first-turn routing.
pub fn has_plan_mode_owner_context(query_text: &str, query_token_list: &[String]) -> bool {
    query_text.contains("cursor plan")
        || query_text.contains("Cursor Plan")
        || query_text.contains("CreatePlan")
        || query_text.contains("plan_profile")
        || query_text.contains("plan-mode")
        || query_text.contains("skill/plan-mode")
        || query_text.contains("plan 模式")
        || query_text.contains("策划文档闸门")
        || text_matches_phrase(query_token_list, "可验收 todo")
        || text_matches_phrase(query_token_list, "subagent 审 plan")
        || text_matches_phrase(query_token_list, "gitx plan 收口")
        || text_matches_phrase(query_token_list, "计划对照实际")
        || text_matches_phrase(query_token_list, "独立上下文 review 计划")
        || (query_text.contains("可验收") && query_text.contains("todo"))
}









pub(crate) fn detect_research_directory(cwd: &std::path::Path) -> bool {
    cwd.ancestors().any(|dir| {
        dir.join("research-state.yaml").is_file() || dir.join(".research.toml").is_file()
    })
}

/// Detect research workspace context: keyword match + directory-based detection.
///
/// Returns true when the query contains research-log keywords OR when the
/// current working directory (or an ancestor) contains a `research-state.yaml`
/// or `.research.toml` marker file. Directory detection is re-evaluated on
/// each call (single `stat` per ancestor — negligible cost).
pub fn has_research_context(query_text: &str, query_token_list: &[String]) -> bool {
    let from_keywords = has_signal_by_name("research_context", query_text, query_token_list);
    if from_keywords {
        return true;
    }
    // No caching: directory detection is cheap (2 is_file per ancestor) and
    // caching would miss user-initiated "cd" across directories during a session.
    std::env::current_dir()
        .is_ok_and(|cwd| detect_research_directory(&cwd))
}

/// True when the query is about reviewing/checking a mathematical proof or derivation,
/// without a full-paper/manuscript context. Helps route pure math-review to `math-derivation`
/// instead of `paper-reviewer`.




pub fn has_ci_failure_context(query_text: &str, query_token_list: &[String]) -> bool {
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
        "修复 ci",
        "ci 修复",
        "模板编译失败",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    });
    phrase_match || query_token_list.iter().any(|token| token == "ci")
}


pub fn should_route_to_gh_fix_ci(query_text: &str, query_token_list: &[String]) -> bool {
    has_ci_failure_context(query_text, query_token_list)
        && (has_github_pr_context(query_text, query_token_list)
            || !has_non_github_ci_provider_context(query_text, query_token_list))
}













pub fn artifact_gate_matches_query(query_token_list: &[String]) -> bool {
    ARTIFACT_GATE_PHRASES
        .iter()
        .any(|phrase| text_matches_phrase(query_token_list, phrase))
}

pub fn artifact_gate_target_slug(query_token_list: &[String]) -> Option<&'static str> {
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













pub fn build_route_context(query_text: &str, query_token_list: &[String]) -> RouteContextPayload {
    let completion_requested = completion_marker_strings().iter().any(|marker| {
        query_text.contains(marker.as_str()) || text_matches_phrase(query_token_list, marker)
    });
    let supervisor_required = supervisor_marker_strings().iter().any(|marker| {
        query_text.contains(marker.as_str()) || text_matches_phrase(query_token_list, marker)
    });
    let delegation_candidate = has_bounded_subagent_context(query_text, query_token_list)
        || has_workflow_orchestration_context(query_text, query_token_list)
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
    .any(|marker| {
        if matches!(*marker, "review") {
            return text_matches_phrase(query_token_list, marker);
        }
        query_text.contains(*marker) || text_matches_phrase(query_token_list, marker)
    });
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
mod paper_prose_edit_context_tests {
    use super::*;
    use crate::route::tokenize_route_text;

    pub(crate) fn prose(text: &str) -> bool {
        let tokens = tokenize_route_text(text);
        has_paper_prose_edit_context(text, &tokens)
    }

    #[test]
    pub(crate) fn standalone_sci_polish_abstract() {
        assert!(prose("SCI润色 abstract"));
    }

    #[test]
    pub(crate) fn polish_this_abstract_without_paper_word() {
        assert!(prose("polish this abstract for clarity"));
    }

    #[test]
    pub(crate) fn colloquial_edit_with_paper_context() {
        assert!(prose("论文讨论节这段读起来不通顺，帮我改改"));
    }

    #[test]
    pub(crate) fn pasted_latex_block_with_paper_context() {
        assert!(prose(
            "论文 改一下下面这段 \\begin{abstract} We propose a method \\cite{foo}"
        ));
    }

    #[test]
    pub(crate) fn pasted_latex_without_paper_context_is_false() {
        assert!(!prose(
            "fix \\begin{abstract} in CI workflow for the template"
        ));
    }

    #[test]
    pub(crate) fn negative_edit_abstract_base_class() {
        assert!(!prose("edit the abstract base class in this Java module"));
    }

    #[test]
    pub(crate) fn negative_cargo_test_only() {
        assert!(!prose("fix cargo test in pull request workflow"));
    }

    #[test]
    pub(crate) fn review_plus_polish_not_blocked() {
        assert!(prose("审稿并润色这篇论文的 abstract"));
    }
}

#[cfg(test)]
mod paper_review_slice_context_tests {
    use super::*;
    use crate::route::tokenize_route_text;

    #[test]
    pub(crate) fn preview_does_not_trigger_figure_layout_review() {
        let q = "论文 preview 图表";
        let tokens = tokenize_route_text(q);
        assert!(!has_paper_figure_layout_review_context(q, &tokens));
    }

    #[test]
    pub(crate) fn figure_layout_review_matches_review_token() {
        let q = "论文 figure layout 只 review 排版";
        let tokens = tokenize_route_text(q);
        assert!(has_paper_figure_layout_review_context(q, &tokens));
    }
}

#[cfg(test)]
mod github_pr_context_tests {
    use super::*;
    use crate::route::tokenize_query;

    #[test]
    pub(crate) fn github_pr_context_does_not_match_preview_primary() {
        let q = "preview the layout before deploy";
        let tok = tokenize_query(q);
        assert!(!has_github_pr_context(q, &tok));
        let q2 = "primary owner for the module";
        let tok2 = tokenize_query(q2);
        assert!(!has_github_pr_context(q2, &tok2));
    }

    #[test]
    pub(crate) fn github_pr_context_matches_pr_token_and_phrase() {
        let spaced = "please triage my pr now";
        let tok = tokenize_query(spaced);
        assert!(has_github_pr_context(spaced, &tok));
        let spaced2 = "please triage pr fixes";
        let tok2 = tokenize_query(spaced2);
        assert!(has_github_pr_context(spaced2, &tok2));
    }
}

#[cfg(test)]
mod research_context_tests {
    use super::*;
    use crate::route::tokenize_route_text;

    #[test]
    fn research_context_matches_research_log_keyword() {
        let q = "科研日志";
        let tokens = tokenize_route_text(q);
        assert!(has_research_context(q, &tokens));
    }

    #[test]
    pub(crate) fn research_context_matches_research_workspace_keyword() {
        let q = "研究工作区";
        let tokens = tokenize_route_text(q);
        assert!(has_research_context(q, &tokens));
    }

    #[test]
    pub(crate) fn research_context_matches_experiment_log_keyword() {
        let q = "帮我记录一下今天的实验记录";
        let tokens = tokenize_route_text(q);
        assert!(has_research_context(q, &tokens));
    }

    #[test]
    pub(crate) fn research_context_normal_query_no_false_positive() {
        let q = "帮我修复这个 bug";
        let tokens = tokenize_route_text(q);
        assert!(!has_research_context(q, &tokens));
    }

    #[test]
    pub(crate) fn research_context_english_research_log() {
        let q = "record a research log entry for today";
        let tokens = tokenize_route_text(q);
        assert!(has_research_context(q, &tokens));
    }

    #[test]
    pub(crate) fn detect_research_directory_finds_state_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("research-state.yaml"), "project: test").unwrap();
        assert!(detect_research_directory(dir.path()));
    }

    #[test]
    pub(crate) fn detect_research_directory_finds_toml_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".research.toml"), "[research]\nenabled = true").unwrap();
        assert!(detect_research_directory(dir.path()));
    }

    #[test]
    pub(crate) fn detect_research_directory_scans_ancestors() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.path().join("research-state.yaml"), "project: test").unwrap();
        assert!(detect_research_directory(&sub));
    }

    #[test]
    pub(crate) fn detect_research_directory_no_marker_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!detect_research_directory(dir.path()));
    }
}