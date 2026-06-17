---
name: prose-verification
description: |
  无状态内部 skill：验证论文/申请书文稿的术语一致性、风格合规、claim drift、
  语言注册和 hedging 适度性。  由 paper-workbench 内联调用。
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
user-invocable: false
disable-model-invocation: true
risk: low
source: local
metadata:
  version: "1.0.0"
  platforms: [supported]
  tags: [prose, writing, verification, style, research]
trigger_hints:
  - 文稿质量验证
  - 术语一致性检查
  - claim drift 检测
  - 风格合规
  - hedging 适度性
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

- 论文结构、LaTeX 编译、格式检查 → 使用 `$structure-verification`
- 文献引用可靠性验证 → 使用 `$literature-verification`
- 统计结果审计 → 使用 `$statistical-verification`

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

> **Note**: verify commands below are pattern templates. Actual commands depend on project setup and available tools.

| # | 检查名 | PASS 条件 | verify command |
|---|--------|-----------|----------------|
| 1 | 术语一致性 | 全文同一概念使用同一术语（与术语表匹配） | `grep -onE '(算法|方法|模型|框架)' draft.tex \| sort \| uniq -c` → 无别名冲突 |
| 2 | 风格指南合规 | 页数/字号/引用格式符合 venue 要求 | `grep -cE '\\cite|\\ref' draft.tex` → 引用格式与 venue guide 一致 |
| 3 | Claim drift 检查 | 论文中每条 claim 与 claim ledger 语义一致 | `diff <(grep -oP 'claim:.*' draft.tex) claim_ledger.md` → 无语义偏差 |
| 4 | 语言注册适配 | 全文注册（formal/technical/accessible）与目标匹配 | `grep -cE '(we show|it is shown|results indicate)' draft.tex` → 注册一致 |
| 5 | Hedging 适度性 | 关键发现有适度 hedging，过度断言 ≤ 5 处 | `grep -cE '(证明了|确定地|无疑)' draft.tex` → ≤ 5 |

## References

- prose-chain-contract：[`../paper-workbench/references/prose-chain-contract.md`](../paper-workbench/references/prose-chain-contract.md)
- prose-quality-gate：[`../paper-workbench/references/prose-quality-gate.md`](../paper-workbench/references/prose-quality-gate.md)
- prose-exemplars：[`../paper-workbench/references/prose-exemplars.md`](../paper-workbench/references/prose-exemplars.md)
- research-language-norms：[`../paper-workbench/references/research-language-norms.md`](../paper-workbench/references/research-language-norms.md)

## Integration

前门 skill 在以下时机内联调用本 skill：

- **paper-workbench**：初稿完成后、投稿前的文稿质量门禁

调用方式：按验证清单逐项执行，FAIL 项回写前门 skill 供 writer lane 修正。
