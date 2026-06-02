# P3 评估备忘：Auto Memory 机制集成评估

> 生成日期：2026-06-02
> 状态：待评估（不阻塞任何 Phase）

## 背景

Claude Code 内置 Auto memory 机制——agent 从用户纠正中自动学习，写入 `~/.claude/memory/` 跨会话持久化。本框架已有独立的连续性机制：

| 本框架机制 | 用途 | 持久性 |
|-----------|------|--------|
| `SESSION_SUMMARY.md` | 会话进展摘要 | 每次 Stop hook 写入 |
| `NEXT_ACTIONS.json` | 下一步行动 | 每次 Stop hook 写入 |
| `EVIDENCE_INDEX.json` | 验证证据 | 生命周期内累计 |
| `GOAL_STATE.json` | 任务状态 | MCP 工具层管理 |
| `PLAN_TRACE.md` | 计划版本历史 | planx 维护 |

## 评估问题

1. **Auto memory 与 SESSION_SUMMARY 是否冲突？**
   - Auto memory 写入用户级别的记忆文件（偏好、模式）
   - SESSION_SUMMARY 写入项目级别的会话摘要
   - 结论：**不冲突**，两者作用域不同（用户 vs 项目）

2. **是否需要集成 auto memory 到 lifecycle？**
   - 当前 lifecycle skill 不读取也不写入 auto memory
   - Auto memory 是 Claude Code 原生行为，无需 skill 主动管理
   - 结论：**无需集成**，auto memory 作为独立层运行

3. **潜在改进**：
   - 可在 `skill-framework-developer` 中添加 auto memory 敏感指令（如 "从用户纠正中学习 skill 路由偏好"）
   - 可在 AGENTS.md 中文档化 auto memory 与框架连续性机制的分工

## 建议

- **不阻塞**任何现有工作
- 短期：文档化两者分工（在 AGENTS.md 或 docs/ 中）
- 中期：评估 auto memory 是否可以改善 skill 路由准确率（需要用户反馈数据）

## 参考

- Claude Code memory 文档: https://code.claude.com/docs/en/memory
- 本框架连续性架构: docs/harness_architecture.md
