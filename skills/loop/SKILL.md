---
name: loop
description: |
  Adversarial multi-pass review-fix-verify with progressive rubric disclosure (supervisor-led).
  Use when the user says `$loop` / `/loop`, wants multi-pass adversarial criticism and fixes without
  revealing total iteration budget to the model, or references LOOP_ROUND_COMPLETE / progressive review tiers.
  Long-running round ledgers use the implicit Rust runtime `framework_rfv_loop` → `RFV_LOOP_STATE.json` (not a hot-routed skill).
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - $loop
  - /loop
  - adversarial loop
  - progressive disclosure review
  - LOOP_ROUND_COMPLETE
  - 对抗审查循环
  - 渐进披露审查
metadata:
  version: "1.0.1"
  platforms: [codex, cursor]
  tags: [loop, adversarial, review, progressive-disclosure, hooks]
---

# loop（对抗审查渐进披露循环）

显式入口：`/loop`、`$loop`（**监督者**发起；模型默认不可自触发本 skill）。

**宿主 hook 状态（重要）**：历史上的 Cursor `adversarial-loop-<session>.json` 注入路径已从 `router-rs` 移除（仅保留 SessionEnd 清扫与路径常量）。下面的 **轮次标记与 lane 契约仍可作为人工/监督协议**；长轮次、跨会话请用 **`framework_rfv_loop`** 写 `RFV_LOOP_STATE.json`（见 harness 参考 [`rfv_loop_harness.md`](../docs/rfv_loop_harness.md)，**不在热 skill 路由**）。

## 何时使用

- 需要多轮 **强对抗审查 → 针对性修复 → 可验证**，且希望 **按轮追加审查维度**（渐进披露）
- 已与用户约定使用 **`LOOP_ROUND_COMPLETE`** 标记一轮闭环，或使用 **`/loop next`** 手动推进（若你本地恢复了对应解析；否则仅作协议文本）

## 何时不要用

- 单轮即可收敛、无验证命令
- 用户明确要求不要用 subagent / 不要多轮

## 用户命令（协议 / 历史宿主解析）

| 命令 | 行为 |
|------|------|
| `/loop [N] <goal>` 等 | 初始化（历史）：strip 后取第一条可解析 loop 行；`N` 默认 `3` |
| `/loop next` / `$loop next` | 手动推进已完成轮次（历史宿主计数） |
| `/loop clear` | 清除本会话的 loop 状态（仅发送此行时可避免并行/delegation 门禁误触） |

## 模型侧契约

每一轮建议：**独立 reviewer subagent → fixer → verifier**；深度与证据链见 [`reasoning-depth-contract.md`](../docs/references/rfv-loop/reasoning-depth-contract.md) 与 lane 模板 [`lane-templates.md`](../docs/references/rfv-loop/lane-templates.md)。数理题另见 [`math-reasoning-harness.md`](../docs/references/rfv-loop/math-reasoning-harness.md)。

一轮结束时，助手可输出**独占一行**标记（区分大小写，行内不得有其他字符）：

```text
LOOP_ROUND_COMPLETE
```

## Hook 注入内容（历史摘要）

历史行为：在预算内于用户提交时追加 `additional_context`（tier + 对抗强度要求），**不披露**总轮数。当前树默认 **无** 该注入；需要同等能力时请用 **RFV 账本 + 续跑块**（`RFV_LOOP_CONTINUE`）或恢复 router 中的 adversarial-loop 实现。

## 验证

- 轮次账本与 stdio：`framework_rfv_loop`（`cd scripts/router-rs && cargo test` 覆盖 RFV）
- Cursor hook 回归：`cd scripts/router-rs && cargo test cursor_hooks`
