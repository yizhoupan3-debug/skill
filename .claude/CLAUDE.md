<!-- managed_by: skill-framework -->
         <!-- projection_id: claude-desktop-self-discipline -->
         <!-- host_projection: claude-desktop -->
         <!-- install_scope: project -->

         # Claude Desktop 自律指引

         MCP 服务器 `router-rs-framework` 提供技能路由、连续性证据、goal/RFV 管理、closeout 检查和框架运行时快照。

         ## 可用工具

         - `framework_digest` — 会话连续性摘要（会话开头调用一次）
         - `framework_snapshot` — 完整框架运行时快照（含连续性视图、task state、evidence、goal state）
         - `skill_route` — 自然语言路由到对应 skill
         - `record_evidence` — 执行验证命令后记录证据
         - `session_checkpoint` — 写入 SESSION_SUMMARY 检查点
         - `goal_state_manage` — 管理 Goal：start / checkpoint / pause / resume / complete / clear
         - `goal_state_read` — 读取当前 Goal 状态
         - `rfv_loop_status` — 查看 RFV 循环状态
         - `rfv_loop_manage` — 管理 RFV 循环：start / append_round
         - `closeout_gate` — 收尾前自检
         - `closeout_record_write` — 写入 closeout record 并验证

         ## 推荐工作流

         1. 开始会话：调用 `framework_digest` 获得连续性摘要
         2. 路由：用 `skill_route` 找到对应 skill 路径
         3. 开始任务：用 `goal_state_manage operation=start` 创建 Goal
         4. 执行与验证：每步验证后调用 `record_evidence`
         5. 检查点：阶段性调用 `goal_state_manage operation=checkpoint`
         6. 多轮调研：用 `rfv_loop_manage` 管理 RFV 循环
         7. 收尾：调用 `closeout_gate` 自检，然后 `goal_state_manage operation=complete`

         ## MCP 协议限制

         Desktop 使用 MCP stdio 协议，与 Claude Code CLI 的 shell hook 有以下差异：

         ### 无法实现的特性

         - **无工具拦截**：MCP 不支持 PreToolUse，无法在执行前拦截危险命令
         - **无硬门控**：Stop/UserPromptSubmit hard block 降级为 advisory（建议性提示）
         - **无 shell hook**：无法使用 router-rs-hook.sh 等 shell 拦截机制

         ### 自律要求

         - 请在执行 `Bash` 工具前评估命令安全性
         - 收尾前务必调用 `closeout_gate` 自检
         - 定期调用 `session_checkpoint` 保存进度
         - 使用 `record_evidence` 记录验证结果

         ## 与 CLI 的区别

         | 特性 | Claude Code CLI | Claude Desktop |
         |------|-----------------|----------------|
         | 工具拦截 | PreToolUse hook | 不可用 |
         | 硬门控 | Stop/UserPromptSubmit | advisory |
         | 证据追加 | PostToolUse 自动 | 需手动调用 |
         | 检查点 | Stop 自动写入 | 需手动调用 |

         ## 共享数据

         `artifacts/current/` 与 CLI 共用，切换宿主时 continuity 无缝延续。

         ## 路由

         Start from AGENTS.md, route via `skills/SKILL_ROUTING_RUNTIME.json`.
