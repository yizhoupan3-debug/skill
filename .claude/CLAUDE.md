<!-- managed_by: skill-framework · claude · keep ≤30 lines -->
<!-- projection_id: claude-project-narrative -->
<!-- host_projection: claude -->
<!-- install_scope: project -->

# Claude Code（本项目）

框架入口：**`AGENTS.md`**（策略真源 + 速查卡）。路由：`skill_route(query)`。

## 生命周期

- **Default lifecycle:** task — Goal/Quality Gate via stdio + manual boards; `router-rs claude hook` 不注入 GOAL_CONTINUE/QUALITY_GATE/digest。REVIEW_GATE advisory-only。Closeout advisory（交互式）。
- Goal 自动检测（UserPromptSubmit 复杂度分析）→ 自动创建 Goal 合约。scope change 自动触发 `[Goal Amendment]`。
- Goal complete 自动归档（archived: true）。
- 严格退出验证：Stop 管线比对 `done_when` 与响应内容。

## Hook 集成

- 四事件：PreToolUse/UserPromptSubmit/PostToolUse/Stop（`.claude/settings.json` → `router-rs claude hook`）。
- Review 默认 findings-only。REVIEW_GATE 不硬阻断 Stop。

## 路由

1) Start from `AGENTS.md`。
2) NL queries → `skills/SKILL_ROUTING_RUNTIME.json`。Slash command（`/name`）→ 原生 `~/.claude/skills/`，不经过 MCP。
3) Read only matched `skill_path`。

Framework root: `${FRAMEWORK_ROOT}` · Project root: `${PROJECT_ROOT}`。

## 产物

`artifacts/current/` · `.claude/mcp.json` 可选注册 MCP servers。
