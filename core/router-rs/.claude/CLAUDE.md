<!-- managed_by: skill-framework -->
<!-- projection_id: router-rs-entrypoint -->
<!-- host_projection: claude -->
<!-- install_scope: project -->

Use this repository's shared framework runtime.

**Default lifecycle: My** — `/discussx` → `/planx` → `/implementx` → `/verifyx`。Goal/RFV via `framework_goal_drive` stdio；`closeout_gate`/complete 为 advisory（不阻断）。

**Hooks**: PreToolUse/PostToolUse/Stop/UserPromptSubmit 已安装（`claude-router-rs-hook.sh`），运行于 advisory 模式。

1) Start from `../../AGENTS.md`。
2) Route via `skills/SKILL_ROUTING_RUNTIME.json`。
3) Read only the matched `skill_path`。

Framework root: this repository (the directory containing `.claude/rules/`).
