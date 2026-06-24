//! Shared hook heuristics for prompt/gate classification and small cross-host JSON key merges.
//! **不含**宿主 hook 的 stdin 生命周期分发、写盘或出站 JSON 投影；这类逻辑在 `cursor_hooks` / `codex_hooks` / `claude_hooks` 等模块。
//! `tool_input_value_from_map` 仅合并常见别名字段（`tool_input` / `input` / `arguments` / `parameters`），不替代各宿主的嵌套扫描或事件路由。
//! Dependency direction: `cursor_hooks` / `codex_hooks` / `claude_hooks` → `hook_common`；`hook_posttool_normalize` 不在此链上（其依赖 `cursor_hooks` 的字段 helper）。

use regex::Regex;
use serde_json::{Map, Value};
use std::cell::Cell;
use std::sync::OnceLock;

thread_local! {
    static TEST_INTERACTIVE_OVERRIDE: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Test-only override for [`is_interactive_profile`] (also used by `router-rs` host hook tests).
/// Thread-local so parallel `#[test]` threads do not race.
#[doc(hidden)]
pub fn set_test_interactive_override(v: Option<bool>) {
    TEST_INTERACTIVE_OVERRIDE.with(|c| c.set(v));
}

/// Default UTF-8 **char** budget for assistant text on hook signal / lint paths (all hosts).
pub const HOOK_SIGNAL_ASSISTANT_TAIL_CHARS: usize = 4096;

/// Truncate assistant text for hook signal paths (char-based; matches deep-continuation tail style).
pub fn hook_assistant_tail_window(raw: &str, max_chars: usize) -> String {
    // Single-pass char_indices() instead of two full chars() traversals (O(n) → O(n) but faster).
    let total = raw.chars().count();
    if total <= max_chars {
        return raw.to_string();
    }
    let omitted = total.saturating_sub(max_chars);
    let byte_start = raw
        .char_indices()
        .nth(omitted)
        .map(|(i, _)| i)
        .unwrap_or(raw.len());
    let tail = &raw[byte_start..];
    format!("[...omitted {omitted} chars...]\n{tail}")
}

fn compile_patterns(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|p| Regex::new(p).expect("invalid regex"))
        .collect()
}

fn review_patterns() -> &'static [Regex] {
    crate::review_routing_signals::review_gate_compiled_regexes()
}

fn parallel_delegation_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)(并行|同时|分头|分路|分三路|多路|多线).*(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证|模块|方向)",
            r"(?i)(前端|后端|测试|API|数据库|UI|安全|性能|架构|实现|策略|验证).*(并行|同时|分头|分路|分三路|多路|多线)",
            r"(?i)(多个|多条|多路|多维|多方向|独立).*(假设|模块|方向|维度|lane|lanes)",
            r"(?i)\b(parallel|concurrent|in parallel|split lanes|split work)\b.*\b(frontend|backend|test|testing|database|security|performance|architecture|implementation|verification|worker|workers)\b",
            r"(?i)(并行|分路|分头|独立).*(lane|路线|路)",
        ])
    })
}

fn parallel_marker_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(parallel|concurrent|in parallel|split lanes?|independent lanes?|split work)\b|(并行|同时|分头|分路|多路|多线|独立)")
            .expect("invalid regex")
    })
}

fn task_context_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(implement|build|run|execute|refactor|migrate|fix|change|ship)\b|(实现|执行|运行|构建|改|修|重构|迁移)")
            .expect("invalid regex")
    })
}

fn capability_domain_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(frontend|backend|test|testing|api|database|ui|security|performance|architecture|implementation|verification|module|lane|lanes)\b|(前端|后端|测试|数据库|安全|性能|架构|模块|方向)")
            .expect("invalid regex")
    })
}

fn review_override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)do not use (a )?subagent",
            r"(?i)without (a )?subagent",
            r"(?i)handle (this|it) locally",
            r"(?i)do it yourself",
            r"(?i)不要.*subagent",
            r"(?i)不用.*subagent",
            r"(?i)不要.*子代理",
            r"(?i)不用.*子代理",
            r"(?i)(你|你自己).*(本地处理|直接处理|自己做)",
        ])
    })
}

fn delegation_override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)no (parallel|delegation|delegating|split)",
            r"(?i)(不要|不用).*(分工|并行|分路|分头)",
        ])
    })
}

fn reject_reason_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            "small_task",
            "shared_context_heavy",
            "write_scope_overlap",
            "next_step_blocked",
            "verification_missing",
            "token_overhead_dominates",
        ]
        .iter()
        .map(|reason| {
            Regex::new(&format!(
                "(?i)(^|[^a-z0-9_])({})($|[^a-z0-9_])",
                regex::escape(reason)
            ))
            .expect("invalid reject regex")
        })
        .collect()
    })
}

/// Merge hook payloads' tool argument object from common alternate keys (`tool_input`, `input`,
/// `arguments`, `parameters`). Shared by all hosts' nested stdin extraction and tool parsing.
pub fn tool_input_value_from_map(obj: &Map<String, Value>) -> Option<Value> {
    obj.get("tool_input")
        .or_else(|| obj.get("input"))
        .or_else(|| obj.get("arguments"))
        .or_else(|| obj.get("parameters"))
        .cloned()
}

