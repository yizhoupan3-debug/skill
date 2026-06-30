# Agent Policy (Cross-Host)

跨宿主叙述性协议真源。

## Language

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外），自然学术中文。
- 仅当用户当轮明确要求英文时才可切换。
- **回答避免空话**；**对不确定的信息直接说明**，严禁凭空编造。

## Coding First Principles

- 五门槛：Goal / Non-goals / Existing owner / Minimal delta / Validation。
- 减法优先；禁止为不确定未来加抽象；证据收口（测试/diff/blocker）。

## Git

- 未经用户明确要求不得创建分支/worktree；只读检查现有状态。
- **Worktree 隔离（硬约束）**：未经用户当轮显式批准，禁止在 git worktree 中运行或修改任何文件。

## Review

- **Review findings-only by default**: review 产出仅为 findings（P0→P1→P2→Caveat），不默认改代码、不执行。参见 `skills/code-review-deep/SKILL.md`。
- Closeout: 完成时使用 `goal_state_manage(operation=complete)` 记录 closeout evidence。
- Skill Routing: 使用 `skill_route` / `skill_search` MCP 工具进行路由。
- Goal state: 通过 `goal_state_read` / `goal_state_manage` 管理目标状态。

## Structured Task Output（TASK_OUTPUT.json）

每个 task 完成后自动产出 `TASK_OUTPUT.json`（schema `task-output-v1`），位于 `artifacts/current/<task_id>/TASK_OUTPUT.json`。

### MCP 工具

| 工具 | 功能 |
|------|------|
| `task_output_write` | 写入 TASK_OUTPUT.json（含 closeout 嵌入） |
| `task_output_read` | 读取 TASK_OUTPUT.json |
| `task_output_init` | 初始化空的运行中 TASK_OUTPUT.json |
| `task_output_pull` | 拉取前置 task 输出到当前 task（`consumed_inputs`） |
| `task_output_validate` | 校验 TASK_OUTPUT.json 字段完整性 |
| `chain_aggregate` | 手动触发生成 CHAIN_OUTPUT.json |

**自动集成**：`task_create` 自动 init；`closeout_record_write` 自动同步 closeout → TASK_OUTPUT。

### 输出字段

```
outputs.changed_files        — Vec<String>
outputs.verification_status  — "passed"|"failed"|"partial"|"not_run"
outputs.summary              — String
closeout                     — closeout-record-v1 (完成时嵌入)
consumed_inputs              — Vec<{source_task_id, fields, ...}>
aggregates                   — {parent_chain_id, chain_index}
```

## Chain Engine DAG（task 级编排）

`chain_dag_*` 工具族用于创建和管理带有依赖图的 DAG 链。`TASK_CHAIN.json` 格式扩展支持 `chain-dag-v1`，向后兼容旧格式。

### MCP 工具

| 工具 | 功能 |
|------|------|
| `chain_dag_init` | 创建 DAG 链（含循环检测 + 引用验证） |
| `chain_dag_tick` | 手动推进一次 DAG 调度 |
| `chain_dag_status` | 读完整链状态视图（含拓扑层级） |
| `chain_dag_retry` | 手动重试特定 task |
| `chain_dag_skip` | 跳过特定 task |
| `chain_dag_resume` | 恢复 paused DAG |

### TASK_CHAIN.json DAG Schema

```json
{
  "schema_version": "chain-dag-v1",
  "chain_id": "fix-bugs",
  "mode": "dag",
  "tasks": [{
    "task_id": "scan",
    "depends_on": [],
    "condition": null,
    "parallel_group": null,
    "retry": { "max_attempts": 3 }
  }],
  "global_config": {
    "max_concurrent_tasks": 4,
    "on_any_failure": "pause_dag"
  }
}
```

**字段说明**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `depends_on` | `Vec<String>` | 上游 task_id 列表 |
| `condition` | `DagCondition` | 条件门控：`{source, type, field, operator, value}` |
| `parallel_group` | `Option<String>` | 同组 task 可并行执行 |
| `timeout_group` | `TimeoutGroupSpec` | 超时组：`{group_id, max_seconds}` |
| `retry` | `RetryPolicy` | 重试策略：`{max_attempts, backoff_base_ms, ...}` |

### 调度算法

幂等轮询，每次完整重算 DAG 状态（崩溃安全）：
1. expired backoff → pending
2. 收集所有可调度 task（依赖满足 + 条件通过）
3. 按 parallel_group round-robin 公平选择，达 capacity 停止
4. 超时组到期 → failed；失败策略 → abort/pause/continue

