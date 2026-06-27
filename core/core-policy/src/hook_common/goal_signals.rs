//! Goal signal detection — contract recognition, progress tracking, and completion claims.
//!
//! Extracted from `hook_common.rs` (2026-06-25 refactor) to reduce file
//! size and clarify responsibility boundaries within `core-policy`.

use regex::Regex;
use std::sync::{LazyLock, OnceLock};

// ────────────────────────────────────────────────────────────────
// Completion claim detection
// ────────────────────────────────────────────────────────────────

/// 完成宣称 token：英文词 + 中文短语。**Single source of truth**：
///
/// - `closeout_enforcement::summary_claims_completion` 直接对 summary 原文扫描；
/// - `cursor_hooks::completion_claimed_in_text` 先剥离引文 / 代码块 / URL 再扫描；
/// - 中文用多字短语避免「完成度 / 完成任务拆分」等子串误命中。
pub const COMPLETION_DETECT_EN: &[&str] = &["done", "finished", "completed", "succeeded", "passed"];

/// 完成宣称（触发 closeout）：不含「验证通过/测试通过」等，避免与 goal verify 聊天轨打架；
/// 中文用 3+ 字短语避免「完成度 / 完成任务拆分 / 通过阅读」等子串误命中。
pub const COMPLETION_DETECT_ZH_PHRASES: &[&str] =
    &["已完成", "已经完成", "全部完成", "完成了", "搞定", "任务完成", "验证通过"];

/// 仅用于无磁盘 GOAL 时的聊天 progress/verify 提示（不进 closeout 词表）。
pub const GOAL_CHAT_VERIFY_ZH_PHRASES: &[&str] = &["验证通过", "测试通过", "审核通过", "已通过"];

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

// ────────────────────────────────────────────────────────────────
// Goal signal detection (shared across all hosts)
// ────────────────────────────────────────────────────────────────

/// Regex for goal contract keywords (EN + ZH).
#[allow(clippy::expect_used)]
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
#[allow(clippy::expect_used)]
pub fn goal_progress_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(checkpoint|milestone|progress|next step)\b|(检查点|里程碑|进度|下一步)")
            .expect("invalid regex")
    })
}

/// Regex for goal verify/block keywords (EN + ZH).
#[allow(clippy::expect_used)]
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
#[allow(clippy::expect_used)]
pub fn nonempty_inline_heading_any(text: &str, heading: &str) -> bool {
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
#[allow(clippy::expect_used)]
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

#[allow(clippy::unwrap_used, clippy::expect_used)]
#[cfg(test)]
mod tests {
    #[test]
    fn is_task_profile_returns_false_without_root() {
        assert!(!crate::hook_common::is_task_profile(None, "run the task"));
        assert!(!crate::hook_common::is_task_profile(None, "fix the bug"));
        assert!(!crate::hook_common::is_task_profile(None, "analyze architecture"));
    }

    #[test]
    fn is_task_profile_returns_false_for_arbitrary_prompt_without_root() {
        assert!(!crate::hook_common::is_task_profile(None, "深度review整个路由系统"));
        assert!(!crate::hook_common::is_task_profile(None, "fix the bug"));
        assert!(!crate::hook_common::is_task_profile(None, ""));
    }

    #[test]
    fn is_task_profile_test_override_takes_priority() {
        let _lock = crate::test_env_sync::process_env_lock();
        // Override to false
        crate::hook_common::set_test_task_override(Some(false));
        assert!(!crate::hook_common::is_task_profile(None, "run the task"));
        // Override to true even for non-interactive prompt
        crate::hook_common::set_test_task_override(Some(true));
        assert!(crate::hook_common::is_task_profile(None, "random text"));
        // Clear override, returns false
        crate::hook_common::set_test_task_override(None);
        assert!(!crate::hook_common::is_task_profile(None, "run the task"));
    }
}
