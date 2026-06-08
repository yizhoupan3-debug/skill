# Workflow supervisor protocol（Cursor / 无 native workflow 宿主）

**真源 phases**：`.claude/workflows/<name>.js` → `export const meta = { phases: [...] }`。

Cursor 等宿主**无** `import "workflow"` 运行时；命中 workflow 编排意图时主线程为 **workflow_supervisor**（见 `.cursor/rules/workflow-orchestration-gate.mdc`）。

## 主线程契约

1. **只调度**：每 phase 写 `artifacts/current/<task_id>/lane-notes/phase-<slug>.json`（机读摘要，非子代理全文）。
2. **读 `meta.phases`**：按顺序执行；phase 内 `parallel` / `pipeline` / `agent` 语义见 [`workflow-script-conventions.md`](workflow-script-conventions.md)。
3. **禁止**在聊天粘贴子 agent 全文；findings 用 compact 列表。
4. **首行机读**（workflow 轮次）：`orchestration: { mode: workflow_supervisor, trigger, reason }`。

## Phase 笔记 JSON（最小）

```json
{
  "phase_id": "search",
  "status": "done",
  "summary": "3 queries, 12 URLs retained",
  "blockers": [],
  "next": "fetch"
}
```

## 与科研 harness 的衔接

- **deep-research**：`.claude/workflows/deep-research.js` — plan searches → fetch → verify → synthesize。
- 科研 NL 路由仍优先 `$research-workbench` / `$deep-research`；workflow 为**显式** `/workflow` 或编排意图加深路径。
- 五宿主 skill 路由一致；仅 **Claude Code** 可 native 执行 workflow JS，其余宿主用本协议仿真。

## 失败时

- Phase 失败：在 phase 笔记标 `blocker` + 最小复现；不静默跳过。
- 缺 `meta.phases`：停止并报告 workflow 文件路径，不即兴编排。
