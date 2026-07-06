//! 严重度分级 — 基于关键词启发式的 finding 分类。
//!
//! - P0（一票否决）、A（核心硬伤）、B（需补充）、Warning（隐晦警告）、C（打磨）。

use crate::types::Severity;

/// Classify a finding's severity based on keyword heuristics with negation guard.
/// This is a first-pass classifier; the reviewer subagent makes the final call.
pub fn classify_finding(text: &str) -> Severity {
    let lower = text.to_ascii_lowercase();

    // P0: 数据完整性/学术诚信/硬理论错误
    if contains_any_negation_aware(
        &lower,
        &[
            "数据造假",
            "fabricat",
            "plagiar",
            "抄袭",
            "学术不端",
            "数据泄露",
            "违反伦理",
            "irreproducible",
            "cannot reproduce",
            "hard理论错误",
            "fundamental error",
            "mathematical impossibility",
        ],
    ) {
        return Severity::P0;
    }

    // A: 核心硬伤
    if contains_any_negation_aware(
        &lower,
        &[
            "逻辑错误",
            "logic error",
            "核心缺陷",
            "fatal flaw",
            "不成立",
            "does not hold",
            "无法支撑",
            "unsupported claim",
            "baseline 不公平",
            "unfair comparison",
            "混淆变量",
            "confounding",
            "关键缺失",
            "critical missing",
            "重大遗漏",
        ],
    ) {
        return Severity::A;
    }

    // B: 需补充
    if contains_any_negation_aware(
        &lower,
        &[
            "缺少实验",
            "missing experiment",
            "需要补充",
            "needs additional",
            "缺少基线",
            "missing baseline",
            "统计不足",
            "insufficient statistics",
            "缺少消融",
            "missing ablation",
            "需要验证",
            "requires verification",
            "缺少分析",
            "lacks analysis",
            "证据不足",
            "insufficient evidence",
        ],
    ) {
        return Severity::B;
    }

    // Warning: 隐晦警告
    if contains_any_negation_aware(
        &lower,
        &[
            "可能误导",
            "potentially misleading",
            "未声明",
            "undeclared",
            "边界条件",
            "boundary condition",
            "隐蔽遗漏",
            "subtle omission",
            "读者误读",
            "misinterpreted",
            "隐含假设",
            "implicit assumption",
        ],
    ) {
        return Severity::Warning;
    }

    // C: 打磨（默认）
    Severity::C
}

/// Check for any keyword match with negation guard.
/// Returns false if keyword is found but preceded (within 40 chars) by a negation word.
/// Only checks the first occurrence of each keyword; multi-keyword lists still work independently.
fn contains_any_negation_aware(text: &str, keywords: &[&str]) -> bool {
    let negations = [
        "not ", "no ", "cannot ", "can't ", "don't ", "doesn't ",
        "isn't ", "aren't ", "won't ", "without ", "never ",
        "没有", "不是", "并非", "无法", "不能", "不会",
    ];
    for kw in keywords {
        if let Some(pos) = text.find(kw) {
            let window_start = pos.saturating_sub(40);
            let window = &text[window_start..pos];
            let negated = negations.iter().any(|n| window.contains(n));
            if !negated {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p0_classification() {
        assert_eq!(
            classify_finding("数据造假：Figure 3 的结果存在 fabrication"),
            Severity::P0
        );
        assert_eq!(
            classify_finding("Plagiarism detected in Section 2"),
            Severity::P0
        );
    }

    #[test]
    fn test_a_classification() {
        assert_eq!(
            classify_finding("逻辑错误：Claim 2 不成立，因为缺少关键证据"),
            Severity::A
        );
        assert_eq!(
            classify_finding("Baseline 不公平：超参设置不一致"),
            Severity::A
        );
    }

    #[test]
    fn test_b_classification() {
        assert_eq!(
            classify_finding("缺少消融实验来验证各组件贡献"),
            Severity::B
        );
        assert_eq!(classify_finding("需要补充统计分析"), Severity::B);
    }

    #[test]
    fn test_c_default() {
        assert_eq!(classify_finding("建议改进段落过渡"), Severity::C);
    }

    #[test]
    fn test_negation_prevents_false_positive() {
        assert_eq!(
            classify_finding("The result is not irreproducible — we confirmed it 3 times"),
            Severity::C,
            "negated 'irreproducible' should not trigger P0"
        );
        assert_eq!(
            classify_finding("没有数据造假嫌疑，所有数据均有原始记录"),
            Severity::C,
            "negated 数据造假 should not trigger P0"
        );
        assert_eq!(
            classify_finding("This is not a fatal flaw — it's a minor presentation issue"),
            Severity::C,
            "negated fatal flaw should not trigger A"
        );
        assert_eq!(
            classify_finding("并非关键缺失，只是可以补充说明"),
            Severity::C,
            "negated 关键缺失 should not trigger A"
        );
    }

    #[test]
    fn test_unnegated_keywords_still_match() {
        assert_eq!(
            classify_finding("irreproducible: cannot reproduce Figure 3 results"),
            Severity::P0,
            "unnegated 'irreproducible' should still trigger P0"
        );
        assert_eq!(
            classify_finding("fatal flaw in the experimental design"),
            Severity::A,
            "unnegated 'fatal flaw' should still trigger A"
        );
    }
}
