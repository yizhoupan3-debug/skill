---
last_verified: "2026-06-02"
depends_on: []
---

# MCP 工具分类标签文档

> 基于当前会话暴露的全部 MCP 工具，按功能域分类并标注风险等级。MCP 协议跨宿主可用，但具体可用工具集因宿主而异（闭集宿主 **codex / claude-code / antigravity / cursor / opencode** 各有 MCP 或 hook 投影面），参见 [`docs/hosts/`](hosts/) 各手册。

## MCP Server 来源

| Server | 注入方式 | 职责 |
|--------|----------|------|
| `browser-mcp` | 宿主侧（用户级 MCP） | 浏览器自动化、Session Worker、后台任务 |
| `Claude_in_Chrome` | 宿主侧（Chrome Extension） | Chrome 原生自动化 |
| `Claude_Preview` | 宿主侧（Preview Server） | Dev server 预览与调试 |
| `router-rs-framework` | 宿主侧（用户级 MCP） | 框架路由 / Goal / Closeout |
| `paperplain` | 项目 `.mcp.json` + cursor / antigravity / opencode 宿主 MCP（`host-integration install` 合并） | 学术论文元数据检索（`npx -y paperplain-mcp`） |
| `scheduled-tasks` | 宿主侧（用户级 MCP） | 定时任务管理 |
| `ccd_directory` | 宿主侧（CCD） | 目录访问授权 |
| `ccd_session` | 宿主侧（CCD） | 会话章节 / 任务分离 |
| `ccd_session_mgmt` | 宿主侧（CCD） | 跨会话管理 |

---

## 风险等级定义

| 等级 | 含义 | 典型场景 |
|------|------|----------|
| **低** | 只读，不改变外部状态 | 读取页面快照、列出任务、截图 |
| **中** | 受控交互，可回退 | 点击按钮、填充表单、导航页面 |
| **高** | 不可逆操作或代码执行 | 终止会话、执行 JS、写入记录、归档 |

---

## 工具分类总览

| 分类 | 工具数 | 主要 Server |
|------|--------|-------------|
| browser-core | 15 | browser-mcp |
| chrome-automation | 21 | Claude_in_Chrome |
| session-mgmt | 6 | browser-mcp |
| background | 3 | browser-mcp |
| diagnostics | 3 | browser-mcp |
| framework | 11 | router-rs-framework |
| preview | 13 | Claude_Preview |
| file-system | 1 | ccd_directory |
| session-other | 5 | ccd_session, ccd_session_mgmt |
| **合计** | **78** | **9 server** |

---

## browser-core — 浏览器核心操作

browser-mcp 提供的 Playwright 级浏览器控制。

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `browser_open` | browser-core | 打开 URL，返回活跃 tab | 中 |
| `browser_close` | browser-core | 关闭单个 tab 或整个会话 | 高 |
| `browser_click` | browser-core | 点击已索引的交互元素 | 中 |
| `browser_fill` | browser-core | 填充输入框，可选自动提交 | 中 |
| `browser_press` | browser-core | 在当前页面按键盘按键 | 低 |
| `browser_tabs` | browser-core | 列出或切换当前 tab | 低 |
| `browser_get_state` | browser-core | 返回页面摘要、交互元素及可选 diff | 低 |
| `browser_get_text` | browser-core | 提取页面或指定元素的可见文本 | 低 |
| `browser_get_elements` | browser-core | 按角色和文本查询过滤交互元素 | 低 |
| `browser_get_network` | browser-core | 返回近期网络请求（状态/时间/body） | 低 |
| `browser_screenshot` | browser-core | 截取页面截图（可全页） | 低 |
| `browser_wait_for` | browser-core | 等待页面条件（文本出现/消失、URL 匹配等） | 低 |
| `browser_save_session` | browser-core | 持久化浏览器上下文到磁盘 | 中 |
| `browser_restore_session` | browser-core | 从磁盘恢复浏览器上下文 | 中 |
| `runtime_heartbeat` | browser-core | 空闲时发出心跳（无新事件时） | 低 |

---

## chrome-automation — Chrome 原生自动化

