//! 对抗审稿 hook — 为论文编辑场景追加对抗性审稿上下文。
//!
//! 独立于 runtime-core 的宿主耦合逻辑：检测用户提示是否涉及论文审稿/返修，
//! 若是则返回强对抗审稿提示段落。

/// 对抗审稿提示文本（内建回落文案）。
pub const ADVERSARIAL_CONTEXT: &str = concat!(
    "**PAPER_ADVERSARIAL_HOOK**\n\n",
    "当前场景检测到论文审稿/返修信号。请执行强对抗审稿：\n",
    "1. 以 closest-work 为基线，逐维度检查 claim ceiling\n",
    "2. 找出 data integrity / academic integrity / hard theory 三个维度的一票否决项\n",
    "3. 对每个 finding 给出 P0/A/B/Warning/C 分级\n",
    "4. 检查 rebuttal 中的 response letter 是否逐条回应\n",
    "5. 检查 revision 是否引入新问题（regression）\n",
    "6. 收敛条件：连续 2 轮无 P0/A/B findings → 通过"
);

/// 轻量启发：倾向少漏报论文审稿任务、少误伤纯工程 PR 与纯 ML 讨论。
/// 可被宿主 hooks 用于判断是否注入对抗审稿上下文。
pub fn prompt_signals_manuscript_work(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();

    let has_zh_paper = text.contains("论文") || text.contains("手稿");
    let has_en_paper = lower.contains("manuscript") || lower.contains("rebuttal");
    let has_paper_signal = has_zh_paper || has_en_paper;

    // 工程噪声过滤
    let code_only_noise = (lower.contains("pull request")
        || lower.contains(".github/workflows")
        || lower.contains("cargo test")
        || lower.contains("cargo build")
        || lower.contains("cargo fmt")
        || lower.contains("clippy"))
        && !has_paper_signal;
    if code_only_noise {
        return false;
    }

    // 强中文信号
    static STRONG_ZH: &[&str] = &[
        "审稿", "审稿人", "审稿意见", "返修", "大修", "小修",
        "改稿", "投稿", "rebuttal", "response letter",
    ];
    if STRONG_ZH.iter().any(|k| text.contains(k)) {
        return true;
    }

    // 强英文信号
    static STRONG_EN: &[&str] = &[
        "manuscript", "revise and resubmit", "meta-review",
        "reviewer comment", "major revision", "minor revision",
        "point-by-point", "\\begin{abstract}", "supplementary material",
    ];
    if STRONG_EN.iter().any(|k| lower.contains(k)) {
        return true;
    }

    // 弱信号组合（需 ≥5 个同时命中才放行）
    static WEAK: &[&str] = &[
        "latex", "appendix", "theorem", "lemma",
        "baseline", "ablation", "novelty", "claim",
    ];
    let weak_count = WEAK.iter().filter(|k| lower.contains(*k)).count();

    // ML 行话降权
    static ANTI_SIGNALS: &[&str] = &[
        "transformer", "attention", "convolution", "normalization",
        "optimizer", "gradient descent", "batch size", "learning rate",
    ];
    let anti_hits = ANTI_SIGNALS.iter().filter(|k| lower.contains(*k)).count();
    let adjusted = if anti_hits >= 2 && !has_paper_signal {
        weak_count.saturating_sub(2)
    } else {
        weak_count
    };

    adjusted >= 5
}

/// 在检测到论文编辑相关操作时，追加对抗性审稿上下文片段。
/// 返回 `None` 表示不追加。
pub fn maybe_append_adversarial_context(context: &str) -> Option<String> {
    if !prompt_signals_manuscript_work(context) {
        return None;
    }
    Some(ADVERSARIAL_CONTEXT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_zh_reviewer() {
        assert!(maybe_append_adversarial_context("请根据审稿意见逐条改 Introduction").is_some());
    }

    #[test]
    fn signal_en_manuscript() {
        assert!(maybe_append_adversarial_context("Revise the manuscript per reviewer comments").is_some());
    }

    #[test]
    fn no_signal_pr_without_paper() {
        assert!(maybe_append_adversarial_context("fix failing cargo test in CI and open a pull request").is_none());
    }

    #[test]
    fn weak_signals_need_five_hits() {
        // 4 weak hits below threshold
        assert!(maybe_append_adversarial_context(
            "baseline, ablation, novelty, claim"
        )
        .is_none());
        // 5 weak hits meets threshold
        assert!(maybe_append_adversarial_context(
            "appendix: baseline ablation novelty claim metrics"
        )
        .is_some());
    }

    #[test]
    fn ml_tech_discussion_suppressed() {
        assert!(maybe_append_adversarial_context(
            "The training loss uses a transformer architecture with layer normalization and attention"
        )
        .is_none());
    }

    #[test]
    fn ml_tech_with_paper_keyword_not_suppressed() {
        assert!(maybe_append_adversarial_context(
            "请根据审稿意见修改这篇手稿的 baseline 和 ablation 实验设计"
        )
        .is_some());
    }
}
