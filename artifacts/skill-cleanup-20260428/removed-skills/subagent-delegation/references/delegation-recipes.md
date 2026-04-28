# Delegation Recipes

这份参考文件给 `subagent-delegation` 提供可直接复用的派单配方。

目标不是生成华丽 prompt，而是让 sidecar 或 local-supervisor queue 的任务边界更稳、回收结果更容易。

## Core note

优先决定 **sidecar 结构**，再决定当前 runtime 能否真的 `spawn_agent`。

在进入 runtime 分支前，先把 delegation plan 写入 state 或 artifact。

如果当前 runtime 不允许 `spawn_agent`：

- 不要放弃 delegation 思维
- 保留同样的任务切片
- 改为 local-supervisor queue
- 主线程仍然只输出简短调度摘要

## Recipe 1 — Read-only explorer

适用：仓库调查、文档调查、风险盘点、证据收集

输出契约：

- target
- findings
- evidence
- confidence
- next-step recommendation

## Recipe 2 — Bounded worker

适用：小范围实现、定点修复、局部验证

输出契约：

- scope completed
- files changed
- risks
- follow-up
- ready-for-integration

## Recipe 3 — Verification sidecar

适用：测试、审计、截图检查、artifact 检查

输出契约：

- verification target
- evidence collected
- pass/fail summary
- rework scope

## Recipe 4 — Local-supervisor queue item

适用：delegation 结构成立，但当前 runtime 不允许 spawning

输出契约：

- delegation_plan_created
- queue item goal
- bounded scope
- expected output
- integration note

策略：

- 保留原 sidecar 边界
- 逐项本地执行
- 每项完成后只回收摘要
- 细节写入 state 或 artifact

## Reuse strategy

策略：

- 第一次真实可派发时用 `spawn_agent`
- 后续同类任务优先 `send_input`
- 不再需要时 `close_agent`
- 若 runtime 不允许 spawning，则继续沿用 local-supervisor queue，而不是重写整体结构