/// 与 `reject_reason_patterns` 同步；用于「整行仅 token」时的精确匹配（规避极少数 Unicode 边界与宿主格式差异）。
const REJECT_REASON_LINE_TOKENS: &[&str] = &[
    "small_task",
    "shared_context_heavy",
    "write_scope_overlap",
    "next_step_blocked",
    "verification_missing",
    "token_overhead_dominates",
];

/// 显式操作符：仅当单独成行（trim 后全串匹配）时生效，避免在正常句子里误触发。
const REVIEW_GATE_LINE_CLEAR_MARKERS: &[&str] = &["rg_clear", "/rg_clear"];

/// 完成宣称 token：英文词 + 中文短语。**Single source of truth**：
///
/// - `closeout_enforcement::summary_claims_completion` 直接对 summary 原文扫描；
/// - `cursor_hooks::completion_claimed_in_text` 先剥离引文 / 代码块 / URL 再扫描；
/// - 中文用多字短语避免「完成度 / 完成任务拆分」等子串误命中。
pub const COMPLETION_DETECT_EN: &[&str] = &["done", "finished", "completed", "succeeded", "passed"];

/// 完成宣称（触发 closeout）：不含「验证通过/测试通过」等，避免与 goal verify 聊天轨打架；
/// 避免「完成度 / 完成任务拆分」等子串误命中。
pub const COMPLETION_DETECT_ZH_PHRASES: &[&str] =
    &["已完成", "已经完成", "全部完成", "完成了", "搞定", "完成", "通过"];

/// 仅用于无磁盘 GOAL 时的聊天 progress/verify 提示（不进 closeout 词表）。
pub const GOAL_CHAT_VERIFY_ZH_PHRASES: &[&str] = &["验证通过", "测试通过", "审核通过", "已通过"];

// ────────────────────────────────────────────────────────────────
// Goal signal detection (shared across all hosts)
// ────────────────────────────────────────────────────────────────

/// Regex for goal contract keywords (EN + ZH).
pub fn goal_contract_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(goal|done when|validation commands|checkpoint plan|non-goals)\b|(目标|完成条件|验证命令|检查点|非目标)",
        )
        .expect("invalid regex")
    })
}

/// Regex for goal progress keywords (EN + ZH).
pub fn goal_progress_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(checkpoint|milestone|progress|next step)\b|(检查点|里程碑|进度|下一步)")
            .expect("invalid regex")
    })
}

/// Regex for goal verify/block keywords (EN + ZH).
pub fn goal_verify_or_block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(verified|verification|test passed|blocker)\b|(已验证|阻塞)")
            .expect("invalid regex")
    })
}

/// Check if text contains goal progress signal.
pub fn has_goal_progress_signal(text: &str) -> bool {
    goal_progress_re().is_match(text)
}

/// Check if text contains goal verify or blocker signal.
pub fn has_goal_verify_or_block_signal(text: &str) -> bool {
    goal_verify_or_block_re().is_match(text)
        || GOAL_CHAT_VERIFY_ZH_PHRASES
            .iter()
            .any(|p| text.contains(p))
}

/// Check for structured goal contract: Goal + Non-goals + Validation commands headings
/// all non-empty, plus at least 2 "Done when" items (EN/ZH).
pub fn has_structured_goal_contract(text: &str) -> bool {
    let goal_ok =
        nonempty_inline_heading_any(text, "Goal") || nonempty_inline_heading_any(text, "目标");
    let non_goals_ok = nonempty_inline_heading_any(text, "Non-goals")
        || nonempty_inline_heading_any(text, "非目标");
    let validation_ok = nonempty_inline_heading_any(text, "Validation commands")
        || nonempty_inline_heading_any(text, "验证命令");
    let done_when_items = count_done_when_items(text);
    if goal_ok && non_goals_ok && validation_ok && done_when_items >= 2 {
        return true;
    }
    // Fallback: if the text passes the heuristic complexity analyzer, treat it
    // as having an implicit contract (supplementary signal for auto-detected goals).
    crate::goal_auto_detect::analyze_complexity(text).is_complex
}

/// Check if a heading has non-empty inline content after `:`.
pub fn nonempty_inline_heading_any(text: &str, heading: &str) -> bool {
    use std::sync::LazyLock;

    static GOAL_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?im)^\s*Goal\s*[:：]\s*(\S.+)$").expect("invalid heading regex")
    });
    static NON_GOALS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?im)^\s*Non-goals\s*[:：]\s*(\S.+)$").expect("invalid heading regex")
    });
    static VALIDATION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?im)^\s*Validation commands\s*[:：]\s*(\S.+)$").expect("invalid heading regex")
    });
    static GOAL_ZH_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?im)^\s*目标\s*[:：]\s*(\S.+)$").expect("invalid heading regex")
    });
    static NON_GOALS_ZH_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?im)^\s*非目标\s*[:：]\s*(\S.+)$").expect("invalid heading regex")
    });
    static VALIDATION_ZH_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?im)^\s*验证命令\s*[:：]\s*(\S.+)$").expect("invalid heading regex")
    });

    let re = match heading {
        "Goal" => &*GOAL_RE,
        "Non-goals" => &*NON_GOALS_RE,
        "Validation commands" => &*VALIDATION_RE,
        "目标" => &*GOAL_ZH_RE,
        "非目标" => &*NON_GOALS_ZH_RE,
        "验证命令" => &*VALIDATION_ZH_RE,
        _ => return false,
    };

    re.captures(text)
        .and_then(|cap| cap.get(1))
        .map(|m| !m.as_str().trim().is_empty())
        .unwrap_or(false)
}