Claude in Chrome 扩展提供的桌面级浏览器操作，包括鼠标键盘控制和 JS 执行。

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `computer` | chrome-automation | 鼠标键盘全操作（点击/拖拽/滚动/截图/缩放） | 高 |
| `navigate` | chrome-automation | 导航 URL 或前进/后退历史 | 中 |
| `find` | chrome-automation | 自然语言查找页面元素，返回引用 ID | 低 |
| `read_page` | chrome-automation | 无障碍树快照（可过滤交互元素） | 低 |
| `get_page_text` | chrome-automation | 提取页面纯文本（文章优先） | 低 |
| `read_console_messages` | chrome-automation | 读取浏览器 console 日志（支持正则过滤） | 低 |
| `read_network_requests` | chrome-automation | 读取网络请求（支持 URL 模式过滤） | 低 |
| `form_input` | chrome-automation | 通过引用 ID 设置表单元素值 | 中 |
| `file_upload` | chrome-automation | 上传文件到文件输入元素（限 10MB） | 中 |
| `upload_image` | chrome-automation | 上传截图或图片到表单/拖拽目标 | 中 |
| `select_browser` | chrome-automation | 按 deviceId 静默选择目标浏览器 | 低 |
| `list_connected_browsers` | chrome-automation | 列出所有已连接的 Chrome 浏览器实例 | 低 |
| `switch_browser` | chrome-automation | 广播连接请求，等待用户选择浏览器 | 低 |
| `tabs_context_mcp` | chrome-automation | 获取当前 MCP tab 组上下文（含所有 tab ID） | 低 |
| `tabs_create_mcp` | chrome-automation | 在 MCP tab 组中创建新空 tab | 低 |
| `tabs_close_mcp` | chrome-automation | 关闭 MCP tab 组中的指定 tab | 中 |
| `shortcuts_list` | chrome-automation | 列出可用快捷方式和工作流 | 低 |
| `shortcuts_execute` | chrome-automation | 在侧边窗口执行快捷方式/工作流 | 中 |
| `gif_creator` | chrome-automation | GIF 录制、导出（含点击指示器/标签/进度条） | 中 |
| `resize_window` | chrome-automation | 调整浏览器窗口尺寸 | 低 |
| `javascript_tool` | chrome-automation | 在页面上下文执行任意 JavaScript | 高 |

---

## session-mgmt — Session Worker 管理

browser-mcp 的长时间运行 Worker 生命周期管理。

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `session_launch` | session-mgmt | 启动长时间运行 worker（纯 Rust 进程 + resume） | 高 |
| `session_list` | session-mgmt | 列出所有 worker 及刷新其运行时状态 | 低 |
| `session_inspect` | session-mgmt | 检查单个 worker 的详细状态 | 低 |
| `session_terminate` | session-mgmt | 终止指定 worker | 高 |
| `session_mark_blocked` | session-mgmt | 标记 worker 阻塞并设置退避秒数 | 中 |
| `session_resume_due` | session-mgmt | 按退避策略恢复所有到期的阻塞 worker | 中 |

---

## background — 后台任务管理

browser-mcp Rust 持久层中的后台任务快照与终止。

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `background_list` | background | 返回后台任务快照（持久层） | 低 |
| `background_inspect` | background | 按 jobId 检查单个后台任务详情 | 低 |
| `background_terminate` | background | 标记后台任务为中断并记录错误 | 高 |

---

## diagnostics — 诊断与运行时状态

健康检查、事件重放、心跳。

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `browser_diagnostics` | diagnostics | 返回运行时健康信息（含 skill 路由暴露状态） | 低 |
| `skill_route_status` | diagnostics | 解释为何 skill 路由工具未暴露 | 低 |
| `session_classify_block` | diagnostics | 分类限速/封锁信号（基于证据文本） | 低 |

---

## framework — 框架路由 / Goal / Closeout

router-rs-framework 提供的框架生命周期管理工具。

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `framework_snapshot` | framework | 框架运行时快照（含连续性视图） | 低 |
| `skill_route` | framework | 自然语言查询，返回匹配的 skill 路由结果 | 低 |
| `goal_state_manage` | framework | Goal 生命周期管理（start/checkpoint/pause/resume/complete/clear/block） | 中 |
| `goal_state_read` | framework | 读取当前 task 的 GOAL_STATE.json | 低 |
| `closeout_gate` | framework | 返回 closeout 状态与缺失项清单（advisory） | 低 |
| `closeout_record_write` | framework | 写入并验证 closeout record（artifacts/closeout/） | 高 |
| `record_evidence` | framework | 追加 evidence 记录到 EVIDENCE_INDEX | 中 |
| `session_checkpoint` | framework | 写入 SESSION_SUMMARY 和 NEXT_ACTIONS checkpoint | 中 |
| `rfv_loop_manage` | framework | RFV 循环管理（start / append_round） | 中 |
| `rfv_loop_status` | framework | 查询 RFV 循环当前状态 | 低 |
| `web_fetch` | framework | 只读 HTTP GET（绕过 CCD 沙箱，有字节上限） | 低 |

---

## preview — 开发预览

