# Agent Policy Reference（按需加载）

> 本文件包含 Structured Task Output 和 Chain Engine DAG 的详细文档。
> 不自动加载到上下文——仅在使用相关功能时由模型按需读取。

---

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
