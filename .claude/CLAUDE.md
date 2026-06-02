<!-- managed_by: skill-framework · claude-desktop · keep ≤48 lines -->
         <!-- projection_id: claude-desktop-self-discipline -->
         <!-- host_projection: claude-desktop -->
         <!-- install_scope: project -->

         # Claude Desktop

         MCP **`router-rs-framework`**（框架路由/goal/closeout）；**`browser-mcp`** 外网调研。详 **`docs/hosts/claude-desktop.md`**；跨宿主 **`AGENTS.md`**。

         ## 语言（硬约束）

         - **面向用户的回复必须使用简体中文**（代码/路径/命令/第三方原文除外）；自然学术中文，避免翻译腔。
         - 仅当用户**当轮明确要求英文**时可切换。
         - **子代理 / Task**：spawn 时在 prompt **首行**写「面向用户的可见输出使用简体中文」；对用户可见层避免中英混排。

         **默认生命周期：My** — `/discussx` → `/planx` → `/implementx` → `/verifyx`。Goal 经 MCP stdio（`goal_state_manage`）；`closeout_gate` / complete 在 MCP 工具层为 advisory（不阻断）；无 PreToolUse/Stop shell hook。

         ## 会话（按序）

         1. `framework_snapshot` — 开头一次
         2. `skill_route`（**router-rs-framework**）→ 只读 `skill_path`
         3. `goal_state_manage operation=start`（宏任务）
         4. 验证后 `record_evidence`
         5. `closeout_gate` → `goal_state_manage operation=complete`
         - 默认 **`lifecycle_profile: my-light`**：closeout/complete 为 advisory（MCP 工具层不阻断）

         ## 无 CLI hook 硬拦

         - 无 PreToolUse/Stop shell 硬拦；Bash 前自行评估安全；勿声称已被 hook 拦截
         - 检查点：`session_checkpoint`（非自动）

         ## 联网（按标签页 — 硬约束）

         | 标签 | MCP | 外网顺序 | 勿用 |
         |------|-----|----------|------|
         | **Chat** | `router-rs-framework` + `browser-mcp` | `web_fetch` → `browser-mcp` → 宿主 WebFetch | Bash `curl`（CCD 沙箱） |
         | **Cowork** | **`browser-mcp`**（Connectors 注入 VM） | **`browser-mcp`**（`browser_open` / `browser_get_text`） | `mcp__workspace__web_fetch`（易 reset）、`WebSearch`（gateway 常失败）、Bash 绕过 |

         Cowork 3P 另须 configLibrary 的 coworkEgressAllowedHosts（个人开发可全开）。运维：**`docs/hosts/claude-desktop-networking.md`**。

         路由：`skills/SKILL_ROUTING_RUNTIME.json` · 产物：`artifacts/current/`（与 Claude Code 共用）。
