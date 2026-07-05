# Legacy Skill Path Mapping

以下为科研 skill 重构（2026-07-01）后的路径映射：

| 旧路径（已迁移） | 新路径 | 状态 |
|------------------|--------|------|
| `skills/research-discovery/SKILL.md` | `skills/research/lanes/discovery.md` | 发现 lane 文档（不再独立路由） |
| `skills/research-execution/SKILL.md` | `skills/research/lanes/execution.md` | 执行 lane 文档（不再独立路由） |
| `skills/paper-workbench/SKILL.md` | `skills/research/paper-workbench/SKILL.md` | 物理迁移，保留 contract/flags（slug 不变，仍可直接路由） |
| `skills/autoresearch/SKILL.md` | `skills/research/research-harness/SKILL.md` | 物理迁移，路由 slug 重命名为 research-workspace |
| `skills/research/research-harness/SKILL.md` | `skills/research-harness/SKILL.md` | 2026-07-05 | 路由 slug 保留 `research-workspace`，layer L3→L2，从 `$research` 父 lane 提升为独立一等技能 |

**路由别名**：
- `$research-discovery` → 由 `$research`（discovery lane）替代（旧 slug 已 disabled）
- `$research-execution` → 由 `$research`（execution lane）替代（旧 slug 已 disabled）
- `$paper-workbench` → 仍可直接路由（slug 不变，路径更新），也通过 `$research`（paper-workbench lane）到达
- `$paper-writing`、`$paper-reviewer` → 纸笔别名，解析到 `$research`（paper-workbench lane）
- `$autoresearch` → 由 `$research-workspace`（独立 L2 一等技能）替代，也可通过 `$research` 到达

**L2 统一前门**：`skills/research/SKILL.md`（slug: research）
