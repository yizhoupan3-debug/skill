<!-- managed_by: skill-framework -->
         <!-- projection_id: claude-desktop-self-discipline -->
         <!-- host_projection: claude-desktop -->
         <!-- install_scope: project -->

         # Claude Desktop 自律指引

         MCP 服务器 `router-rs-framework` 提供技能路由、连续性证据、goal/RFV 管理、closeout 检查和框架运行时快照。

         ## ⚠️ 与 CLI 的关键区别

         > **Desktop 使用 MCP 协议，无工具拦截能力**
         >
         > - 无法自动拦截危险命令（依赖自律）
         > - **证据追踪需手动调用** `record_evidence`（CLI 会自动追加）
         > - **检查点写入需手动调用** `session_checkpoint`（CLI Stop hook 自动写入）
         > - 所有门控为 **advisory**（建议性），非硬阻止

         ## 可用工具

         - `framework_digest` — 会话连续性摘要（会话开头调用一次）
         - `framework_snapshot` — 完整框架运行时快照（含连续性视图、task state、evidence、goal state）
         - `skill_route` — 自然语言路由到对应 skill
         - `record_evidence` — **执行验证命令后必须调用**（CLI 自动追加，Desktop 需手动）
         - `session_checkpoint` — **阶段性工作时必须调用**（CLI Stop 自动写入，Desktop 需手动）
         - `goal_state_manage` — 管理 Goal：start / checkpoint / pause / resume / complete / clear
         - `goal_state_read` — 读取当前 Goal 状态
         - `rfv_loop_status` — 查看 RFV 循环状态
         - `rfv_loop_manage` — 管理 RFV 循环：start / append_round
         - `closeout_gate` — 收尾前自检

         ## 推荐工作流

         1. **开始会话**：调用 `framework_digest` 获得连续性摘要
         2. **路由**：用 `skill_route` 找到对应 skill 路径
         3. **开始任务**：用 `goal_state_manage operation=start` 创建 Goal
         4. **执行与验证**：每步验证后调用 `record_evidence`（⚠️ CLI 自动追加，Desktop 需手动）
         5. **检查点**：阶段性调用 `goal_state_manage operation=checkpoint` 和 `session_checkpoint`（⚠️ CLI Stop 自动写入，Desktop 需手动）
         6. **多轮调研**：用 `rfv_loop_manage` 管理 RFV 循环
         7. **收尾**：调用 `closeout_gate` 自检，然后 `goal_state_manage operation=complete`

         ## 共享数据

         `artifacts/current/` 与 CLI 共用，切换宿主时 continuity 无缝延续。

         ## 路由

         Start from AGENTS.md, route via `skills/SKILL_ROUTING_RUNTIME.json`.

         ## 已知限制

         - **模型路由**: Claude Desktop 通过 cc-switch 代理时，需要在 cc-switch sqlite 数据库中配置模型路由。
           如遇 "Claude Desktop 模型路由未配置" 错误，通常是因为 `claudeDesktopModelRoutes` 中缺少完整版本号模型 ID（如 `claude-haiku-4-5-20251001`）。
           排查方法：
           ```bash
           # 1. 查看哪些模型 ID 触发了路由错误
           sqlite3 ~/.cc-switch/cc-switch.db "SELECT request_model, COUNT(*) as cnt FROM proxy_request_logs WHERE status_code=500 AND error_message LIKE '%模型路由未配置%' GROUP BY request_model ORDER BY cnt DESC;"
           
           # 2. 查看当前 provider 已有路由
           sqlite3 ~/.cc-switch/cc-switch.db "SELECT name, json_extract(meta, '$.claudeDesktopModelRoutes') as routes FROM providers WHERE app_type='claude-desktop' AND is_current=1;"
           
           # 3. 修复：添加缺失的版本号路由
           sqlite3 ~/.cc-switch/cc-switch.db "UPDATE providers SET meta = json_set(meta, '$.claudeDesktopModelRoutes.<MISSING_MODEL_ID>', json_extract(meta, '$.claudeDesktopModelRoutes.<EXISTING_SHORT_NAME>')) WHERE id='<PROVIDER_ID>' AND app_type='claude-desktop';"
           ```
           ⚠️ cc-switch 使用字符串精确匹配，Claude Desktop 发完整版本号（如 `claude-haiku-4-5-20251001`）时必须一一映射。
         - **Session Supervisor**: Desktop 不支持外部 tmux session 管理，长任务依赖会话内 continuity artifacts。
