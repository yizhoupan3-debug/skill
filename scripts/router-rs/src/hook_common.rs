//! Shared hook heuristics for prompt/gate classification and small cross-host JSON key merges.
//! **不含**宿主 hook 的 stdin 生命周期分发、写盘或出站 JSON 投影；这类逻辑在 `cursor_hooks` / `codex_hooks` / `claude_hooks` 等模块。
//! `tool_input_value_from_map` 仅合并常见别名字段（`tool_input` / `input` / `arguments` / `parameters`），不替代各宿主的嵌套扫描或事件路由。
//! Dependency direction: `cursor_hooks` / `codex_hooks` / `claude_hooks` → `hook_common`；`hook_posttool_normalize` 不在此链上（其依赖 `cursor_hooks` 的字段 helper）。

use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;

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
/// `arguments`, `parameters`). Shared by Cursor nested stdin extraction and Claude tool parsing.
pub(crate) fn tool_input_value_from_map(obj: &Map<String, Value>) -> Option<Value> {
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
pub(crate) const COMPLETION_DETECT_EN: &[&str] =
    &["done", "finished", "completed", "succeeded", "passed"];

pub(crate) const COMPLETION_DETECT_ZH_PHRASES: &[&str] = &[
    "已完成",
    "已经完成",
    "全部完成",
    "完成了",
    "验证通过",
    "测试通过",
    "审核通过",
    "已通过",
    "搞定",
];

/// 在已剥离/未剥离的文本中查找完成宣称 token。空串直接返回 false；EN 走 ASCII 大小写不敏感，ZH 走原文子串匹配。
pub(crate) fn contains_completion_claim_token(text: &str) -> bool {
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
pub(crate) fn completion_claim_keywords_export() -> Vec<&'static str> {
    COMPLETION_DETECT_EN
        .iter()
        .chain(COMPLETION_DETECT_ZH_PHRASES.iter())
        .copied()
        .collect()
}

/// 单一来源：`EVIDENCE_INDEX.json` 单条 artifact 是否计作「成功验证」。
/// 规则：`success == true` **或** `exit_code` 取 0（i64 或 u64 皆可）。
/// `rfv_loop` 与 `autopilot_goal` 都走这里，防止两路证据口径分叉。
pub(crate) fn evidence_index_entry_implies_success(entry: &Value) -> bool {
    if entry.get("success").and_then(Value::as_bool) == Some(true) {
        return true;
    }
    match entry.get("exit_code") {
        Some(v) => v.as_i64() == Some(0) || v.as_u64() == Some(0),
        None => false,
    }
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

pub fn is_narrow_review_prompt(text: &str) -> bool {
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

fn strong_code_review_anchor(sanitized: &str, tokens: &[String]) -> bool {
    if crate::route::has_github_pr_context(sanitized, tokens) {
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
    let tokens = crate::route::tokenize_query(&sanitized);
    if crate::route::has_paper_context(&sanitized, &tokens)
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

/// Recognize Codex/Cursor gate clearance: bounded subagent **`reject_reason` tokens**, `rg_clear`,
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

/// 已 `normalize_subagent_type` 后的 lane：**Cursor / Codex** 默认为可清点 **`REVIEW_GATE` / CODEX_STOP 独立审稿** 的深度子代理 lane（与 `fork_context=false` 组合使用）。
///
/// **不是** Claude/Qoder stdio-agent reviewer lane 的超集（Claude 另含 review/critic 等）；勿把本函数结果套到 Claude 门控。
pub fn is_deep_review_gate_lane_normalized(lane: &str) -> bool {
    crate::registry_review_gate::is_deep_review_gate_lane_from_registry(lane)
}

pub fn normalize_tool_name(value: Option<&str>) -> String {
    value.map(|s| s.trim().to_lowercase()).unwrap_or_default()
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
        crate::registry_review_gate::assert_deep_review_gate_lane_matrix();
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
            !is_review_prompt(
                "cursor 对话频繁触发 claude 的 hook，深度review，我的设计是主 harness + 三个独立宿主"
            ),
            "host-hook debugging complaints should not arm the shared deep-review gate"
        );
    }

    #[test]
    fn review_subagent_gate_mdc_lists_deep_lanes_consistent_with_hook() {
        use std::path::PathBuf;

        let mdc_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.cursor/rules/review-subagent-gate.mdc");
        let mdc = std::fs::read_to_string(&mdc_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", mdc_path.display()));
        for needle in ["general-purpose", "best-of-n-runner"] {
            assert!(
                mdc.contains(needle),
                "review-subagent-gate.mdc should mention {needle}: {}",
                mdc_path.display()
            );
        }
        assert!(is_deep_review_gate_lane_normalized("general-purpose"));
        assert!(is_deep_review_gate_lane_normalized("best-of-n-runner"));
    }
}
