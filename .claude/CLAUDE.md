<!-- managed_by: skill-framework · claude-code · keep ≤48 lines -->
<!-- projection_id: claude-code-project-narrative -->
<!-- host_projection: claude-code -->
<!-- install_scope: project -->

# Claude Code（本项目）

跨宿主 **`AGENTS.md`**；宿主差异 **`AGENTS_CLAUDE.md`**；手册 **`docs/hosts/claude.md`**。

## 语言（硬约束）

- **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外）；自然学术中文，避免翻译腔。
- 仅当用户**当轮明确要求英文**时可切换。
- **子代理 / Task**：spawn 时在 prompt **首行**写「面向用户的可见输出使用简体中文」。

**Default lifecycle (all supported hosts):** `/discussx` → `/planx` → `/implementx` → `/verifyx`. Pre-execution (`discussx`, `planx`) is doc-only; `implementx` runs **all waves in one breath** per `WAVE_STATE.json`; `verifyx` merges verify + ship. REVIEW_GATE Stop is **advisory-only on all hosts**; `my-light` suppresses review nudge and spawn-first. Legacy `/gsd-*` removed (2026-05); use My commands only.

## Hook 集成（非 MCP）

- 四事件：`PreToolUse`、`UserPromptSubmit`、`PostToolUse`、`Stop`（`.claude/settings.json` + `router-rs claude hook`）。
- Goal/RFV：`framework_goal_drive` / `framework_rfv_loop` stdio + `artifacts/current/<task_id>/`。
- 默认 **`lifecycle_profile: my-light`**：closeout/complete 为 advisory，suppress review Stop nudge；非 my-light 时 closeout 可 fail-closed（与 REVIEW_GATE advisory 分层，见 `docs/host_adapter_contract.md` §0.1）。
- 检查点：`session_checkpoint`（非自动）。

## MCP（可选）

项目 `.claude/mcp.json` 可注册 `browser-mcp` 等；历史 Desktop 配置见 **`mcp.README.md`**（`claude-desktop` 已退役，勿作真源）。

路由：`skills/SKILL_ROUTING_RUNTIME.json` · 产物：`artifacts/current/`。
