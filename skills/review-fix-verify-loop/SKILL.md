---
name: review-fix-verify-loop
description: |
  Orchestrate a configurable multi-round self-loop with independent subagents per round (review / optional external research in parallel with review / fix / verify). Use for Codex or Cursor self-loop, 自循环轮次控制, review-fix-verify closed loop, or evidence-driven passes until convergence. Entrypoints $review-fix-verify-loop and /review-fix-verify-loop. Large max_rounds (e.g. 100) require Rust ledger `RFV_LOOP_STATE.json` via stdio op `framework_rfv_loop`.
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
user-invocable: true
disable-model-invocation: true
trigger_hints:
  - $review-fix-verify-loop
  - /review-fix-verify-loop
  - review fix verify loop
  - codex self-loop
  - 自循环
  - 指定轮次
  - review->fix->verify
  - rfv loop
  - 多轮修复
  - 多轮审查
  - 独立 subagent 循环
metadata:
  version: "1.2.0"
  platforms: [codex, cursor]
  tags: [loop, review, fix, verify, subagent, orchestration, external-research, long-running]
---

# review-fix-verify-loop

显式入口（与路由 description 对齐）：`$review-fix-verify-loop`、`/review-fix-verify-loop`。

编排前先过 [`agent-swarm-orchestration`](../agent-swarm-orchestration/SKILL.md)：确认各 lane 可独立、`fix_scope` disjoint、且 `verify_commands` 已定义；若启用外部调研并行，还须明确 **external 与 review 的只读边界** 及汇总责任在 supervisor。不满足则拒绝 spawning 并给出 reject reason。

可复制 lane 与轮次日志模板见 [references/lane-templates.md](references/lane-templates.md)。

## Rust 轮次真源（长任务必用）

当 `max_rounds` 较大（例如 **100**）或跨多会话执行时，必须用 `router-rs` 落盘轮次账本，避免仅靠聊天上下文：

- 文件：`artifacts/current/<task_id>/RFV_LOOP_STATE.json`
- stdio op：**`framework_rfv_loop`**
  - `operation: start` — 字段含 `goal`、`max_rounds`、`allow_external_research`、`parallel_external_with_review`（默认 `true`）、`review_scope`、`fix_scope`、`verify_commands`、`stop_when`
  - `operation: append_round` — 每轮结束后 supervisor 写入：`round`、`review_summary`、`external_research_summary`（可空）、`fix_summary`、`verify_result`（`PASS|FAIL|SKIPPED`）、`supervisor_decision`（`continue|close|block`）、`reason`
  - `operation: status`
- `max_rounds` 在 Rust 侧有 **硬上限 1000**（防止误填天文数字）；超过会截断并在响应中带 `warning`。
- **Cursor**：若 `.cursor/hooks.json` 接入 `router-rs cursor hook`，且 **`RFV_LOOP_STATE.json`** 中 **`loop_status=active`**，Stop / beforeSubmit 可合并 **RFV_LOOP_CONTINUE** 跟进；preCompact 可附带一行 RFV 摘要。关闭注入：`ROUTER_RS_RFV_LOOP_HOOK=0`。

可与宏任务 **`GOAL_STATE.json`** / `framework_autopilot_goal` 同目录并用：目标级续跑 + 轮次级质量闭环。

### 推理深度契约（与可审计链）

- **真源**：[references/reasoning-depth-contract.md](references/reasoning-depth-contract.md) — **不靠单模型拉长 CoT**；靠 **`review ∥ external → fix → verify`** + **`EVIDENCE_INDEX` / `append_round`** 形成可审计链。
- **宿主注入文案**：`configs/framework/HARNESS_OPERATOR_NUDGES.json`（RFV / Autopilot 续跑末尾附带的「推理深度」句）；**`ROUTER_RS_HARNESS_OPERATOR_NUDGES=0`** 可整体关闭。
- **外研不得顶替 verify**：external 只产出可引用结论与假设；**Pass/Fail 只认可执行验证**。

### 可执行验证与 Cursor 钩子（证据自动落盘）

