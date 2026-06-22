//! 多轮审稿编排 — 构建 reviewer subagent prompt 和 loop 执行流程。
//!
//! 这是 paper-workbench loop mode 的 Rust 真源。SKILL.md 通过 MCP tool 调用这些函数。

use crate::review::dimensions;
use crate::types::ReviewDimension;

/// Build the complete reviewer prompt for a given round.
///
/// The prompt includes:
/// - Target venue context
/// - Dimension-specific instructions (progressive disclosure)
/// - Severity classification rules
/// - Output format requirements
///
/// The reviewer subagent does NOT know the total number of rounds (progressive disclosure).
pub fn build_reviewer_prompt(
    round: u64,
    dimension: &ReviewDimension,
    manuscript_summary: &str,
) -> String {
    let dim_prompt = dimensions::dimension_prompt(dimension);
    let checklist = dimensions::dimension_checklist(dimension);
    let checklist_text = checklist
        .iter()
        .enumerate()
        .map(|(i, item)| format!("{}. {}", i + 1, item))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"# Paper Review — Round {round}

{dim_prompt}

## Checklist
{checklist_text}

## Severity Classification
- **P0** (一票否决): 数据完整性、学术诚信、硬理论错误 → 立即拒稿
- **A** (核心硬伤): 逻辑/方法/证据核心缺陷 → 大修或拒稿
- **B** (需补充): 缺失数据/实验/基线/统计验证 → 削弱但可修复
- **Warning** (隐晦警告): 隐蔽遗漏、未声明边界 → 必须列出
- **C** (打磨): 文字润色/风格/排版 → 不影响录用

## Manuscript Summary
{manuscript_summary}

## Output Format
输出 JSON：
```json
{{
  "verdict": "accept|revise|reject",
  "dimension_covered": "{dim_name}",
  "findings": [
    {{
      "id": "R{round}-001",
      "severity": "A|B|Warning|C|P0",
      "dimension": "{dim_name}",
      "location": "§2.1 第3段",
      "description": "具体问题描述",
      "suggestion": "修复建议（可选）"
    }}
  ],
  "summary": "本轮审查总结"
}}
```

注意：
- 只报告你确实找到的、有具体位置的问题
- 不编造、不重复已修复的问题
- 如果没有找到问题，findings 为空数组，verdict 为 "accept""#,
        round = round,
        dim_prompt = dim_prompt,
        checklist_text = checklist_text,
        manuscript_summary = manuscript_summary,
        dim_name = dimension.display_name(),
    )
}

/// Build the orchestrator's initialization prompt for the loop.
///
/// This is passed to the orchestrator subagent that manages the multi-round loop.
pub fn build_orchestrator_prompt(
    goal: &str,
    target_venue: &str,
    manuscript_path: &str,
) -> String {
    format!(
        r#"# Paper Revision Orchestrator

## Goal
{goal}

## Target Venue
{target_venue}

## Manuscript Path
{manuscript_path}

## Procedure
1. Initialize Quality Gate loop via `quality_gate_manage`:
   - operation: start
   - goal: "{goal}"
   - max_rounds: 10
   - min_rounds: 5
   - consecutive_stable_required: 2

2. For each round (R1 through R10):
   a. Read the current manuscript
   b. Spawn ONE reviewer subagent with the dimension-specific prompt
   c. Receive findings (JSON)
   d. Apply surgical fixes based on findings
   e. Log round via `quality_gate_manage`:
      - operation: append_round
      - round: N
      - review_summary: summary from reviewer
      - fix_summary: summary of changes made
      - adversarial_findings: findings array
      - supervisor_decision: "continue" (always, until convergence)
   f. Check convergence:
      - If no new P0/A/B findings AND round >= min_rounds → stable_count++
      - Else → stable_count = 0
      - If stable_count >= 2 → BREAK (converged)

3. Close QG:
   - quality_gate_manage(operation=append_round, supervisor_decision="close")

4. Write closeout record:
   - summary: "Paper revision converged after N rounds"
   - verification_status: "passed"

## Progressive Disclosure
Reviewer subagents do NOT know the total round count.
Each round reveals only the current dimension (R1→Logic, R2→Novelty, ...).
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_reviewer_prompt_contains_dimension() {
        let prompt = build_reviewer_prompt(1, &ReviewDimension::LogicAndEvidence, "Test paper about X");
        assert!(prompt.contains("逻辑与证据"));
        assert!(prompt.contains("Round 1"));
        assert!(prompt.contains("Claim Ceiling"));
    }

    #[test]
    fn test_build_orchestrator_prompt_contains_params() {
        let prompt = build_orchestrator_prompt("Test goal", "NeurIPS", "manuscript/main.tex");
        assert!(prompt.contains("min_rounds: 5"));
        assert!(prompt.contains("NeurIPS"));
    }
}
