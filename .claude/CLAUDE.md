<!-- managed_by: skill-framework · claude-desktop · keep ≤48 lines -->
         <!-- projection_id: claude-desktop-self-discipline -->
         <!-- host_projection: claude-desktop -->
         <!-- install_scope: project -->

         # Claude Desktop

         MCP **`router-rs-framework`**（框架路由/goal/closeout）；**`browser-mcp`** 仅调研。协议：**`docs/hosts/claude-desktop.md`**；政策：**`AGENTS_CLAUDE.md`**。

         **Default lifecycle: My** (same chain). Goal drive via MCP stdio; MCP hard closeout at tool level for non-my-light (`goal_state_manage` / `closeout_gate`); no PreToolUse/Stop shell hook.

         ## 会话（按序）

         1. `framework_snapshot` — 开头一次
         2. `skill_route`（**router-rs-framework**）→ 只读 `skill_path`
         3. `goal_state_manage operation=start`（宏任务）
         4. 验证后 `record_evidence`
         5. `closeout_gate` → `goal_state_manage operation=complete`
         - 默认 **`lifecycle_profile: my-light`**：closeout/complete 为 advisory；非 my-light 时 MCP **硬拦**

         ## 无 CLI hook 硬拦

         - 无 PreToolUse；Bash 前自行评估安全
         - Stop/UserPromptSubmit 无 shell 硬 block — 勿声称已被 hook 拦截
         - 检查点：`session_checkpoint`（非自动）

         ## 联网（按标签页 — 硬约束）

         | 标签 | MCP | 外网顺序 | 勿用 |
         |------|-----|----------|------|
         | **Chat** | `router-rs-framework` + `browser-mcp` | `web_fetch` → `browser-mcp` → 宿主 WebFetch | Bash `curl`（CCD 沙箱） |
         | **Cowork** | **`browser-mcp`**（Connectors 注入 VM） | **`browser-mcp`**（`browser_open` / `browser_get_text`） | `mcp__workspace__web_fetch`（易 reset）、`WebSearch`（gateway 常失败）、Bash 绕过 |

         Cowork 3P 另须 configLibrary 的 coworkEgressAllowedHosts（个人开发可全开）。运维：**`docs/hosts/claude-desktop-networking.md`**。

         路由：`skills/SKILL_ROUTING_RUNTIME.json` · 产物：`artifacts/current/`（与 Claude Code 共用）。