/// Count "Done when" items: prefer bullet/numbered items, fallback to inline separators.
fn count_done_when_items(text: &str) -> usize {
    const HEADINGS: [&str; 2] = ["Done when", "完成条件"];
    static NUMBERED_LINE_RE: OnceLock<Regex> = OnceLock::new();
    static RE_DONE: OnceLock<Regex> = OnceLock::new();
    static RE_ZH: OnceLock<Regex> = OnceLock::new();
    let numbered_line_re = NUMBERED_LINE_RE
        .get_or_init(|| Regex::new(r"(?m)^\d+\.\s+\S").expect("invalid numbered line regex"));
    let re_done = RE_DONE.get_or_init(|| {
        Regex::new(&format!(
            r"(?im)^\s*{}\s*[:：]\s*(.*)$",
            regex::escape(HEADINGS[0])
        ))
        .expect("invalid done regex")
    });
    let re_zh = RE_ZH.get_or_init(|| {
        Regex::new(&format!(
            r"(?im)^\s*{}\s*[:：]\s*(.*)$",
            regex::escape(HEADINGS[1])
        ))
        .expect("invalid zh regex")
    });
    let heading_pairs = [
        (HEADINGS[0], Some(re_done)),
        (HEADINGS[1], Some(re_zh)),
    ];
    for (h, maybe_re) in heading_pairs {
        let Some(re) = maybe_re else {
            continue;
        };
        let Some(cap) = re.captures(text) else {
            continue;
        };
        let inline = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        if !inline.is_empty() {
            let parts = inline
                .split(&[';', '；', ',', '，', '|', '、'][..])
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .count();
            if parts >= 2 {
                return parts;
            }
        }

        let mut in_section = false;
        let mut count = 0usize;
        let h_lower = h.to_ascii_lowercase();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() {
                if in_section {
                    continue;
                }
                continue;
            }
            if !in_section {
                let lowered = line.to_ascii_lowercase();
                if lowered.starts_with(&h_lower) && (lowered.contains(':') || line.contains('：')) {
                    in_section = true;
                }
                continue;
            }

            if goal_contract_re().is_match(line)
                && !line
                    .to_ascii_lowercase()
                    .starts_with(&h_lower)
            {
                break;
            }

            let is_bullet = line.starts_with("- ")
                || line.starts_with("* ")
                || line.starts_with("• ")
                || numbered_line_re.is_match(line);
            if is_bullet {
                count += 1;
            }
        }
        if count > 0 {
            return count;
        }
    }
    0
}

/// 在已剥离/未剥离的文本中查找完成宣称 token。空串直接返回 false；EN 走 ASCII 大小写不敏感，ZH 走原文子串匹配。
pub fn contains_completion_claim_token(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if COMPLETION_DETECT_EN
        .iter()
        .any(|kw| lower.contains(&kw.to_ascii_lowercase()))
    {
        return true;
    }
    COMPLETION_DETECT_ZH_PHRASES
        .iter()
        .any(|p| text.contains(p))
}

/// Contract JSON 导出：EN 词 ++ ZH 短语，保持原 `COMPLETION_KEYWORDS` 的顺序契约。
pub fn completion_claim_keywords_export() -> Vec<&'static str> {
    COMPLETION_DETECT_EN
        .iter()
        .chain(COMPLETION_DETECT_ZH_PHRASES.iter())
        .copied()
        .collect()
}

fn review_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\breview\b").expect("invalid regex"))
}

fn pr_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(pr|pull request)\b").expect("invalid regex"))
}

fn deep_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(深度|全面|全仓|跨模块|多模块|多维|架构|安全|回归风险|严重程度|findings)")
            .expect("invalid regex")
    })
}

fn narrow_review_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*review\s+(/|\.|[A-Za-z0-9_-].*\.(md|rs|tsx?|jsx?|py|json|toml))")
            .expect("invalid regex")
    })
}

