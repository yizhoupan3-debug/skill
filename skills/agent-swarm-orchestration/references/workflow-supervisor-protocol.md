# Workflow supervisor 协议（非 Claude 宿主）

当 `orchestration.mode = workflow_supervisor` 时，主线程 **不** 运行 `import 'workflow'`，但 **必须** 与目标 JS 脚本的 `meta.phases` 与 `parallel`/`pipeline`/`agent` 结构 **同构** 执行。

真源脚本：仓库 [`.claude/workflows/`](../../../.claude/workflows/)（优先已有模板；可当场生成后保存）。

## 准备

1. 选定脚本（见 [workflow-template-catalog.md](./workflow-template-catalog.md)）或复制 `deep-review-template.js` 改 `LENSES`。
2. 读取 `export const meta` 与 `LENSES` / 各 `phase('…')` 顺序。
3. 输出 `orchestration: { mode: workflow_supervisor, trigger, reason }`。

## 按 phase 执行

| JS 构造 | Supervisor 动作 |
|---------|-----------------|
| `phase('Scan')` + `pipeline([() => agent(...)])` | **串行** spawn **只读** Task，每个 lens 一路；`fork_context=false`；prompt 首行简体中文；schema 对齐 `FINDINGS_SCHEMA` |
| `phase('Merge')` | **主线程** 跑 `conservativeMerge` 或等价逻辑（读 lane 输出 JSON），**不** spawn |
| `phase('Verify')` + `agent(...)` | **单 agent** 批量验证所有 findings；`BATCH_VERDICT_SCHEMA` 含 `finding_index` 回映射 |
| `phase('Synthesize')` | **主线程** 排序、分 confirmed/rejected、写报告路径；findings-first |

## Phase 产物（HARD）

每 phase 结束写入：

`artifacts/current/<task_id>/lane-notes/phase-<slug>.json`

JSON 形状见 `configs/framework/WORKFLOW_LANE_NOTES_SCHEMA.json` (removed)（`phase`、`agents_run`、`agents_failed`、`findings_count`、`artifact_paths`）。

```json
{
  "phase": "Scan",
  "agents_run": 3,
  "agents_failed": 0,
  "findings_count": 12,
  "artifact_paths": ["artifacts/.../scan-raw.json"]
}
```

（≤15 行等价信息即可。）

## 主线程禁止

- 在聊天粘贴子 Task 全文
- 跳过某 `phase()` 或把 Verify 改为 parallel
- 在 Merge/Synthesize 阶段 spawn agent（除非脚本明确例外——默认禁止）

## Cursor 并行度

- 单轮 parallel 块 ≤ 宿主实际并行上限；超出则 **分批** 执行同一 `parallel` 块，仍在同一 phase 内完成后再 Merge。
- Task **省略 `model`**（继承主会话）。

## 完成

- 最终用户可见：admission 一行 + 各 phase 一行 + **findings-first** 列表（severity 排序）。
- Audit 默认 **不** 自动 `/implementx`；用户要求修复时再 handoff。

## 自检清单（supervisor 审 JS 脚本时用）

1. 四阶段齐全：Scan、Merge、Verify、Synthesize  
2. `parallel` 仅用于执行/搜索（plan/review 不用）；Scan 串行 pipeline  
3. Merge 主线程；保守去重（file+lens+行重叠）  
4. Verify 用单 `agent()` + `BATCH_VERDICT_SCHEMA`；`finding_index` + `is_real` + `reasoning` required  
5. Synthesize 含 `coverage` 或等价计数  
6. findings 必含 `evidence`  

详见 [workflow-script-conventions.md](./workflow-script-conventions.md)。
