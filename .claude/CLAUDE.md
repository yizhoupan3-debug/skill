<!-- managed_by: skill-framework · claude-desktop · keep ≤40 lines -->
         <!-- projection_id: claude-desktop-self-discipline -->
         <!-- host_projection: claude-desktop -->
         <!-- install_scope: project -->

         # Claude Desktop

         MCP **`router-rs-framework`**。协议与限制：**`docs/hosts/claude-desktop.md`**；政策：**`AGENTS.md`**。

         ## 会话（按序）

         1. `framework_digest` — 开头一次
         2. `skill_route` → 只读 `skill_path`
         3. `goal_state_manage operation=start`（宏任务）
         4. 验证后 `record_evidence`
         5. `closeout_gate` → `goal_state_manage operation=complete`

         ## 无 hook 硬拦

         - 无 PreToolUse；Bash 前自行评估安全
         - Stop/UserPromptSubmit 无 CLI 硬 block — 勿声称已被 hook 拦截
         - 检查点：`session_checkpoint`（非自动）

         ## 共享

         `artifacts/current/` 与 Claude Code CLI 共用。路由：`skills/SKILL_ROUTING_RUNTIME.json`。
