//! Review gate heuristics — prompt detection, gate status, and nudge injection.
//!
//! Extracted from `hook_common.rs` (2026-06-25 refactor) to reduce file
//! size and clarify responsibility boundaries within `core-policy`.

use regex::Regex;
use std::sync::OnceLock;

use super::{strip_quoted_or_codeblock_or_url, compile_patterns};

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
        ]).unwrap_or_else(|e| panic!("compile parallel delegation patterns failed: {e}"))
    })
}

#[allow(clippy::expect_used)]
fn parallel_marker_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(parallel|concurrent|in parallel|split lanes?|independent lanes?|split work)\b|(并行|同时|分头|分路|多路|多线|独立)")
            .expect("invalid regex")
    })
}

#[allow(clippy::expect_used)]
fn task_context_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(implement|build|run|execute|refactor|migrate|fix|change|ship)\b|(实现|执行|运行|构建|改|修|重构|迁移)")
            .expect("invalid regex")
    })
}

#[allow(clippy::expect_used)]
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
        ]).unwrap_or_else(|e| panic!("compile review override patterns failed: {e}"))
    })
}

fn delegation_override_patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        compile_patterns(&[
            r"(?i)no (parallel|delegation|delegating|split)",
            r"(?i)(不要|不用).*(分工|并行|分路|分头)",
        ]).unwrap_or_else(|e| panic!("compile delegation override patterns failed: {e}"))
    })
}

#[allow(clippy::expect_used)]
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
pub const REVIEW_GATE_LINE_CLEAR_MARKERS: &[&str] = &["rg_clear", "/rg_clear"];

#[allow(clippy::expect_used)]
fn review_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\breview\b").expect("invalid regex"))
}

#[allow(clippy::expect_used)]
fn pr_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(pr|pull request)\b").expect("invalid regex"))
}

#[allow(clippy::expect_used)]
fn deep_keyword_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(深度|全面|全仓|跨模块|多模块|多维|架构|安全|回归风险|严重程度|findings)")
            .expect("invalid regex")
    })
}

#[allow(clippy::expect_used)]
fn narrow_review_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*review\s+(/|\.|[A-Za-z0-9_-].*\.(md|rs|tsx?|jsx?|py|json|toml))")
            .expect("invalid regex")
    })
}

#[allow(clippy::expect_used)]
fn framework_non_goal_entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(^|\s)/(gitx|update)\b").expect("invalid regex"))
}

/// Framework slash commands that may arm delegation (excludes goal-only entries).
pub fn is_framework_non_goal_entrypoint_prompt(text: &str) -> bool {
    let sanitized = strip_quoted_or_codeblock_or_url(text);
    framework_non_goal_entry_re().is_match(&sanitized)
}

/// 用户粘贴的 goal 续跑清门行前缀（分段拼接，避免在源码检索里出现完整误拼 token）。
/// **不含** `rg_followup`：该形态与 harness 文档中的「仿冒机读行」一致，若允许用户粘贴清门会鼓励误用模型自拟行；清门请用 `rg_clear`、拒因 token 或自然语言 override。
const PASTED_LINE_AG_FOLLOWUP_PREFIX: &str = concat!("ag", "_followup");

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
    if crate::hook_common::is_task_profile(repo_root, prompt_text)
    {
        // Task profiles suppress spawn-first nudge.
        // The "task" profile entry in RUNTIME_REGISTRY.json always
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
    crate::hook_common::is_task_profile(repo_root, text)
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

#[allow(clippy::expect_used)]
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

#[allow(clippy::unwrap_used, clippy::expect_used)]
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
        // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
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
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe { core_state_utils::env_sync::set_env(key, &v) },
            // SAFETY: test-only; process_env_lock() prevents concurrent env access from other tests.
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
    fn review_gate_advisory_only_is_global() {
        assert!(review_gate_advisory_only());
    }

    #[test]
    fn review_gate_hard_block_disabled_false_when_not_task_profile() {
        let _lock = crate::test_env_sync::process_env_lock();
        crate::hook_common::set_test_task_override(Some(false));
        assert!(!review_gate_hard_block_disabled(None, "fix the bug"));
        assert!(!review_gate_hard_block_disabled(None, "run the task"));
        crate::hook_common::set_test_task_override(None);
    }

    #[test]
    fn review_gate_hard_block_disabled_true_when_task_active() {
        let _lock = crate::test_env_sync::process_env_lock();
        crate::hook_common::set_test_task_override(Some(true));
        // Advisory-only: task profile active → hard block disabled
        assert!(review_gate_hard_block_disabled(None, "run the task"));
        assert!(review_gate_hard_block_disabled(None, "random text"));
        crate::hook_common::set_test_task_override(None);
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
}
