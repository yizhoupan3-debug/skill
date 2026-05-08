# decisions

- 长期记忆的 canonical source 是项目内 `./memory/`；`./.codex/memory/` 只允许作为显式 symlink 或本地 override。
- memory recall 与 runtime automation 默认读取同一个 `./memory/` canonical source。
- 短期工件与长期记忆分离；长期层只保留摘要、稳定决策和关键索引。
- 第一性原理是体系级推理约束：任何方案先还原为目标、事实、硬约束、因果链和最小可验证动作。
- 减法原理是体系级约束，适用于路由、记忆、artifact、回复和实现；默认删范围、删层、删输出、删兼容幻想；raw trace 和可重建缓存不进长期记忆。
- Codex CLI 和 Codex App 共享同一套 `skills/` 基础设施，路由以项目内 runtime artifacts 为准。
- `/Users/joe/.codex/AGENTS.md` 是全局 Codex 入口代理，应指向 `/Users/joe/Documents/skill/AGENTS.md` 和项目 runtime，而不是复制一份长期 policy。
- `/Users/joe/.codex/memories/skill/` 是全局 Codex 记忆代理，不是 skill workspace 的 canonical memory root。
- Rust 是路由、host entrypoint、hook、framework runtime 和记忆策略的真源；不引入并行实现。
- 当前任务恢复以 task artifacts、`artifacts/current/active_task.json` 和 `.supervisor_state.json` 为准，不靠聊天记录。
