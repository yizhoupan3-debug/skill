//! Shared Cursor/Codex hook heuristics (no host-specific JSON/event dispatch).
//! Dependency direction: `cursor_hooks` / `codex_hooks` → `hook_common` only.

use regex::Regex;
use std::sync::OnceLock;

fn compile_patterns(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|p| Regex::new(p).expect("invalid regex"))
        .collect()
}

fn review_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)\b(code|security|architecture|architect)\s+review\b",
            r"(?i)\breview\s+this\s+(pr|pull request)\b",
            r"(?i)\breview\s+(my\s+)?(pr|pull request)\b",
            r"(?i)\b(pr|pull request)\s+review\b",
            r"(?i)\breview\s+(code|security|architecture)\b",
            r"(?i)^\s*review\b.*\bagain\b",
            r"(?i)\bfocus on finding\b.*\bproblems\b",
            r"(?i)(深度|全面|全仓|仓库级|跨模块|多模块|多维)\s*review",
            r"(?i)review.*(仓库|全仓|跨模块|多模块|严重程度|findings|severity|repo|repository|cross[- ]module|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
            r"(?i)(深度|全面|全仓|仓库级|跨模块|多模块|多维).*(审查|审核|审计|评审)",
            r"(?i)(审查|审核|审计|评审).*(仓库|全仓|跨模块|多模块|严重程度|回归风险|架构风险|实现质量|路由系统|skill\s*边界)",
            r"(?i)(代码审查|安全审查|架构审查|审查这个\s*PR|审查这段代码)",
            r"(?i)(审查|评审|审核).*(PR|pull request|合并请求)",
        ])
    })
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

fn override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)do not use (a )?subagent",
            r"(?i)without (a )?subagent",
            r"(?i)handle (this|it) locally",
            r"(?i)do it yourself",
            r"(?i)no (parallel|delegation|delegating|split)",
            r"(?i)不要.*subagent",
            r"(?i)不用.*subagent",
            r"(?i)不要.*子代理",
            r"(?i)不用.*子代理",
            r"(?i)(你|你自己).*(本地处理|直接处理|自己做)",
            r"(?i)(不要|不用).*(分工|并行|分路|分头)",
        ])
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

pub fn is_review_prompt(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    if is_narrow_review_prompt(&sanitized) {
        return false;
    }
    review_patterns().iter().any(|p| p.is_match(&sanitized))
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
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    override_patterns().iter().any(|p| p.is_match(&sanitized))
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

/// 用户粘贴的 goal/续跑清门行前缀（分段拼接，避免在源码检索里出现完整误拼 token）。
const PASTED_LINE_AG_FOLLOWUP_PREFIX: &str = concat!("ag", "_followup");
const PASTED_LINE_LEGACY_REVIEW_GATE_FOLLOWUP_PREFIX: &str = concat!("rg", "_followup");

/// Recognize Codex/Cursor gate clearance: bounded subagent **`reject_reason` tokens**, `rg_clear`,
/// plus **paste-style** legacy followup prefixes **only when they appear in the user's turn**.
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

/// 用户把机读续跑行贴回输入框（含真源 `ag_followup` 行或历史上误用的 review-gate 前缀）。
/// **仅检查用户本轮提交**，不得用整会话 scrape，否则助手自拟 `rg_followup` 会误清门。
fn pasted_followup_line_clear_in_user_turn_only(user_turn_text: &str) -> bool {
    for raw_line in user_turn_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        if lower.starts_with(PASTED_LINE_LEGACY_REVIEW_GATE_FOLLOWUP_PREFIX)
            || lower.starts_with(PASTED_LINE_AG_FOLLOWUP_PREFIX)
        {
            return true;
        }
    }
    false
}

pub fn normalize_subagent_type(value: Option<&str>) -> String {
    value
        .map(|s| s.trim().to_lowercase().replace('_', "-"))
        .unwrap_or_default()
}

pub fn normalize_tool_name(value: Option<&str>) -> String {
    value.map(|s| s.trim().to_lowercase()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saw_reject_reason_accepts_line_only_tokens_and_rg_clear() {
        assert!(saw_reject_reason("small_task", ""));
        assert!(saw_reject_reason("\n  SMALL_TASK  \n", ""));
        assert!(saw_reject_reason("rg_clear", ""));
        assert!(saw_reject_reason("/rg_clear", ""));
        assert!(!saw_reject_reason("small_tasking", ""));
    }

    #[test]
    fn saw_reject_reason_ignores_fake_rg_followup_in_scrape_not_in_user_turn() {
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
            saw_reject_reason("ok", bad.as_str()),
            "user paste of legacy line in their turn must still clear gate"
        );
    }
}
