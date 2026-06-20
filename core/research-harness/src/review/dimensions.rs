//! Review dimension prompts for progressive disclosure.
//!
//! Each dimension provides a detailed reviewer prompt that guides the adversarial
//! reviewer subagent for that round.

use crate::types::ReviewDimension;

/// Get the detailed reviewer prompt for a given dimension.
pub fn dimension_prompt(dim: &ReviewDimension) -> String {
    match dim {
        ReviewDimension::LogicAndEvidence => LOGIC_AND_EVIDENCE_PROMPT.to_string(),
        ReviewDimension::NoveltyAndPositioning => NOVELTY_AND_POSITIONING_PROMPT.to_string(),
        ReviewDimension::MathAndNotation => MATH_AND_NOTATION_PROMPT.to_string(),
        ReviewDimension::FiguresAndReadability => FIGURES_AND_READABILITY_PROMPT.to_string(),
        ReviewDimension::LanguageAndTone => LANGUAGE_AND_TONE_PROMPT.to_string(),
        ReviewDimension::LengthAndAppendix => LENGTH_AND_APPENDIX_PROMPT.to_string(),
        ReviewDimension::FullRegression => FULL_REGRESSION_PROMPT.to_string(),
    }
}

/// Get the sub-dimensions checklist for a given dimension.
pub fn dimension_checklist(dim: &ReviewDimension) -> Vec<&'static str> {
    match dim {
        ReviewDimension::LogicAndEvidence => vec![
            "claim ceiling 是否被证据支撑",
            "evidence coverage: 每个 claim 是否有对应 evidence anchor",
            "ablation isolation: 消融实验是否隔离了各贡献",
            "comparison fairness: baseline 是否用了公平的设置/超参/数据",
            "统计检验是否正确（p 值、效应量、多重比较校正）",
        ],
        ReviewDimension::NoveltyAndPositioning => vec![
            "closest prior work 是否被完整引用和讨论",
            "novelty positioning: 与最近工作的关键区别是否清晰",
            "venue calibration: 目标 venue 的审稿标准是否被满足",
            "related work 是否只列不析",
        ],
        ReviewDimension::MathAndNotation => vec![
            "equation closure: 所有推导步骤是否闭合",
            "symbol uniqueness: 每个符号是否唯一定义",
            "derivation gaps: 是否有跳步未说明",
            "overmath: 是否用了不必要的数学复杂度",
            "方程编号是否连续且被正确引用",
        ],
        ReviewDimension::FiguresAndReadability => vec![
            "figure rendering: 分辨率、字体大小、颜色是否符合出版标准",
            "caption self-containment: 不看正文能否理解图表",
            "axis/legend clarity: 坐标轴标签、图例是否清晰",
            "table density: 列数是否过多，数据是否需要拆表",
            "图表在正文中的引用顺序是否正确",
        ],
        ReviewDimension::LanguageAndTone => vec![
            "terminology density: 术语使用是否一致",
            "defensive tone: 是否有过多 hedging 或防御性措辞",
            "EN slop: 英文是否有 AI 痕迹（Moreover/Furthermore/Delve/Tapestry）",
            "ZH 套话: 中文是否有'值得注意的是'/'众所周知'等空话",
            "topic sentence: 每段是否有清晰的主题句",
        ],
        ReviewDimension::LengthAndAppendix => vec![
            "page pressure: 是否因页面限制隐藏了必要证据",
            "appendix routing: 重要材料是否被不当移到附录",
            "format compliance: 是否符合目标 venue 的格式要求",
            "word count: 是否在允许范围内",
        ],
        ReviewDimension::FullRegression => vec![
            "前几轮修复是否引入了回归",
            "所有 claim 是否仍被证据支撑",
            "修改后的文本是否与原始 claim 一致",
            "全文一致性：术语、符号、格式是否统一",
        ],
    }
}

const LOGIC_AND_EVIDENCE_PROMPT: &str = r#"你是目标期刊/会议的恶意审稿人（hostile but fair）。
本轮聚焦：**逻辑与证据**。

审查重点：
1. **Claim Ceiling**：每个 claim 的强度是否被证据支撑？是否有过度声称？
2. **Evidence Coverage**：每个 claim 是否有对应的 evidence anchor？证据链是否完整？
3. **Ablation Isolation**：消融实验是否正确隔离了各组件的贡献？是否有混淆变量？
4. **Comparison Fairness**：baseline 对比是否公平？超参/数据/评估指标是否一致？
5. **统计检验**：p 值、效应量、置信区间、多重比较校正是否正确？

输出要求：
- 只报告你确实找到的、有具体位置（节/段/行）的问题
- 每个 finding 包含 severity（P0/A/B/Warning/C）
- 不编造问题，不重复已修复的问题
- 如果没有找到问题，明确说明"本轮未发现问题""#;