pub fn strip_quoted_or_codeblock_or_url(text: &str) -> String {
    static RE_FENCED: OnceLock<Regex> = OnceLock::new();
    static RE_INLINE: OnceLock<Regex> = OnceLock::new();
    static RE_URL: OnceLock<Regex> = OnceLock::new();
    static RE_BLOCKQUOTE: OnceLock<Regex> = OnceLock::new();
    static RE_QUOTED: OnceLock<Regex> = OnceLock::new();
    let mut cleaned = text.to_string();
    cleaned = RE_FENCED
        .get_or_init(|| Regex::new(r"(?s)```.*?```").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned();
    cleaned = RE_INLINE
        .get_or_init(|| Regex::new(r"`[^`\n]*`").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned();
    cleaned = RE_URL
        .get_or_init(|| Regex::new(r"https?://\S+").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned();
    cleaned = RE_BLOCKQUOTE
        .get_or_init(|| Regex::new(r"(?m)^\s*>\s.*$").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned();
    RE_QUOTED
        .get_or_init(|| Regex::new("\"[^\"\\n]*\"").expect("invalid regex"))
        .replace_all(&cleaned, " ")
        .into_owned()
}

fn framework_non_goal_entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(^|\s)/(gitx|update)\b").expect("invalid regex"))
}

/// Framework slash commands that may arm delegation (excludes goal-only entries).
pub fn is_framework_non_goal_entrypoint_prompt(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    framework_non_goal_entry_re().is_match(&sanitized)
}

/// True when the current session is in an interactive profile.
///
/// Interactive profiles suppress review-gate hard block,
/// disable spawn-first nudge, and reject being scheduled by the Loop Engine.
///
/// Detection (in priority order):
/// 1. Thread-local `TEST_INTERACTIVE_OVERRIDE` (testing only)
/// 2. (Future) GOAL_STATE.lifecycle_profile == "interactive" via repo_root
///
/// Cf. docs/architecture.md §1.2 (hook model)
pub fn is_interactive_profile(repo_root: Option<&std::path::Path>, _text: &str) -> bool {
    if let Some(v) = TEST_INTERACTIVE_OVERRIDE.with(|c| c.get()) {
        return v;
    }
    let Some(_root) = repo_root else {
        return false;
    };
    // Single-conversation mode: no pointer fallback for goal state lookup.
    // (Future: check GOAL_STATE.lifecycle_profile for "interactive")
    false
}

pub fn is_narrow_review_prompt(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("small_task")
        || trimmed.starts_with("small_task\n")
        || trimmed.starts_with("small_task ")
    {
        return true;
    }
    if trimmed.contains("不用子代理") || trimmed.to_ascii_lowercase().contains("no subagent") {
        return true;
    }
    if !review_keyword_re().is_match(text) {
        return false;
    }
    if pr_keyword_re().is_match(text) {
        return false;
    }
    if deep_keyword_re().is_match(text) {
        return false;
    }
    narrow_review_prefix_re().is_match(text)
}

/// Whether beforeSubmit may inject the subagent model inherit one-liner (independent of interactive / REVIEW_GATE).
pub fn should_inject_subagent_model_inherit_nudge(
    prompt_text: &str,
    user_gate_override: bool,
    goal_drive_entrypoint: bool,
    delegation: bool,
    review: bool,
) -> bool {
    if !crate::env_flags::router_rs_subagent_model_inherit_nudge_enabled() {
        return false;
    }
    if user_gate_override {
        return false;
    }
    if is_narrow_review_prompt(prompt_text) {
        return false;
    }
    goal_drive_entrypoint || delegation || review
}

/// Whether hooks may inject the spawn-first pairing reviewer one-liner.
pub fn should_inject_spawn_first_review_nudge(
    repo_root: Option<&std::path::Path>,
    prompt_text: &str,
) -> bool {
    if is_interactive_profile(repo_root, prompt_text)
    {
        // Interactive profiles suppress spawn-first nudge.
        // The "interactive" profile entry in RUNTIME_REGISTRY.json always
        // has disable_spawn_first_nudge: true.
        return false;
    }
    crate::env_flags::router_rs_review_spawn_first_nudge_enabled()
        && crate::registry_review_gate::review_spawn_first_enabled(repo_root)
}

/// Global posture: REVIEW_GATE at Stop is advisory-only on all hosts (no `continue:false` /
/// `decision:block` for review incomplete). L4 handlers use [`review_gate_stop_would_nudge`] for
/// nudge injection; closeout may still hard-block when `ROUTER_RS_CLOSEOUT_ENFORCEMENT` applies.
pub fn review_gate_advisory_only() -> bool {
    true
}

/// Whether the full review-gate Stop path is suppressed (interactive profile or env disable via host
/// `*_review_gate_suppressed`): skips arming **and** Stop nudges, not merely hard block.
/// When [`review_gate_advisory_only`] is true, non-suppressed hosts still inject advisory text.
pub fn review_gate_hard_block_disabled(repo_root: Option<&std::path::Path>, text: &str) -> bool {
    is_interactive_profile(repo_root, text)
}

/// True when an armed review gate would inject a Stop nudge (metrics / advisory detection).
/// Does **not** imply hard block when [`review_gate_advisory_only`] is true.
pub fn review_gate_stop_would_nudge(
    review_required: bool,
    review_override: bool,
    independent_reviewer_seen: bool,
) -> bool {
    crate::review_gate_engine::review_gate_blocks_stop(crate::review_gate_engine::ReviewGateFacts {
        review_required,
        review_override,
        independent_reviewer_seen,
    })
}

/// True when the lifecycle_profile is "loop-auto" (loop-capable).
///
/// Reads from LOOP_REGISTRY.json (not GOAL_STATE). Loop Runner in PREFLIGHT
/// phase calls this to confirm the entry profile is schedulable.
///
/// Cf. docs/architecture.md §1.2 (hook model)
pub fn lifecycle_profile_is_loop_capable(profile: &str) -> bool {
    profile == "loop-auto"
}

fn strong_code_review_anchor(sanitized: &str, tokens: &[String]) -> bool {
    if crate::review_context_signals::has_github_pr_context(sanitized, tokens) {
        return true;
    }
    if sanitized.contains("路由系统") || sanitized.contains("代码库") {
        return true;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(codebase|repo|repository|skill)\b").expect("invalid anchor regex")
    })
    .is_match(sanitized)
}

pub fn is_review_prompt(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    if is_narrow_review_prompt(&sanitized) {
        return false;
    }
    let matched = review_patterns().iter().any(|p| p.is_match(&sanitized));
    if !matched {
        return false;
    }
    let tokens = framework_kernel::tokenize_query(&sanitized);
    if crate::review_context_signals::has_paper_context(&sanitized, &tokens)
        && !strong_code_review_anchor(&sanitized, &tokens)
    {
        return false;
    }
    true
}

pub fn is_parallel_delegation_prompt(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    let matched = parallel_delegation_patterns()
        .iter()
        .any(|p| p.is_match(&sanitized));
    if !matched {
        return false;
    }
    if parallel_marker_re().is_match(&sanitized) {
        return task_context_re().is_match(&sanitized)
            || capability_domain_re().is_match(&sanitized);
    }
    true
}

pub fn has_override(text: &str) -> bool {
    has_review_override(text) || has_delegation_override(text)
}

pub fn has_review_override(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    review_override_patterns()
        .iter()
        .any(|p| p.is_match(&sanitized))
}

pub fn has_delegation_override(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    delegation_override_patterns()
        .iter()
        .any(|p| p.is_match(&sanitized))
}

/// 用户粘贴的 goal 续跑清门行前缀（分段拼接，避免在源码检索里出现完整误拼 token）。
/// **不含** `rg_followup`：该形态与 harness 文档中的「仿冒机读行」一致，若允许用户粘贴清门会鼓励误用模型自拟行；清门请用 `rg_clear`、拒因 token 或自然语言 override。
const PASTED_LINE_AG_FOLLOWUP_PREFIX: &str = concat!("ag", "_followup");

/// Recognize host gate clearance: bounded subagent **`reject_reason` tokens**, `rg_clear`,
/// plus **paste-style** `ag_followup` leader **only when it appears in the user's turn**.
///
/// # Why split `signal_text` vs `user_turn_text`
///
/// Cursor `signal_text` often includes `hook_event_all_text` (conversation scrape). Assistants sometimes
/// fabricate bogus two-letter imitation follow-up blocks; matching those pasted-style prefixes globally would falsely clear
/// the gate (`pre_goal_review_satisfied`, escalation counters), which encourages the hallucination loop the
/// host-visible policy explicitly forbids. Real host followups remain `router-rs AG_FOLLOWUP …` (injected fields).
pub fn saw_reject_reason(signal_text: &str, user_turn_text: &str) -> bool {
    if reject_reason_patterns()
        .iter()
        .any(|p| p.is_match(signal_text))
    {
        return true;
    }
    for raw_line in signal_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if REJECT_REASON_LINE_TOKENS.contains(&lower.as_str()) {
            return true;
        }
        if REVIEW_GATE_LINE_CLEAR_MARKERS.contains(&lower.as_str()) {
            return true;
        }
    }
    pasted_followup_line_clear_in_user_turn_only(user_turn_text)
}

/// 用户把 goal 相关续跑行贴回输入框（`ag_followup` 前缀，无 `router-rs ` 的粘贴兼容路径）。
/// **仅检查用户本轮提交**，不得用整会话 scrape，否则助手自拟仿机读行会误清门。
fn pasted_followup_line_clear_in_user_turn_only(user_turn_text: &str) -> bool {
    for raw_line in user_turn_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        if lower.starts_with(PASTED_LINE_AG_FOLLOWUP_PREFIX) {
            return true;
        }
    }
    false
}

