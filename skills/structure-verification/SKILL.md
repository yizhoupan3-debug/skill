---

description: 'Structure verification capability: LaTeX compilation, cross-reference consistency, claim-evidence alignment, format compliance, notation consistency, equation numbering.'
metadata:
  platforms:
  - supported
  tags:
  - structure
  - verification
  - LaTeX
  - format
  - research
  version: '1.0.0'
name: structure-verification
scene: research
sub_scene: structure
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: n/a
source: local
trigger_hints:
- $structure-verification
- LaTeX编译
- cross-reference
- format check
- structure verification
- 图表引用检查
- 结构验证
---
# Structure Verification

无状态能力 skill：对论文结构与格式做完整性审计。不独立编排会话。

## When to Use

- 前门 skill 需要验证 LaTeX 项目能否无错编译
- 需要检查 \ref 与 \label 是否一一对应
- 需要确认每条 claim 有 ≥ 1 条 evidence 支撑
- 需要检查格式合规（页数、字体、边距）
- 需要验证符号定义在各 section 中的一致性
- 需要确认方程编号连续无跳号

## Do not use

- 文稿语言质量、术语一致性 → 使用 `$prose-verification`
- 文献引用验证 → 使用 `$literature-verification`
- 数学推导验证 → 使用 `$formal-verification`

## Hard constraints

- LaTeX 编译失败（exit non-zero）为 P0 blocker——文稿必须可编译
- 悬空引用（\ref 无对应 \label）为 FAIL，每处必须修复或删除
- 方程编号跳号为 FAIL——读者无法定位特定方程
- 符号歧义（同一符号在不同 section 有不同含义）为 FAIL——必须统一或加下标区分
- Claim-evidence 检查仅在提供了 claim ledger 时执行；无 ledger 时声明跳过此检查

## Input / Output

| 输入 | 输出 |
|------|------|
| LaTeX 项目目录（main.tex + 子文件） | 每项检查的 PASS / FAIL / WARN 状态 |
| claim ledger（可选） | 悬空引用 / 断裂编号清单 |
| 目标 venue 格式规范 | 格式违规清单及修正建议 |

## Verification Checklist

Rust 实现：`research_harness::verification::structure`（通过 MCP tool 或直接调用）

MCP tool: `research_verification_structure` → `verification_tool_dispatch`（`check` 参数：`latex` / `figures`）

```
# LaTeX 语法检查：
research_harness::verification::structure::check_latex_compilable(tex_path)

# 图表引用完整性：
research_harness::verification::structure::check_figure_references(tex_path)
# 返回未找到定义的引用列表
```

| # | 检查名 | PASS 条件 |
|---|--------|-----------|
| 1 | LaTeX 编译 | latexmk exit 0，无 error |
| 2 | 图表引用一致 | 每个 \ref 有对应 \label |
| 3 | Claim-evidence 对齐 | 每条 claim ≥ 1 条 evidence（需提供 claim ledger） |
| 4 | 格式合规 | 页数、字号、边距符合 venue 要求 |
| 5 | 符号一致性 | 同一符号在所有 section 中含义相同 |
| 6 | 方程编号连续 | 方程编号无跳号 |

## References

- citation-management skill：[`../citation-management/SKILL.md`](../citation-management/SKILL.md)（引用一致性检查）
- scientific-figure-plotting skill：[`../scientific-figure-plotting/SKILL.md`](../scientific-figure-plotting/SKILL.md)（图表规范）
- claim-evidence-ladder：[`../paper-workbench/references/claim-evidence-ladder.md`](../paper-workbench/references/claim-evidence-ladder.md)
- claim-spine-and-section-contract：[`../paper-workbench/references/claim-spine-and-section-contract.md`](../paper-workbench/references/claim-spine-and-section-contract.md)

## Integration Contract

### Trigger

| Caller | When | Blocking | Call mode |
|--------|------|----------|-----------|
| `paper-workbench` | LaTeX project structure finalized, before submission gate | Yes (FAIL blocks submission readiness) | Inline |

### Input

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tex_dir` | `Path` | yes | LaTeX project root (containing main.tex) |
| `claim_ledger` | `Vec<Claim>` | no | Claim list for evidence alignment (omit = skip check #3) |
| `venue_spec` | `VenueFormat` | no | Target venue format constraints (omit = skip check #4) |

### Output

```json
{
  "status": "PASS" | "FAIL" | "WARN",
  "checks": [
    { "name": "latex_compilation", "status": "PASS" | "FAIL", "detail": "latexmk exit 0" },
    { "name": "cross_reference_consistency", "status": "PASS" | "FAIL", "detail": "..." },
    { "name": "claim_evidence_alignment", "status": "PASS" | "SKIP" | "FAIL", "detail": "..." },
    { "name": "format_compliance", "status": "PASS" | "SKIP" | "FAIL", "detail": "..." },
    { "name": "notation_consistency", "status": "PASS" | "FAIL", "detail": "..." },
    { "name": "equation_numbering", "status": "PASS" | "FAIL", "detail": "..." }
  ],
  "blockers": ["LaTeX compilation failed: undefined control sequence at line 342"]
}
```

### Failure propagation

- **PASS**: caller continues normally.
- **WARN**: caller continues with annotation in structure report.
- **FAIL** (blocking caller): caller MUST NOT advance to submission; blocker list returned to upstream orchestrator.
