# decisions

- 长期记忆使用项目内 `./.codex/memory/`，当前映射到 `./memory/`。
- 短期工件与长期记忆分离；长期层只保留摘要、稳定决策和关键索引。
- 三 CLI 共享同一套 `skills/` 基础设施，路由以项目内 runtime artifacts 为准。