Claude_Preview 提供的 dev server 管理和页面调试工具。

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `preview_start` | preview | 按 launch.json 配置启动 dev server（复用已有） | 中 |
| `preview_stop` | preview | 停止指定 dev server | 中 |
| `preview_list` | preview | 列出已启动的 preview server 及其 ID | 低 |
| `preview_screenshot` | preview | 页面截图（JPEG 压缩，用于布局检查） | 低 |
| `preview_snapshot` | preview | 无障碍树快照（首选：验证文本/元素/结构） | 低 |
| `preview_click` | preview | 通过 CSS 选择器点击元素 | 中 |
| `preview_fill` | preview | 填充 input/textarea/select 元素 | 中 |
| `preview_inspect` | preview | DOM 元素样式/尺寸检查（精确验证颜色/字体/间距） | 低 |
| `preview_eval` | preview | 调试用 JS 执行（仅读取/检查，不可持久化 DOM） | 高 |
| `preview_console_logs` | preview | 浏览器 console 输出（支持 level 过滤） | 低 |
| `preview_network` | preview | 网络请求列表或按 requestId 检查响应体 | 低 |
| `preview_logs` | preview | dev server stdout/stderr（支持 level/search 过滤） | 低 |
| `preview_resize` | preview | 视口尺寸调整（含预设：mobile/tablet/desktop 和暗色模式） | 低 |

---

## file-system — 文件系统访问

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `request_directory` | file-system | 请求授予工作目录外的目录访问权限 | 中 |

---

## session-other — 会话管理 / 论文检索 / 定时任务

CCD 会话辅助、学术检索和定时任务管理。

| 工具名 | 分类 | 简要说明 | 风险等级 |
|--------|------|----------|----------|
| `mark_chapter` | session-other | 标记会话新章节（出现在转录和目录中） | 低 |
| `spawn_task` | session-other | 标记旁路问题，在独立 worktree 中分叉处理 | 低 |
| `list_sessions` | session-other | 列出用户的其他 CCD 会话（活跃/可选归档） | 低 |
| `search_session_transcripts` | session-other | 跨会话全文搜索（子串匹配，大小写不敏感） | 低 |
| `archive_session` | session-other | 归档 CCD 会话（停止进程并清理 worktree） | 高 |

---

## 附：research / scheduling（项目或宿主级 MCP）

以下工具由 `.mcp.json`（paperplain）或宿主级 scheduled-tasks server 提供，不在上述九大分类中，但为完整起见列出。

| 工具名 | 分类 | Server | 简要说明 | 风险等级 |
|--------|------|--------|----------|----------|
| `search_research` | research | paperplain | 跨 PubMed / ArXiv / S2 学术检索 | 低 |
| `fetch_paper` | research | paperplain | 按 ID/DOI 获取论文元数据 | 低 |
| `find_paper_by_title` | research | paperplain | 按标题查找论文（Semantic Scholar） | 低 |
| `create_scheduled_task` | scheduling | scheduled-tasks | 创建定时/一次性任务（需用户审批） | 高 |
| `update_scheduled_task` | scheduling | scheduled-tasks | 更新已有定时任务的 prompt/schedule/状态 | 中 |
| `list_scheduled_tasks` | scheduling | scheduled-tasks | 列出所有定时任务及其状态 | 低 |

> 含附录共 **84** 个工具（主分类 78 + research/scheduling 6）。

---

## 风险等级汇总

| 风险等级 | 工具数 | 代表性工具 |
|----------|--------|-----------|
| **低** | 34 | `browser_get_state`, `read_page`, `preview_snapshot`, `goal_state_read` |
| **中** | 25 | `browser_click`, `navigate`, `session_resume_due`, `goal_state_manage` |
| **高** | 9 | `computer`, `javascript_tool`, `session_terminate`, `background_terminate`, `closeout_record_write`, `preview_eval`, `browser_close`, `session_launch`, `archive_session` |

---

## 配置来源

| 文件 | 内容 | 适用宿主 |
|------|------|----------|
| `.mcp.json` | `router-rs-framework`、`browser-mcp`、`paperplain`、`mcp-codegraph` — 项目级 MCP server（gitignored；`host-integration install` 合并） | **Claude Code**（`claude-code`）；Codex 通过 `.codex/config.toml` |
| `~/.codex/config.toml` + `.codex/hooks.json` | `[features] hooks = true`；`[mcp_servers.*]` 托管 `router-rs-framework`、`browser-mcp`、`mcp-codegraph`、`paperplain`；hook 绑定 PreToolUse / UserPromptSubmit / PostToolUse / Stop | **Codex**（`codex`） |
| `.claude/settings.json` | hooks（PreToolUse / PostToolUse / Stop / UserPromptSubmit）+ Bash/WebFetch 权限白名单 + 沙箱策略 | **Claude Code**（`claude-code`） |
| `.claude/settings.local.json` | 本地 cargo 权限扩展 | Claude Code |
| `~/.cursor/mcp.json`（user） | 托管 `router-rs-framework`、`browser-mcp`、`mcp-codegraph`、`paperplain`（见 `framework host-integration install --to cursor --scope user`） | **Cursor**（`cursor`） |
| `.gemini/mcp.json` | `router-rs-framework`、`browser-mcp`、`mcp-codegraph`、`paperplain`（MCP stdio；无 shell hook） | **Antigravity**（`antigravity`） |
| `.opencode/opencode.json` | `mcpServers.router-rs-framework`、`browser-mcp`、`mcp-codegraph`、`paperplain`（MCP stdio；无 shell hook） | **OpenCode**（`opencode`） |
