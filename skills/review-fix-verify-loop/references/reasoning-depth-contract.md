# 推理深度契约（非 CoT）

## 原则

**不靠单模型拉长 CoT**；**靠 `review ∥ external → fix → verify` 的结构化分工**，并把验证过程落到 **`EVIDENCE_INDEX`（及每轮 `append_round`）**，形成 **可审计链**。

## 含义

| 做法 | 说明 |
|------|------|
| **分工** | review（内审）与 external（外研）可并行；fix 只动约定范围；verify 只跑约定命令、只报结果（默认不修）。 |
| **深度** | 来自多视角对照 + **可执行验证**，不是单线程 prose 变长。 |
| **审计** | 终端类验证命令在连续性就绪时可由 hook 写入 `EVIDENCE_INDEX`；每轮 RFV 决策必须 `append_round` 落盘。 |

## Supervisor 自检（每轮）

- [ ] A 阶段是否 **并行** 仅包含 **只读** lane（review + optional external）？
- [ ] **verify** 是否对应明确 `verify_commands`，且 **PASS/FAIL** 有命令/日志而非「感觉通过」？
- [ ] 本轮是否写入 **`append_round`**（含 `verify_result`）？

## 反模式

- 用外研长文代替 verifier 跑命令。
- 单 agent 在同一上下文里轮流扮演 reviewer/fixer/verifier 却声称「多 lane」。
- 完成叙事不指向 `EVIDENCE_INDEX` 或等价 exit code 记录。
