//! Prose 质量 hook — 为论文编辑场景追加语言质量检查上下文。
//!
//! 独立于 runtime-core 的宿主耦合逻辑：检测用户提示是否涉及论文写作/润色，
//! 若是则返回 prose 质量检查提示段落。

/// Prose 质量检查提示文本（内建回落文案）。
pub const PROSE_QUALITY_CONTEXT: &str = concat!(
    "**PAPER_PROSE_QUALITY_HOOK**\n\n",
    "当前场景检测到论文写作/润色信号。请遵守以下语言质量准则：\n",
    "1. 避免 AI slop 词汇（Moreover, Furthermore, Delve, Tapestry, Landscape, Leverage）\n",
    "2. 保持学术语气但避免防御性过重（hedging: may, might, could, possibly ≤ 3 处/段）\n",
    "3. 中文正文不用套话（值得注意的是, 众所周知, 不言而喻, 具有重要意义）\n",
    "4. 术语全篇一致，首次出现给出定义\n",
    "5. 英文论文使用 active voice 为主，被动语态仅在方法描述中出现"
);

/// 检测用户提示是否涉及论文写作/润色（中英文）。
/// 可被宿主 hooks 用于判断是否注入 prose 质量上下文。
pub fn prompt_signals_prose_work(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();

    // 工程噪声过滤：abstract base class / abstract class 是 OOP 术语
    if lower.contains("abstract base class") || lower.contains("abstract class") {
        return false;
    }

    // 强信号：论文写作/润色
    static STRONG_ZH: &[&str] = &[
        "润色", "改稿", "论文", "手稿", "引言", "摘要",
        "讨论节", "结论节", "相关工作", "方法论",
    ];
    if STRONG_ZH.iter().any(|k| text.contains(k)) {
        return true;
    }

    static STRONG_EN: &[&str] = &[
        "polish", "manuscript", "abstract", "introduction",
        "discussion", "related work", "methodology",
    ];
    if STRONG_EN.iter().any(|k| lower.contains(k)) {
        return true;
    }

    // LaTeX 片段信号
    if text.contains("\\begin{abstract}") || text.contains("\\cite{") {
        return true;
    }

    false
}

/// 在检测到论文编辑相关操作时，追加 prose 质量检查上下文片段。
/// 返回 `None` 表示不追加。
pub fn maybe_append_prose_context(context: &str) -> Option<String> {
    if !prompt_signals_prose_work(context) {
        return None;
    }
    Some(PROSE_QUALITY_CONTEXT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_polish_zh() {
        assert!(maybe_append_prose_context("帮我把这段引言润色一下，中文正文").is_some());
    }

    #[test]
    fn signal_abstract_en() {
        assert!(maybe_append_prose_context("polish this abstract").is_some());
    }

    #[test]
    fn signal_latex_abstract() {
        assert!(maybe_append_prose_context(
            "论文 改一下下面这段 \\begin{abstract} We propose a method \\cite{foo}"
        )
        .is_some());
    }

    #[test]
    fn no_signal_ci_only() {
        assert!(maybe_append_prose_context("fix cargo test in pull request workflow").is_none());
    }

    #[test]
    fn no_signal_abstract_base_class() {
        assert!(maybe_append_prose_context("edit the abstract base class in this Java module").is_none());
    }
}
