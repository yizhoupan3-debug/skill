# Workflow 模板目录（`.claude/workflows/`）

> Claude 可 `/workflows` 保存为命令；其他宿主作 **supervisor** 编排真源引用同一文件。

| 文件 | `meta.name` | 阶段概要 | 适用场景 |
|------|-------------|----------|----------|
| `deep-review-template.js` | deep-review-template | Scan → Merge → Verify → Synthesize | **复制起点**；多 lens 深度审查 |
| `claude-chain-deep-review.js` | claude-chain-deep-review | 四阶段 | Claude 全链路 hooks/安装/文档 |
| `claude-code-cli-audit.js` | claude-code-cli-audit | 并行审计 → 综合报告 | CLI 使用问题 |
| `hook-route-deep-audit.js` | hook-route-deep-audit | Audit → Verify → Plan → Fix → Clean | hook/路由深度审计+修复 |
| `hook-guard-audit.js` | hook-guard-audit | Investigate → Cross-ref → Test → Plan → Review | PreToolUse guard |
| `full-audit-closeout.js` | full-audit-closeout | 11 阶段大规模审计 | 全仓收口 |
| `full-closeout-audit.js` | full-closeout-audit | 16 phase | 升级后全量审计/合并 |
| `batch1-p0-fixes.js` | batch1-p0-security-fixes | P0修复 → 验证 | 安全批修 |
| `workflow-helpers.js` | — | 共享 schema/merge | import 用，非独立命令 |

## 选用建议

| 需求 | 模板 |
|------|------|
| 新的多 lens 审查 | 复制 `deep-review-template.js`，改 `LENSES` |
| Claude 宿主配置审查 | `claude-chain-deep-review.js` |
| 框架 hook 路由 | `hook-route-deep-audit.js` |

## 约定

- 新脚本须符合 [workflow-script-conventions.md](./workflow-script-conventions.md) 四阶段（审查类）。
- 保存路径：项目 `.claude/workflows/`（团队共享）或 `~/.claude/workflows/`（个人）。