pub fn normalize_subagent_type(value: Option<&str>) -> String {
    value
        .map(crate::lane_normalize::normalize_subagent_lane)
        .unwrap_or_default()
}

/// 已 `normalize_subagent_type` 后的 lane：registry `reviewer_lanes` 闭集（跨宿主 canonical）。
pub fn is_reviewer_lane_normalized(lane: &str) -> bool {
    crate::registry_review_gate::is_reviewer_lane_from_registry(lane, None)
}

/// Countable deep-review gate lane（`review_gate.reviewer_lanes` 闭集；与 [`is_reviewer_lane_normalized`] 同义）。
pub fn is_deep_review_gate_lane_normalized(lane: &str) -> bool {
    is_reviewer_lane_normalized(lane)
}

pub fn normalize_tool_name(value: Option<&str>) -> String {
    value.map(|s| s.trim().to_lowercase()).unwrap_or_default()
}

// ────────────────────────────────────────────────────────────────
// Tool vs Skill namespace isolation
// ────────────────────────────────────────────────────────────────

/// 工具来源分类，用于隔离 hook 事件处理中的 tool vs skill 边界。
///
/// - `NativeHost`：宿主内置工具（Bash, Write, Edit, Read, Agent 等）
/// - `McpServer`：MCP 工具，FQN 格式 `mcp__{server_id}__{tool_name}`
/// - `Unknown`：未识别的工具名（可能是新宿主工具或第三方扩展）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOrigin {
    NativeHost,
    McpServer {
        server_id: String,
        tool_name: String,
    },
    Unknown,
}

