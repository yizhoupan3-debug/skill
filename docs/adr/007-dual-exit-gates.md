---
last_verified: "2026-06-22"
depends_on:
  - ../spec.md
  - ../spec-closeout.md
---

# ADR-007: 双门退出机制 — 核查门与质量门

## Status

Accepted (2026-06-22).

## Context

当前 closeout 逻辑存在三个问题：

1. **三处实现碎片化**：closeout gate、review gate、Goal complete 三处退出逻辑各有独立实现，但功能重叠（都检查 blockers、验证条件），修改一处需同步另外两处。
2. **Goal complete 防欺诈不足**：任一工具/子代理可调用 `goal_state_manage operation=complete` 直接完成 Goal，不经过证据检查和质量验证。
3. **Goal↔RFV 互斥关系错误**：Goal 完成时自动关闭 RFV 循环（互斥），但实际需要的是 Goal 完成前先通过 RFV 检查——Goal 应从属于 RFV，而非互斥。

## Decision

引入**双门退出机制**，将退出检查从 Goal 自身剥离：

1. **核查门（Verification Gate）**：
   - 职责：验证实现是否满足 Goal 定义的 `done_when` 条件
   - 独立工具：`verification_gate_check`（非 `goal_state_manage`）
   - 触发时机：`goal_state_manage operation=complete` 自动触发
   - 不通过则阻断 complete，返回缺失项列表

2. **质量门（Quality Gate，即原有 RFV 循环）**：
   - 职责：多轮对抗式质量审查，确保输出质量
   - 独立工具：`quality_gate_manage`
   - 从属关系：Quality Gate 从属于 Goal，而非互斥
   - Goal complete 前必须通过 Quality Gate（至少 PASS 或 SKIPPED）

3. **Goal↔RFV 关系变更**：
   - 旧模型：Goal ↔ RFV（互斥，完成一个自动关闭另一个）
   - 新模型：Goal ← Quality Gate（单向从属，Quality Gate 是 Goal 完成的前置条件）
   - Goal complete 时检查 Quality Gate 状态，未通过则拒绝

4. **接口变更**：
   - `goal_state_manage operation=complete` 增加隐式校验：自动调用 `verification_gate_check` 和 `quality_gate_status`
   - 新增 `verification_gate_check` 工具
   - `quality_gate_manage` 保持独立，增加 `status` 查询模式

## Consequences

- **优势**：
  - 单一退出路径：所有完成操作都经过双门检查
  - Goal complete 不再可直接绕过质量验证
  - 消除碎片化：三处退出逻辑统一为双门机制
  - 模型更清晰：Quality Gate 从属于 Goal，不再互斥
- **代价**：
  - Goal complete 增加额外 RTT（两次门检查），属可接受开销
  - 现有调用 `goal_state_manage operation=complete` 的工具需兼容新校验
  - SKIPPED 状态需要明确定义：何时可跳过质量门
- **迁移**：`goal_state_manage` 先软实现（告警而非阻断），一个 release 后硬阻断。

## Related

- `docs/spec.md` §6 — Lifecycle 规约
- `docs/spec-closeout.md` — Closeout 规约
- `artifacts/current/roadmap-v8.md` §3 — 退出机制重构
