---
name: implementx
description: |
  Personal lifecycle — execute ALL waves in one breath. Main thread schedules lanes only; subagents write compact lane-notes.
  Sets drive_until_done true. REVIEW_GATE hard block off under lifecycle_profile my-light.
  Use for /implementx after /planx.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_gate_evidence: "ROADMAP.md and WAVE_STATE.json exist"
routing_priority: P1
session_start: n/a
user-invocable: true
disable-model-invocation: true
risk: low
source: local
trigger_hints:
  - /implementx
  - implementx
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [my-lifecycle, implement, multi-agent, one-breath]
---

# implementx

**Zone**: execution+ · **profile**: `my-light`

Under **`lifecycle_profile: my-light`**, Cursor **Stop** does **not** emit hard `router-rs AG_FOLLOWUP` (goal continuity is manual: `framework_goal_drive` stdio + `artifacts/current/<task_id>/` boards). **`beforeSubmit` does not arm `goal_required`** (uses `goal_drive_entry_active` for pre-goal only). Closeout / `CLOSEOUT_FOLLOWUP` may still apply when completion is claimed.

## One-breath all-waves (HARD)

When invoked, run **every wave** in `WAVE_STATE.json` from current `wave_id` through the last wave **without** stopping at wave boundaries to ask the user.

| CAN continue (no user ping) | MUST stop |
|----------------------------|-----------|
| Next lane in parallel group | Scope/requirement error |
| Next wave after merge checkpoint | P0 security |
| Verification failed, fix obvious | External dependency down |
| Retry with new evidence | User said stop |

**Do not** treat “Wave N complete” as a pause point.

## Main thread (scheduler only)

1. Read `WAVE_STATE.json` + `ROADMAP.md`
2. For each wave (in order): spawn all lanes in `parallel_group` when `execution_mode=parallel`.
   - **Antigravity 强化约束**：在 `antigravity` 宿主下，若任务包含多文件或跨模块设计（且总体 Delta > 50 行），**必须优先选用并行模式并派生子代理**。主线程严格担任 scheduler 角色。**例外豁免**：(1) 当子代理遭遇并发故障、模型或 Region 不可用时，允许优雅降级为串行主线程；(2) 允许主线程在 Verification 阶段对于简单错误执行 "fix obvious" 自愈，其余实质性代码编写必须分流并发。
3. Merge: read `lane-notes/<lane_id>.md` only; chat ≤3 bullets + paths
4. Update wave `status` → `completed`; `current_wave`++; checkpoint `EVIDENCE_INDEX`
5. After final wave → suggest `/verifyx` (or auto-chain if user asked full pipeline)

Target: coordinator visible content ≤35% of turn.

## Subagent contract

```json
{
  "lane_id": "w3-lane-cursor",
  "scope_paths": ["core/router-rs/src/hosts/cursor_hooks/"],
  "output_path": "artifacts/current/<task_id>/lane-notes/w3-lane-cursor.md",
  "max_lines": 15,
  "forbidden": ["paste full transcript to main chat"]
}
```

Prefer `fork_context=false`, disjoint paths, 3–5 parallel lanes when plan allows.

**Model (Cursor)**: omit `Task` `model` (inherit parent session); do not default `claude-*` / `sonnet*`. See `.cursor/rules/subagent-model-inherit.mdc`.

**Model (CLI/Terminal)**: inherit主会话模型，不显式指定。CLI 端积极鼓励并行 lane——≥2 独立子问题时**应** spawn 子代理，通常 3–5 个 `fork_context=false` lane，不因缺少 hook 级子代理事件而退缩为主线程串行。详见 [`docs/hosts/codex-cli.md` § 多代理编排](../../docs/hosts/codex-cli.md)。

## Spec-driven 并行构建

当任务可分解为独立子模块时，使用 spec-driven 模式提升执行效率。

### 三阶段管线
1. **Spec 阶段**：将需求转化为结构化规格说明（输入/输出/约束/验收标准）
2. **并行 Build 阶段**：将 spec 分配给多个 subagent 并行实现
   - 每个 subagent 接收独立的 spec 片段
   - subagent 之间无依赖，可完全并行
   - 使用 worktree 隔离避免文件冲突
3. **集成验证阶段**：收集所有 subagent 产物，运行集成测试

### 与 Wave Execution Model 对齐
- 每个 wave 可包含多个 spec-driven 并行构建
- Wave 间仍保持串行依赖
- Spec 文件存储在 artifacts/current/ 下

### 何时使用
- 任务可分解为 3+ 个独立子模块
- 子模块之间接口明确
- 有明确的集成测试方案

### 何时不用
- 子模块之间强耦合
- 需要共享状态或增量修改
- 只有 1-2 个子任务

## GOAL_STATE writes (HARD)

遵循 [../my-lifecycle-common/GOAL_STATE_CONTRACT.md](../my-lifecycle-common/GOAL_STATE_CONTRACT.md) 中的 GOAL_STATE 写入规范。

implement 阶段特殊字段：`drive_until_done: true`；无 Stop `GOAL_CONTINUE` hook 注入（2026-05 连续性拔除）。

## Next

`/verifyx` — evidence + ship in one command.