const NOVELTY_AND_POSITIONING_PROMPT: &str = r#"你是目标期刊/会议的恶意审稿人（hostile but fair）。
本轮聚焦：**最近工作与新颖性**。

审查重点：
1. **Closest Prior Work**：是否遗漏了关键的最近工作？是否正确引用和讨论？
2. **Novelty Positioning**：与最近工作的关键区别是否清晰表述？
3. **Venue Calibration**：论文是否满足目标 venue 的审稿标准？
4. **Related Work**：是否"只列不析"？是否有深入的对比分析？

输出要求同上。"#;

const MATH_AND_NOTATION_PROMPT: &str = r#"你是目标期刊/会议的恶意审稿人（hostile but fair）。
本轮聚焦：**数学与符号**。

审查重点：
1. **Equation Closure**：所有推导步骤是否闭合？是否有跳步？
2. **Symbol Uniqueness**：每个符号是否唯一定义？是否有符号复用导致歧义？
3. **Derivation Gaps**：关键推导步骤是否有省略未说明？
4. **Overmath**：是否有不必要的数学复杂度？
5. **方程编号**：是否连续？引用是否正确？

输出要求同上。"#;

const FIGURES_AND_READABILITY_PROMPT: &str = r#"你是目标期刊/会议的恶意审稿人（hostile but fair）。
本轮聚焦：**图表与可读性**。

审查重点：
1. **Figure Rendering**：分辨率(DPI≥300)、字体大小、色盲友好
2. **Caption Self-containment**：不看正文能否理解图表含义
3. **Axis/Legend Clarity**：坐标轴标签、单位、图例
4. **Table Density**：列数是否过多，是否需要拆分
5. **正文引用顺序**：图表在正文中是否按顺序出现

输出要求同上。"#;

const LANGUAGE_AND_TONE_PROMPT: &str = r#"你是目标期刊/会议的恶意审稿人（hostile but fair）。
本轮聚焦：**语言与防御性**。

审查重点：
1. **Terminology Consistency**：同一概念是否使用了不同术语
2. **Defensive Tone**：是否有过多 hedging（may/might/possibly 叠加）
3. **AI Slop（英文）**：Moreover/Furthermore/Delve/Tapestry/Landscape 等 AI 高频词
4. **套话（中文）**：值得注意的是/众所周知/不言而喻 等空话
5. **Topic Sentence**：每段是否有清晰的主题句

输出要求同上。"#;

const LENGTH_AND_APPENDIX_PROMPT: &str = r#"你是目标期刊/会议的恶意审稿人（hostile but fair）。
本轮聚焦：**长度与附录路由**。

审查重点：
1. **Page Pressure**：是否因页面限制隐藏了必要证据
2. **Appendix Routing**：重要材料是否被不当移到附录
3. **Format Compliance**：是否符合目标 venue 的格式要求
4. **Word Count**：是否在允许范围内

输出要求同上。"#;

const FULL_REGRESSION_PROMPT: &str = r#"你是目标期刊/会议的恶意审稿人（hostile but fair）。
本轮聚焦：**全面重审（回归检查）**。

审查重点：
1. 前几轮修复是否引入了回归
2. 所有 claim 是否仍被证据支撑
3. 修改后的文本是否与 claim ledger 一致
4. 全文术语/符号/格式一致性
5. 任何前三轮未覆盖到的问题

输出要求同上。这是最后一轮，请尽最大努力发现遗留问题。"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_dimensions_have_prompts() {
        let dims = [
            ReviewDimension::LogicAndEvidence,
            ReviewDimension::NoveltyAndPositioning,
            ReviewDimension::MathAndNotation,
            ReviewDimension::FiguresAndReadability,
            ReviewDimension::LanguageAndTone,
            ReviewDimension::LengthAndAppendix,
            ReviewDimension::FullRegression,
        ];
        for dim in &dims {
            let prompt = dimension_prompt(dim);
            assert!(!prompt.is_empty());
            assert!(prompt.contains("恶意审稿人"));
        }
    }

    #[test]
    fn test_all_dimensions_have_checklists() {
        let dims = [
            ReviewDimension::LogicAndEvidence,
            ReviewDimension::NoveltyAndPositioning,
            ReviewDimension::MathAndNotation,
            ReviewDimension::FiguresAndReadability,
            ReviewDimension::LanguageAndTone,
            ReviewDimension::LengthAndAppendix,
            ReviewDimension::FullRegression,
        ];
        for dim in &dims {
            let checklist = dimension_checklist(dim);
            assert!(!checklist.is_empty());
        }
    }
}
