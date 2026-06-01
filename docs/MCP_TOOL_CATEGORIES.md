---
last_verified: "2026-06-02"
depends_on: []
---

# MCP 工具分类参考

> 自动生成于 2026-06-02，基于当前会话实际暴露的 MCP 工具。

## 配置来源

| 位置 | 内容 |
|------|------|
| `.mcp.json` | `paperplain`（npx paperplain-mcp） |
| `.claude/settings.json` | hooks（PreToolUse / PostToolUse / Stop / UserPromptSubmit）+ 权限白名单 + 沙箱策略 |
| `.claude/settings.local.json` | 本地 cargo 权限扩展 |

> `router-rs-framework`、`browser-mcp`、`Claude_in_Chrome`、`Claude_Preview`、`ccd_*`、`scheduled-tasks` 等 server 由宿主侧注入（用户级 MCP 配置），不在项目 `.mcp.json` 中。

---

## 工具分类表

### browser-core — 浏览器核心操作

| 工具 | Server | 说明 |
|------|--------|------|
| `browser_open` | browser-mcp | 打开 URL，返回 tab |
| `browser_close` | browser-mcp | 关闭 tab 或会话 |
| `browser_click` | browser-mcp | 点击元素 |
| `browser_fill` | browser-mcp | 填充输入框并可选提交 |
| `browser_press` | browser-mcp | 按键 |
| `browser_tabs` | browser-mcp | 列出/切换 tab |
| `browser_get_state` | browser-mcp | 页面摘要 + 交互元素 + diff |
| `browser_get_text` | browser-mcp | 可见文本提取 |
| `browser_get_elements` | browser-mcp | 按角色/文本过滤交互元素 |
| `browser_screenshot` | browser-mcp | 截图（内联图片） |
| `browser_wait_for` | browser-mcp | 等待页面条件 |
| `browser_get_network` | browser-mcp | 近期网络请求 |
| `browser_save_session` | browser-mcp | 持久化浏览器上下文 |
| `browser_restore_session` | browser-mcp | 恢复浏览器上下文 |
| `browser_diagnostics` | browser-mcp | 运行时健康信息 |
| `browser_batch` | Claude_in_Chrome | 批量顺序执行浏览器操作 |
| `computer` | Claude_in_Chrome | 鼠标键盘交互 + 截图 |
| `navigate` | Claude_in_Chrome | 导航 URL / 前进后退 |
| `read_page` | Claude_in_Chrome | 无障碍树快照 |
| `find` | Claude_in_Chrome | 自然语言查找元素 |
| `form_input` | Claude_in_Chrome | 表单赋值 |
| `get_page_text` | Claude_in_Chrome | 提取页面纯文本 |
| `upload_image` | Claude_in_Chrome | 上传截图/图片 |
| `file_upload` | Claude_in_Chrome | 上传文件到表单 |
| `read_console_messages` | Claude_in_Chrome | 浏览器 console 日志 |
| `read_network_requests` | Claude_in_Chrome | 网络请求监控 |
| `resize_window` | Claude_in_Chrome | 调整窗口尺寸 |
| `gif_creator` | Claude_in_Chrome | GIF 录制与导出 |
| `javascript_tool` | Claude_in_Chrome | 页面内执行 JS |

### session-mgmt — 会话 / Worker 管理

| 工具 | Server | 说明 |
|------|--------|------|
| `session_launch` | browser-mcp | 启动长时间运行 worker |
| `session_terminate` | browser-mcp | 终止 worker |
| `session_list` | browser-mcp | 列出 worker 及状态 |
| `session_inspect` | browser-mcp | 检查单个 worker |
| `session_mark_blocked` | browser-mcp | 标记 worker 阻塞 + 退避 |
| `session_resume_due` | browser-mcp | 恢复到期阻塞 worker |
| `session_classify_block` | browser-mcp | 分类限速/封锁信号 |
| `tabs_context_mcp` | Claude_in_Chrome | MCP tab 组上下文 |
| `tabs_create_mcp` | Claude_in_Chrome | 创建新 tab |
| `tabs_close_mcp` | Claude_in_Chrome | 关闭 tab |
| `select_browser` | Claude_in_Chrome | 按 deviceId 选择浏览器 |
| `switch_browser` | Claude_in_Chrome | 跨浏览器连接请求 |
| `list_connected_browsers` | Claude_in_Chrome | 列出已连接浏览器 |

### background — 后台任务

| 工具 | Server | 说明 |
|------|--------|------|
| `background_list` | browser-mcp | 后台任务快照 |
| `background_inspect` | browser-mcp | 检查单个后台任务 |
| `background_terminate` | browser-mcp | 中断后台任务 |

