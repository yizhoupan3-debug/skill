<!-- managed_by: skill-framework · claude-code · keep ≤48 lines -->
<!-- projection_id: claude-code-project-narrative -->
<!-- host_projection: claude-code -->
<!-- install_scope: project -->

# Claude Code（本项目）

跨宿主 **`AGENTS.md`**；宿主差异 **`AGENTS_CLAUDE.md`**；手册 **`docs/hosts/claude.md`**。
语言/lifecycle/profile 等全局规则见全局 CLAUDE.md。

## Hook 集成（非 MCP · 项目特有）

- 七事件全注册：`SessionStart`、`UserPromptSubmit`、`PreToolUse`、`PostToolUse`、`Stop`、`SubagentStart`、`SubagentStop`（`.claude/settings.json` + `router-rs claude hook`）。
- Goal/RFV：`framework_goal_drive` / `framework_rfv_loop` stdio + `artifacts/current/<task_id>/`。
- 检查点：`session_checkpoint`（非自动）。

## MCP（可选 · 项目特有）

项目 `.claude/mcp.json` 可注册 `browser-mcp` 等（`claude-desktop` 已退役，勿作真源）。

路由：`skills/SKILL_ROUTING_RUNTIME.json` · 产物：`artifacts/current/`。