- 连续性已初始化时，**verifier lane** 在终端跑的命令若匹配 `router-rs` 内置启发式（如 `cargo test` / `cargo check` / `pytest` / `verify_cursor_hooks` / `policy_contracts` / `nextest` 等），**Cursor `postToolUse`** 会把 **`cursor_post_tool_verification`** 写入 `EVIDENCE_INDEX.json`（需 `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` 未关闭）。
- `verify_commands` 建议优先选仓库内 **短、确定性** 命令，并与上述启发式有交集，便于 hook 自动记账；冷僻命令可改用语义等价且含关键字的拼法，或依赖 verifier 人工粘贴 `hook-evidence-append`。

## When to use

- 用户明确要求可配置轮次的闭环执行，而不是单轮修复
- 任务适合拆为独立 lane：审查、修复、验证
- 需要每轮都保留 supervisor 集成判断和停机决策
- 需要把失败模式从“感觉完成”改成“证据驱动继续/停止”
- 需要 **外部调研** 与仓库内审查 **对照**（并行或串行均可，但须在契约里写明）
- **`max_rounds` 很大**（如 100）：必须启用 **`framework_rfv_loop` 账本**，并在 `stop_when` 中保留提前收敛条件（见下）

## Do not use

- 小任务在一轮内可稳定完成
- 三个 lane 不能独立（共享强上下文或写入范围重叠严重）
- 缺少可执行验证命令或可观测验证标准
- 用户明确要求不要使用 subagent

## Loop contract

先建立最小契约：

```text
goal:
max_rounds:
round_timeout_hint:
review_scope:
fix_scope:
verify_commands:
stop_when:
```

默认 `max_rounds=3`。**用户显式给定轮次（如 100）时**：以用户值写入 `RFV_LOOP_STATE.max_rounds`，但仍必须满足下列 **提前停机** 规则之一才可 `close`，禁止“刷满轮次才算完”：

`stop_when` 至少包含一条（推荐多条同时启用）：

- verifier 全部通过且无 A 级 blocker
- **连续两轮无实质 delta**（无新修复、无新高优先级发现、外部调研无新的高置信结论）
- 到达 `max_rounds`（耗尽预算）
- 出现明确外部 blocker（缺权限、缺数据、人工确认）
- **外部调研与内部审查结论收敛**（对关键假设/风险登记达成一致或明确存疑点）

## Round execution model

每一轮推荐 **两阶段并行 + 两阶段串行**（在 `allow_external_research=true` 且 `parallel_external_with_review=true` 时）：

**阶段 A（可并行）**

1. `reviewer lane`（独立 subagent，仓库内只读为主）
2. `external research lane`（独立 subagent，仅做 **可引用来源** 的调研；默认 **禁止** 改仓库；与 reviewer **并行**启动）

supervisor **汇总** A 阶段：合并内部 findings 与外部证据，形成本轮唯一 handoff（写入 `append_round` 前的中间笔记即可）。

**阶段 B（串行）**

3. `fixer lane`（独立 subagent，仅允许改 `fix_scope`）
4. `verifier lane`（独立 subagent，执行 `verify_commands`；默认只报告不修复）

**阶段 C**

5. supervisor 合并证据，**`framework_rfv_loop` `append_round`**，并决定：
   - close（完成）
   - continue（下一轮）
   - block（输出 blocker 与 next action）

每个 lane 必须 **每轮新起独立 worker**，不复用同一会话在 reviewer / external / fixer / verifier 间切换。

## Lane prompt contract

每个 lane 的提示必须包含：

- lane goal（本轮目标）
- allowed scope / forbidden scope（文件或模块边界）
- required output format（固定字段）
- verification evidence requirement（命令、日志、差异）

统一输出格式：

```text
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

## Supervisor responsibilities

- 维护每轮状态：`round`, `key_findings`, `applied_fixes`, `verification_result`
- 防止 lane 写入重叠；发现重叠时暂停并重分配范围
- 仅 supervisor 决定是否进入下一轮
- 最终输出包含：
  - 轮次执行摘要
  - 最终验证证据
  - 残余风险或 blocker

## Minimal execution checklist

```text
- [ ] contract complete
- [ ] round 1 review/fix/verify done
- [ ] convergence decision recorded
- [ ] if needed, next round started with updated scopes
- [ ] final verification evidence attached
```