impl ToolOrigin {
    /// 是否为 MCP 工具
    pub fn is_mcp(&self) -> bool {
        matches!(self, ToolOrigin::McpServer { .. })
    }

    /// 是否为宿主内置工具
    pub fn is_native(&self) -> bool {
        matches!(self, ToolOrigin::NativeHost)
    }
}

/// 判断工具名是否为 MCP 工具 FQN（`mcp__{server}__{tool}`）。
pub fn is_mcp_tool_name(name: &str) -> bool {
    name.starts_with("mcp__")
}

/// 解析 MCP 工具 FQN：`mcp__{server_id}__{tool_name}`。
///
/// 使用 `rsplit_once("__")` 从右侧解析，支持 server_id 包含连字符
/// （如 `browser-mcp`、`router-rs-framework`）。
pub fn parse_mcp_tool_fqn(fqn: &str) -> Option<(&str, &str)> {
    let rest = fqn.strip_prefix("mcp__")?;
    let (server_id, tool_name) = rest.rsplit_once("__")?;
    if server_id.is_empty() || tool_name.is_empty() {
        return None;
    }
    Some((server_id, tool_name))
}

/// 已知宿主内置工具闭集（跨 Claude/Cursor/Codex/OpenCode 全覆盖）。
///
/// 包含：宿主原生工具 + 跨宿主 shell/写入/子代理工具。
fn is_known_native_tool(name: &str) -> bool {
    matches!(
        name,
        // Claude 原生工具
        "Bash" | "Write" | "Edit" | "Read" | "Agent" | "NotebookEdit"
        | "WebSearch" | "WebFetch" | "Glob" | "Grep" | "LS"
        | "SendMessage" | "Skill" | "EnterWorktree" | "ExitWorktree"
        | "TeamCreate" | "DesignSync" | "CronCreate" | "CronDelete" | "CronList"
        // 跨宿主 shell 工具（is_shell_tool 闭集）
        | "shell" | "bash" | "run_terminal_cmd" | "execute_command"
        | "terminal" | "run_command" | "sh" | "exec" | "cmd"
        // 跨宿主写入工具（is_file_write_tool 闭集）
        | "write" | "strreplace" | "str_replace" | "delete"
        | "applypatch" | "apply_patch" | "notebookedit" | "notebook_edit"
        // 子代理工具（SUBAGENT_TOOL_NAMES + 扩展）
        | "task" | "subagent" | "spawn_agent" | "dispatch_agent"
        | "functions.task" | "functions.subagent" | "functions.spawn_agent"
        | "functions.exec_command"
    )
}

/// 分类工具来源。
///
/// 优先检查 MCP FQN 格式，再检查宿主内置工具闭集，最后归为 Unknown。
pub fn classify_tool_origin(tool_name: &str) -> ToolOrigin {
    if let Some((server, tool)) = parse_mcp_tool_fqn(tool_name) {
        ToolOrigin::McpServer {
            server_id: server.to_string(),
            tool_name: tool.to_string(),
        }
    } else if is_known_native_tool(tool_name) {
        ToolOrigin::NativeHost
    } else {
        ToolOrigin::Unknown
    }
}

