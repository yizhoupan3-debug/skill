# decisions

## 2026-04-11

- 采用四层系统：短期工作记忆、长期记忆、自进化层、自动化层。
- 长期记忆目录优先使用项目内 .codex/memory/，回退到 ~/.codex/memories/<workspace>。
- 短期工件与长期记忆分离，避免 raw trace 无限膨胀。
- sessions / archived_sessions 作为原始上下文源，长期只保留摘要和关键索引。
- logs_1.sqlite、maintenance-backups、可重建缓存只做 report-first 管理。
- 将 routing_checkpoint / evolution_journal 视为自进化原始事件流。
