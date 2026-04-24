# decisions

- 长期记忆使用项目内 `./.codex/memory/`，当前映射到 `./memory/`。
- 短期工件与长期记忆分离；长期层只保留摘要、稳定决策和关键索引。
- 第一性原理是体系级推理约束：任何方案先还原为目标、事实、硬约束、因果链和最小可验证动作。
- 减法原理是体系级约束，适用于路由、记忆、artifact、回复和实现；默认删范围、删层、删输出、删兼容幻想；raw trace 和可重建缓存不进长期记忆。
- 三 CLI 共享同一套 `skills/` 基础设施，路由以项目内 runtime artifacts 为准。
- Rust 是路由、host entrypoint、hook、framework runtime 和记忆策略的真源；不恢复旧 Python 桥接/并行实现。
- 当前任务恢复以 task artifacts、`artifacts/current/active_task.json` 和 `.supervisor_state.json` 为准，不靠聊天记录。
