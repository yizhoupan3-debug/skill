---

description: 'Prose verification capability: terminology consistency, style guide compliance, claim drift detection, language register, hedging appropriateness.'
metadata:
  platforms:
  - supported
  tags:
  - prose
  - writing
  - verification
  - style
  - research
  version: '1.0.0'
name: prose-verification
scene: research
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P2
session_start: n/a
source: local
trigger_hints:
- $prose-verification
- claim drift
- prose check
- writing verification
- 文体验证
- 术语一致性
- 风格检查
- prose-verification
---
# Prose Verification

无状态能力 skill：对文稿做语言质量门禁检查。不独立编排会话。

## When to Use

- 前门 skill 需要验证术语使用是否前后一致
- 需要检查文稿是否符合目标 venue 的风格指南
- 需要检测 claim 是否在文稿传播中发生漂移
- 需要验证语言注册（formal / technical / accessible）是否匹配目标受众
- 需要检查 hedging 用语是否适度

## Do not use

- 论文结构调整（→ `$structure-verification`）
- 统计结果审计（→ `$statistical-verification`）
- 文献引用验证（→ `$literature-verification`）
- 数学推导验证（→ `$formal-verification`）

## Hard constraints

- 术语不一致（同一概念使用不同名称）为 FAIL——读者混淆风险
- Claim drift 检测到语义偏差为 P0 blocker——论文声称必须与 claim ledger 一致
- 过度断言（"proven"、"definitively" 在非证明性论文中）为 FAIL，必须降级为 hedged 表述
- 语言注册不匹配为 WARN，但若跨注册混用（同一段落 formal + accessible 混杂）为 FAIL
- hedging 检查仅做格式检测（关键词匹配），不做语义判断——需声明此局限性

## Input / Output

| 输入 | 输出 |
|------|------|
| 文稿全文（LaTeX / Markdown） | 每项检查的 PASS / FAIL / WARN 状态 |
| 术语表（glossary） | 不一致项清单及建议修正 |
| claim ledger | claim drift 报告 |
| 目标 venue 风格指南 | 风格违规清单 |

## Verification Checklist

Rust 实现：`research_harness::verification::prose_qc`（通过 MCP tool 或直接调用）

MCP tool: `research_verification_prose` → `verification_tool_dispatch`（`check` 参数：`terminology` / `slop` / `hedging`）

```
# 术语一致性检查：
research_harness::verification::prose_qc::check_terminology_consistency(text, glossary)

# AI slop 检测（英文）：
research_harness::verification::prose_qc::detect_en_slop(text)

# 中文套话检测：
research_harness::verification::prose_qc::detect_zh_slop(text)

# Hedging 词统计：
research_harness::verification::prose_qc::count_hedging_words(text)
```

| # | 检查名 | PASS 条件 |
|---|--------|-----------|
| 1 | 术语一致性 | 全文同一概念使用同一术语（与术语表匹配） |
| 2 | 风格指南合规 | 引用格式与 venue 要求一致 |
| 3 | Claim drift 检查 | 论文中每条 claim 与 claim ledger 语义一致 |
| 4 | 语言注册适配 | 全文注册（formal/technical/accessible）与目标匹配 |
| 5 | Hedging 适度性 | 关键发现有适度 hedging，过度断言 ≤ 5 处 |

## References

- prose-chain-contract：[`../../skills/research/paper-workbench/references/prose-chain-contract.md`](../../skills/research/paper-workbench/references/prose-chain-contract.md)
- prose-quality-gate：[`../../skills/research/paper-workbench/references/prose-quality-gate.md`](../../skills/research/paper-workbench/references/prose-quality-gate.md)
- prose-exemplars：[`../../skills/research/paper-workbench/references/prose-exemplars.md`](../../skills/research/paper-workbench/references/prose-exemplars.md)
- research-language-norms：[`../../skills/research/paper-workbench/references/research-language-norms.md`](../../skills/research/paper-workbench/references/research-language-norms.md)

## Integration Contract

### Trigger

| Caller | When | Blocking | Call mode |
|--------|------|----------|-----------|
| `paper-workbench` | draft complete, before submission gate | Yes (FAIL blocks submission readiness) | Inline |
| `$research` (execution lane) | experiment narrative / research prose produced | No (advisory — WARN/FAIL annotates but does not block) | Inline |

### Input

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `text` | `String` | yes | Full manuscript or research prose (LaTeX / Markdown) |
| `glossary` | `Vec<{term, preferred_term}>` | no | Terminology glossary (omit = skip check #1) |
| `claim_ledger` | `Vec<Claim>` | no | Claim list for drift detection (omit = skip check #3) |
| `style_guide` | `String` | no | Target venue style guide (omit = skip check #2) |

### Output

```json
{
  "status": "PASS" | "FAIL" | "WARN",
  "checks": [
    { "name": "terminology_consistency", "status": "PASS" | "SKIP" | "FAIL", "detail": "..." },
    { "name": "style_guide_compliance", "status": "PASS" | "SKIP" | "FAIL", "detail": "..." },
    { "name": "claim_drift", "status": "PASS" | "SKIP" | "FAIL", "detail": "..." },
    { "name": "language_register", "status": "PASS" | "WARN" | "FAIL", "detail": "..." },
    { "name": "hedging_appropriateness", "status": "PASS" | "WARN", "detail": "..." }
  ],
  "blockers": ["Claim drift: §3.1 claim 'outperforms SOTA by 5%' != ledger 'outperforms SOTA by 3%'"]
}
```

### Failure propagation

- **PASS**: caller continues normally.
- **WARN**: caller continues with annotation in evidence map / prose report.
- **FAIL** (blocking caller for `$research` paper-workbench lane, advisory for `$research` execution lane): paper-workbench MUST NOT advance; execution lane continues with blocker noted.
