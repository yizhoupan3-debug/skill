# /workflow

编排意图入口（五宿主 skill 路由一致；Cursor 无 native `import "workflow"` 运行时）。

1. Read `skills/agent-swarm-orchestration/SKILL.md`（Workflow triggers / Main-thread contract）。
2. 按 [`skills/agent-swarm-orchestration/references/workflow-supervisor-protocol.md`](../../skills/agent-swarm-orchestration/references/workflow-supervisor-protocol.md) 以 **workflow_supervisor** 模式调度；真源 phases：`.claude/workflows/<name>.js` → `meta.phases`。
3. 首行输出：`orchestration: { mode: workflow_supervisor, trigger, reason }`。
