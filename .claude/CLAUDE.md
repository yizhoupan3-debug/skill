<!-- managed_by: skill-framework · claude · keep ≤48 lines -->
<!-- projection_id: claude-project-narrative -->
<!-- host_projection: claude -->
<!-- install_scope: project -->

# Claude Code（本项目）

跨宿主 **`AGENTS.md`**（宿主差异见该文件内「宿主行为差异」节）。

## 语言（硬约束）

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外）；自然学术中文，避免翻译腔。
- 仅当用户**当轮明确要求英文**时可切换。
- **子代理 / Task**：spawn 时在 prompt **首行**写「面向用户的可见输出使用简体中文」。

**Default lifecycle:** task — Goal/Quality Gate via stdio + manual boards; `router-rs claude hook` does not inject GOAL_CONTINUE/QUALITY_GATE/digest. REVIEW_GATE Stop advisory-only (Claude canonical clearance); `task` suppresses review nudge and spawn-first.

## Hook 集成（非 MCP）

- 四事件：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`（`.claude/settings.json` + `router-rs claude hook`）。
- Goal/Quality Gate：`framework_goal_drive` / `framework_quality_gate` stdio + `artifacts/current/<task_id>/`。
- 默认 **交互式模式**：closeout/complete 为 advisory，suppress review Stop nudge；非交互式时 closeout 可 fail-closed（与 REVIEW_GATE advisory 分层，见 `docs/README.md` §Stop/closeout）。
- 检查点：`session_checkpoint`（非自动）。
- Goal 自动触发：`UserPromptSubmit` 检测复杂任务（自然语言+启发式）→ 注入 goal 建议上下文；`has_structured_goal_contract` 已扩展为在 regex 失败时回退到复杂度分析。
- Goal amend：`goal_state_manage(operation="amend")` 更新 goal 字段，保留 checkpoints；scope change 检测自动触发 `[Goal Amendment]` 上下文注入。
- Goal 完成自动归档：`complete` 操作标记 `archived: true`，不再物理删除 GOAL_STATE.json。
- 严格退出验证：Stop 管线读取磁盘 `done_when` 与响应内容比对，列出未完成项。

## MCP（可选）

项目 `.claude/mcp.json` 可注册 `browser-mcp` 等。

路由：`skills/SKILL_ROUTING_RUNTIME.json` · 产物：`artifacts/current/`。
