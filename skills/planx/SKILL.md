---
name: planx
description: |
  Personal lifecycle — plan/roadmap (doc-only). Writes ROADMAP.md and WAVE_STATE.json with explicit serial/parallel DAG.
  Use when user explicitly requests plan after /discussx. Does not mutate product code.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_gate_evidence: "REQUIREMENTS.md exists"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /planx
  - planx
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [my-lifecycle, plan, waves]
---

# planx

**Zone**: pre-execution · **profile**: `my-light`

**Entry gate**: user must explicitly invoke `/planx` (or clear plan intent) after `/discussx`; do not enter from agent nudge alone.

## When to use vs plan-mode

- **planx**（本 skill）：my lifecycle 标准计划层，产出 `ROADMAP.md` + `WAVE_STATE.json`，必须在 `/discussx` 之后由用户显式触发。**不适用于 Cursor Plan 模式**。
- **plan-mode**（`skills/plan-mode/`）：跨宿主 Plan 闸门，覆盖 Cursor CreatePlan、调研计划、可验收 todo。**不适用于 my lifecycle 的 wave/phase 编排**。
- 两者**互斥**：同一任务只用其一。my lifecycle 用 planx，Cursor Plan 模式用 plan-mode。

## Do not use

- 用户在 Cursor Plan 模式下需要策划文档闸门 → 使用 `plan-mode`
- 任务不涉及 my lifecycle 的 phase/wave 规划 → 使用 `plan-mode` 或直接实现

**Inputs**: `REQUIREMENTS.md`, `DECISIONS.md`, `OPEN_QUESTIONS.md` (carry unresolved items into plan scope or wave notes).

## Disk outputs

| File | Purpose |
|------|---------|
| `artifacts/current/<task_id>/ROADMAP.md` | Phases, exit criteria, verification commands |
| `artifacts/current/<task_id>/WAVE_STATE.json` | Each wave: `parallel_group`, `depends_on`, `execution_mode`, `lanes[]` |
| `artifacts/current/<task_id>/GOAL_STATE.json` | Via `framework_goal_drive` stdio — 遵循 [../my-lifecycle-common/GOAL_STATE_CONTRACT.md](../my-lifecycle-common/GOAL_STATE_CONTRACT.md) 中的 GOAL_STATE 写入规范 |
| `artifacts/current/<task_id>/PLAN_TRACE.md` | 计划版本历史与执行进度追踪（人机可读） |

### GOAL_STATE writes

遵循 [../my-lifecycle-common/GOAL_STATE_CONTRACT.md](../my-lifecycle-common/GOAL_STATE_CONTRACT.md) 中的 GOAL_STATE 写入规范。

## 持久化计划文件模式

增强 planx 的跨会话状态追踪能力。

### 计划文件结构
- 主文件：ROADMAP.md（已有）
- 状态文件：WAVE_STATE.json（已有）
- 新增：PLAN_TRACE.md — 计划版本历史和执行进度

### PLAN_TRACE.md 格式

```markdown
# Plan Trace: {项目名}

## v1 — 初始计划
- 创建时间: {timestamp}
- 阶段数: N
- 关键决策: ...

## v1.1 — 执行调整
- 调整原因: {wave N 发现新依赖}
- 变更: {新增/删除/重排的任务}
- 影响范围: {哪些后续 wave 受影响}

## 执行进度
| Wave | 状态 | 开始 | 完成 | 备注 |
|------|------|------|------|------|
| W1 | done | ... | ... | |
| W2 | in_progress | ... | - | 进行中 |
| W3 | pending | - | - | |
```

### 跨会话恢复
- session 启动时读取 PLAN_TRACE.md 恢复进度
- 从最后一个 checkpoint 继续
- 无需用户重复描述上下文

### 与现有产物的兼容
- ROADMAP.md：高层阶段规划（不变）
- WAVE_STATE.json：当前 wave 机器可读状态（不变）
- PLAN_TRACE.md：新增的人机可读完整追踪记录

## Outputs (schema)

Topology fields (schema id **`my-wave-state-v1`**; field manifest [`configs/framework/WAVE_STATE_FIELDS.json`](../../configs/framework/WAVE_STATE_FIELDS.json)):

| Field | Meaning |
|-------|---------|
| `depends_on` | Prior `wave_key` values (serial edge) |
| `parallel_group` | Lanes in same wave that may run together |
| `execution_mode` | `parallel` \| `serial` |
| `lanes[].scope_paths` | Disjoint write scopes per lane |

## Optional review

At most **one** read-only reviewer on `ROADMAP.md` → compact `lane-notes/` only (no mandatory RFV).

## Next

`/implementx` — executes **all waves** in one breath (see `skills/implementx/SKILL.md`).
