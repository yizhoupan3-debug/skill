---
name: loop
description: |
  Host-orchestrated adversarial review-fix-verify rounds with progressive rubric disclosure.
  Use when the user says `$loop` / `/loop`, wants multi-pass adversarial criticism and fixes without
  revealing total iteration budget to the model, or references LOOP_ROUND_COMPLETE / progressive review tiers.
  Complements review-fix-verify-loop; loop adds Cursor hook-injected context via router-rs.
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
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
  version: "1.0.0"
  platforms: [codex, cursor]
  tags: [loop, adversarial, review, progressive-disclosure, hooks]
---

# loop（对抗审查渐进披露循环）

显式入口：`/loop`、`$loop`（与 `router-rs` Cursor `BeforeSubmitPrompt` hook 协同）。

宿主（`scripts/router-rs` → `.cursor/hook-state/adversarial-loop-<session>.json`）保存 **`max_rounds` 与 `completed_passes`**，**不向模型提示词注入总轮数或剩余轮数**，仅注入当前序列下的审查层级说明，避免模型「掐点收尾」。

## 何时使用

- 需要多轮 **强对抗审查 → 针对性修复 → 可验证**，且希望 **按轮追加审查维度**（渐进披露）
- 已与用户约定使用 **`LOOP_ROUND_COMPLETE`** 标记一轮闭环，或使用 **`/loop next`** 手动推进

## 何时不要用

- 单轮即可收敛、无验证命令
- 用户明确要求不要用 subagent / 不要多轮

## 用户命令（宿主解析）

| 命令 | 行为 |
|------|------|
| `/loop [N] <goal>` 等 | 初始化：在与 `review-gate` 相同的 **strip**（去代码块/引号等）之后，取**第一条**非空且可解析的 loop 指令行；`N` 默认 `3`，仅写入宿主状态文件 |
| `/loop next` / `$loop next` | 将已完成轮次计数 +1（不超过宿主预算），用于模型未打标时的手动推进 |
| `/loop clear` | 清除本会话的 loop 状态（**仅发送此行**时不触发 `/loop` 的并行/delegation 框架门禁） |

## 模型侧契约

每一轮（序列索引 = `completed_passes + 1`，仅宿主内部使用）建议：**独立 reviewer subagent → fixer → verifier**，边界见 [`review-fix-verify-loop`](../review-fix-verify-loop/SKILL.md)。

一轮结束时，助手须输出**独占一行**的标记（区分大小写，行内不得有其他字符）：

```text
LOOP_ROUND_COMPLETE
```

宿主在 **`AfterAgentResponse`**：对助手回复先 **CRLF→LF**，再经与用户侧门禁相同的 **strip**（去掉 fenced code、行内反引号片段等），最后在剩余正文中做**整行**匹配；命中后将 **`completed_passes`** 加一（未耗尽预算时）。标记只在 **围栏外** 的独占行生效；行内夹杂其它文字**不会**计数。

## Hook 注入内容（摘要）

每轮用户提交时，若会话仍在预算内，宿主追加 **`additional_context`**，包含：

- 当前 **审查层级 tier**（由会话盐与已完成轮次混合选取四类之一，**非**固定 A→B→C→D 周期），**不披露**总轮数或剩余轮数
- 要求本轮最大化对抗性发现；修复必须对应清单；验证必须可执行

预算用尽后注入简短收口提示（仍不泄露数字），引导验证证据与收口，或让用户重新 `/loop ...`。

## 验证

- 配置与契约：`configs/framework/RUNTIME_REGISTRY.json` 中 `framework_commands.loop`
- 运行时：`cd scripts/router-rs && cargo test cursor_hooks`