#[cfg(test)]
pub(crate) fn install_review_prompt_test_deps() {
    struct WhitespaceTokenizer;

    impl framework_kernel::TokenizerProvider for WhitespaceTokenizer {
        fn tokenize_query(&self, text: &str) -> Vec<String> {
            text.split_whitespace()
                .map(|s| s.to_ascii_lowercase())
                .collect()
        }

        fn has_parallel_review_candidate_context(&self, _query: &str, _tokens: &[String]) -> bool {
            false
        }
    }

    framework_kernel::install_tokenizer_provider(Box::new(WhitespaceTokenizer));
    crate::review_context_signals::install_test_review_context_probes();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_override_superset_of_review_and_delegation_overrides() {
        let delegation_only = "Please no parallel delegation on this task.";
        assert!(
            has_delegation_override(delegation_only),
            "fixture must hit delegation-only patterns"
        );
        assert!(
            !has_review_override(delegation_only),
            "delegation-only wording must not match review-override patterns alone"
        );
        assert!(
            has_override(delegation_only),
            "has_override delegates to delegation narrow matcher for this fixture"
        );

        let review_only = "Do not use a subagent.";
        assert!(has_review_override(review_only));
        assert!(!has_delegation_override(review_only));
        assert!(has_override(review_only));
    }

    #[test]
    fn deep_review_gate_lane_normalized_matches_registry_matrix() {
        crate::registry_review_gate::assert_reviewer_lane_matrix(None);
    }

    #[test]
    fn saw_reject_reason_accepts_line_only_tokens_and_rg_clear() {
        assert!(saw_reject_reason("small_task", ""));
        assert!(saw_reject_reason("\n  SMALL_TASK  \n", ""));
        assert!(saw_reject_reason("rg_clear", ""));
        assert!(saw_reject_reason("/rg_clear", ""));
        assert!(!saw_reject_reason("small_tasking", ""));
    }

    #[test]
    fn saw_reject_reason_ignores_rg_followup_in_scrape_and_user_turn() {
        let bad = format!(
            "{} {}",
            concat!("RG", "_FOLLOWUP"),
            "missing_parts=independent_escalation_line"
        );
        let scrape = format!("user asks for help\n{bad}\nmore");
        assert!(
            !saw_reject_reason(scrape.as_str(), "just the user question"),
            "assistant-hallucinated imitation follow-up must not clear gate via conversation scrape"
        );
        assert!(
            !saw_reject_reason("ok", bad.as_str()),
            "user paste of RG_FOLLOWUP imitation line must not clear gate (use rg_clear or reject_reason tokens)"
        );
    }

    #[test]
    fn saw_reject_reason_accepts_ag_followup_paste_in_user_turn_only() {
        let line = concat!("ag", "_followup", " missing_parts=checkpoint_progress");
        assert!(!saw_reject_reason("signal", "no paste"));
        assert!(saw_reject_reason("ok", line));
    }

    #[test]
    fn is_review_prompt_suppresses_manuscript_without_code_anchor_by_default() {
        install_review_prompt_test_deps();
        assert!(
            !is_review_prompt("深度 review 论文 methodology 节"),
            "manuscript + depth review should not arm code review gate without code anchor"
        );
        assert!(is_review_prompt("深度 review 整个路由系统"));
        assert!(
            !is_review_prompt("deep review this manuscript introduction"),
            "English manuscript review should not arm code review gate"
        );
        assert!(
            is_review_prompt("深度 review 整个路由系统"),
            "routing-system phrase is a strong code/framework anchor"
        );
        assert!(
            is_review_prompt("深度 review 论文 pull request 把关"),
            "PR/github anchor keeps review prompt"
        );
        assert!(is_review_prompt("Please do a code review of this change."));
        assert!(
            is_review_prompt("请全面review这个路由系统 /gitx 修复刚发现的问题"),
            "dual review+gitx with routing anchor must still count as review prompt"
        );
        assert!(
            !is_review_prompt(
                "cursor 对话频繁触发 claude 的 hook，深度review，我的设计是主 harness + 三个独立宿主"
            ),
            "host-hook debugging complaints should not arm the shared deep-review gate"
        );
    }

    #[test]
    fn should_inject_subagent_model_inherit_for_review_and_delegation() {
        let _lock = crate::test_env_sync::process_env_lock();
        let key = "ROUTER_RS_CURSOR_SUBAGENT_MODEL_INHERIT_NUDGE";
        let prev = std::env::var_os(key);
        unsafe { core_state_utils::env_sync::set_env(key, "1") };
        // Delegation arm triggers inject
        assert!(should_inject_subagent_model_inherit_nudge(
            "implement the feature",
            false,
            false,
            true,
            false
        ));
        assert!(should_inject_subagent_model_inherit_nudge(
            "Please do a code review of this change.",
            false,
            false,
            false,
            true
        ));
        assert!(!should_inject_subagent_model_inherit_nudge(
            "small_task",
            false,
            true,
            false,
            true
        ));
        match prev {
            Some(v) => unsafe { core_state_utils::env_sync::set_env(key, &v) },
            None => unsafe { core_state_utils::env_sync::remove_env(key) },
        }
    }

    #[test]
    fn narrow_review_prompt_skips_arm_for_small_task_and_single_path() {
        assert!(is_narrow_review_prompt("small_task"));
        assert!(is_narrow_review_prompt("review ./README.md"));
        assert!(!is_review_prompt("review ./README.md"));
        assert!(!is_review_prompt("small_task\nplease check one paragraph"));
        assert!(is_narrow_review_prompt("不用子代理，review src/lib.rs"));
        assert!(!is_review_prompt("不用子代理，review src/lib.rs"));
    }

    #[test]
    fn review_subagent_gate_mdc_lists_deep_lanes_consistent_with_hook() {
        use std::path::PathBuf;

        let mdc_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.rules/review-subagent-gate.mdc");
        let mdc = std::fs::read_to_string(&mdc_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", mdc_path.display()));
        for needle in ["reviewer_lanes", "fork_context"] {
            assert!(
                mdc.contains(needle),
                "review-subagent-gate.mdc should mention {needle}: {}",
                mdc_path.display()
            );
        }
        assert!(is_reviewer_lane_normalized("general-purpose"));
        assert!(is_reviewer_lane_normalized("best-of-n-runner"));
    }

    #[test]
    fn is_interactive_profile_returns_false_without_root() {
        assert!(!is_interactive_profile(None, "run the task"));
        assert!(!is_interactive_profile(None, "fix the bug"));
        assert!(!is_interactive_profile(None, "analyze architecture"));
    }

    #[test]
    fn is_interactive_profile_returns_false_for_arbitrary_prompt_without_root() {
        assert!(!is_interactive_profile(None, "深度review整个路由系统"));
        assert!(!is_interactive_profile(None, "fix the bug"));
        assert!(!is_interactive_profile(None, ""));
    }

    #[test]
    fn is_interactive_profile_test_override_takes_priority() {
        let _lock = crate::test_env_sync::process_env_lock();
        // Override to false
        set_test_interactive_override(Some(false));
        assert!(!is_interactive_profile(None, "run the task"));
        // Override to true even for non-interactive prompt
        set_test_interactive_override(Some(true));
        assert!(is_interactive_profile(None, "random text"));
        // Clear override, returns false
        set_test_interactive_override(None);
        assert!(!is_interactive_profile(None, "run the task"));
    }

    #[test]
    fn review_gate_hard_block_disabled_false_when_not_interactive() {
        let _lock = crate::test_env_sync::process_env_lock();
        set_test_interactive_override(Some(false));
        assert!(!review_gate_hard_block_disabled(None, "fix the bug"));
        assert!(!review_gate_hard_block_disabled(None, "run the task"));
        set_test_interactive_override(None);
    }

    #[test]
    fn review_gate_advisory_only_is_global() {
        assert!(review_gate_advisory_only());
    }

    #[test]
    fn review_gate_hard_block_disabled_true_when_interactive_active() {
        let _lock = crate::test_env_sync::process_env_lock();
        set_test_interactive_override(Some(true));
        // Advisory-only: interactive active → hard block disabled
        assert!(review_gate_hard_block_disabled(None, "run the task"));
        assert!(review_gate_hard_block_disabled(None, "random text"));
        set_test_interactive_override(None);
    }

    #[tokio::test]
    async fn once_lock_patterns_are_send_safe() {
        let result = tokio::task::spawn_blocking(|| {
            (
                parallel_marker_re().is_match("parallel work"),
                task_context_re().is_match("implement the feature"),
                capability_domain_re().is_match("前端开发"),
                !parallel_marker_re().is_match("hello world"),
            )
        })
            .await
            .expect("spawn_blocking");
        assert!(result.0, "parallel_marker_re");
        assert!(result.1, "task_context_re");
        assert!(result.2, "capability_domain_re");
        assert!(result.3, "negative match");
    }

    #[tokio::test]
    async fn strip_quoted_or_codeblock_or_url_is_send_safe() {
        let text = "hello `code` world ```block``` https://example.com".to_string();
        let result = tokio::task::spawn_blocking(move || strip_quoted_or_codeblock_or_url(&text))
            .await
            .expect("spawn_blocking");
        assert!(result.contains("hello"), "should preserve hello");
        assert!(result.contains("world"), "should preserve world");
        assert!(!result.contains("code"), "should strip inline code");
        assert!(!result.contains("block"), "should strip fenced code");
        assert!(!result.contains("https://"), "should strip URL");
    }

    // ── ToolOrigin / classify_tool_origin tests ──────────────────

    #[test]
    fn parse_mcp_tool_fqn_basic() {
        assert_eq!(
            parse_mcp_tool_fqn("mcp__browser-mcp__browser_click"),
            Some(("browser-mcp", "browser_click"))
        );
        assert_eq!(
            parse_mcp_tool_fqn("mcp__router-rs-framework__goal_state_manage"),
            Some(("router-rs-framework", "goal_state_manage"))
        );
        assert_eq!(
            parse_mcp_tool_fqn("mcp__paperplain__search_research"),
            Some(("paperplain", "search_research"))
        );
        assert_eq!(
            parse_mcp_tool_fqn("mcp__mcp-codegraph__codegraph_search"),
            Some(("mcp-codegraph", "codegraph_search"))
        );
    }

    #[test]
    fn parse_mcp_tool_fqn_rejects_invalid() {
        assert_eq!(parse_mcp_tool_fqn("Bash"), None);
        assert_eq!(parse_mcp_tool_fqn("mcp__"), None);
        assert_eq!(parse_mcp_tool_fqn("mcp__server__"), None);
        assert_eq!(parse_mcp_tool_fqn("mcp____tool"), None);
        assert_eq!(parse_mcp_tool_fqn(""), None);
    }

    #[test]
    fn is_mcp_tool_name_works() {
        assert!(is_mcp_tool_name("mcp__browser-mcp__browser_click"));
        assert!(!is_mcp_tool_name("Bash"));
        assert!(!is_mcp_tool_name(""));
        assert!(!is_mcp_tool_name("mcp_tool")); // single underscore
    }

    #[test]
    fn classify_tool_origin_mcp() {
        let origin = classify_tool_origin("mcp__browser-mcp__browser_click");
        assert!(origin.is_mcp());
        assert!(!origin.is_native());
        match &origin {
            ToolOrigin::McpServer { server_id, tool_name } => {
                assert_eq!(server_id, "browser-mcp");
                assert_eq!(tool_name, "browser_click");
            }
            _ => panic!("expected McpServer"),
        }
    }

    #[test]
    fn classify_tool_origin_native() {
        for tool in &["Bash", "Write", "Edit", "Read", "Agent", "shell", "bash", "task"] {
            let origin = classify_tool_origin(tool);
            assert!(origin.is_native(), "{tool} should be NativeHost");
        }
    }

    #[test]
    fn classify_tool_origin_unknown() {
        let origin = classify_tool_origin("SomeNewTool");
        assert_eq!(origin, ToolOrigin::Unknown);
    }
}
