# 项目长期记忆

只放跨会话稳定事实；当前任务态看 `artifacts/current/<task_id>/` 和
`.supervisor_state.json`。

## 稳定事实

- 仓库：`/Users/joe/Documents/skill`。
- 三个宿主 Codex / Claude / Gemini 共享 `skills/`、`AGENT.md` 和
  `./.codex/memory/`；本仓库的 memory 物理落点是 `./memory/`。
- Rust 是路由、host entrypoint、hook、framework runtime 和记忆策略的真源；
  不恢复旧 Python 桥接/并行实现。
- 当前任务恢复以 task artifacts 与 `.supervisor_state.json` 为准，不靠聊天记录。

## 操作约束

- 明确任务直接做安全、可回退的小步；外部发布、账号操作、破坏性动作先确认。
- Skill 变更后用 `scripts/skill-compiler-rs` 重新生成路由产物并跑对应测试。
- 复杂任务只沉淀摘要、决策、证据索引；不要把 raw trace 或可重建缓存写进长期记忆。