### diagnostics — 诊断 / 快捷方式

| 工具 | Server | 说明 |
|------|--------|------|
| `browser_diagnostics` | browser-mcp | 运行时健康检查 |
| `skill_route_status` | browser-mcp | 解释 skill 路由工具未暴露原因 |
| `browser_get_attached_runtime_events` | browser-mcp | 重放运行时事件 |
| `runtime_heartbeat` | browser-mcp | 空闲心跳 |
| `shortcuts_list` | Claude_in_Chrome | 列出可用快捷方式/工作流 |
| `shortcuts_execute` | Claude_in_Chrome | 执行快捷方式/工作流 |

### preview — 开发预览

| 工具 | Server | 说明 |
|------|--------|------|
| `preview_start` | Claude_Preview | 启动 dev server |
| `preview_stop` | Claude_Preview | 停止 dev server |
| `preview_list` | Claude_Preview | 列出运行中 server |
| `preview_screenshot` | Claude_Preview | 页面截图 |
| `preview_snapshot` | Claude_Preview | 无障碍树快照 |
| `preview_click` | Claude_Preview | CSS 选择器点击 |
| `preview_fill` | Claude_Preview | 表单填充 |
| `preview_inspect` | Claude_Preview | DOM 元素样式检查 |
| `preview_eval` | Claude_Preview | 调试用 JS 执行 |
| `preview_console_logs` | Claude_Preview | console 输出 |
| `preview_network` | Claude_Preview | 网络请求列表/响应体 |
| `preview_logs` | Claude_Preview | server stdout/stderr |
| `preview_resize` | Claude_Preview | 视口尺寸调整 |

### framework — 框架路由 / Goal / Closeout

| 工具 | Server | 说明 |
|------|--------|------|
| `framework_snapshot` | router-rs-framework | 框架运行时快照 |
| `skill_route` | router-rs-framework | 自然语言查询 → skill 路由 |
| `goal_state_manage` | router-rs-framework | Goal 生命周期管理 |
| `goal_state_read` | router-rs-framework | 读取 GOAL_STATE.json |
| `closeout_gate` | router-rs-framework | Closeout 门控检查 |
| `closeout_record_write` | router-rs-framework | 写入 closeout record |
| `record_evidence` | router-rs-framework | 追加 evidence 记录 |
| `session_checkpoint` | router-rs-framework | 写入 SESSION_SUMMARY checkpoint |
| `rfv_loop_manage` | router-rs-framework | RFV 循环管理 |
| `rfv_loop_status` | router-rs-framework | RFV 循环状态查询 |
| `web_fetch` | router-rs-framework | 只读 HTTP GET（绕过沙箱） |

### research — 学术检索

| 工具 | Server | 说明 |
|------|--------|------|
| `search_research` | paperplain | 跨 PubMed / ArXiv / S2 检索 |
| `fetch_paper` | paperplain | 按 ID/DOI 获取论文元数据 |
| `find_paper_by_title` | paperplain | 按标题查找论文 |

### scheduling — 定时任务

| 工具 | Server | 说明 |
|------|--------|------|
| `create_scheduled_task` | scheduled-tasks | 创建定时/一次性任务 |
| `update_scheduled_task` | scheduled-tasks | 更新已有定时任务 |
| `list_scheduled_tasks` | scheduled-tasks | 列出所有定时任务 |

### file-ops — 文件 / 目录 / CCD 会话

| 工具 | Server | 说明 |
|------|--------|------|
| `request_directory` | ccd_directory | 请求目录访问权限 |
| `mark_chapter` | ccd_session | 标记会话新章节 |
| `spawn_task` | ccd_session | 分离后台任务 |
| `list_sessions` | ccd_session_mgmt | 列出 CCD 会话 |
| `search_session_transcripts` | ccd_session_mgmt | 跨会话全文搜索 |
| `archive_session` | ccd_session_mgmt | 归档 CCD 会话 |

---

## 分类统计

| 分类 | 工具数 | 主要 Server |
|------|--------|-------------|
| browser-core | 28 | browser-mcp, Claude_in_Chrome |
| session-mgmt | 13 | browser-mcp, Claude_in_Chrome |
| background | 3 | browser-mcp |
| diagnostics | 6 | browser-mcp, Claude_in_Chrome |
| preview | 13 | Claude_Preview |
| framework | 11 | router-rs-framework |
| research | 3 | paperplain |
| scheduling | 3 | scheduled-tasks |
| file-ops | 6 | ccd_directory, ccd_session, ccd_session_mgmt |
| **合计** | **86** | **8 server** |
