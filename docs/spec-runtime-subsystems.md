---
parent: docs/spec.md
version: unified-v7
---

## 9. 运行时子系统

### 9.1 browser_mcp/ — 浏览器 MCP 集成

**功能**：基于 CDP 的 MCP 服务器，提供浏览器自动化、页面快照、网络监控和 Skill 路由。

- 核心：`run_browser_mcp_stdio_loop()` — JSON-RPC 2.0 over stdio
- 30+ MCP 工具：browser_open/click/fill/screenshot/get_state/network/tabs 等
- Session 管理：session_launch/inspect/terminate/mark_blocked/resume_due
- 依赖：`runtime-storage::background_state`, `session-supervisor`, `tungstenite`, `reqwest`

### 9.2 background_state/ — 后台任务状态（位于 core/runtime-storage/src/background_state/）

**功能**：持久化后台作业状态存储（filesystem/sqlite/memory 三后端）。v7 从 runtime-core 提取至 `runtime-storage` crate。

- 状态机：`queued → running → completed/failed/interrupted`
- 支持 `retry_scheduled/retry_claimed/retry_exhausted`
- 过期回收：活跃 1h TTL，终态 24h TTL
- 入口：`handle_background_state_operation()`

### 9.3 session_supervisor/ — Worker 生命周期（位于 core/session-supervisor/）

**功能**：Worker 生命周期管理（launch/resume/terminate/mark_blocked/resume_due）。

- 驱动：codex/cursor/claude（`driver.rs`）
- 原生进程驱动（v7 已提取为独立 crate `core/session-supervisor/`；`runtime-core` 通过 re-export facade 兼容）
- 速率限制检测：正则模式匹配
- 入口：`handle_session_supervisor_operation()`

### 9.4 framework_runtime/ — 运行时行为（runtime-core facade 子模块 + core/framework-runtime/）

**功能**：运行时快照、契约摘要、workspace 初始化、doctor 检查、状态行构建。v7 将 runtime 核心模块拆分为两部分：

- **`runtime-core/src/framework_runtime/`**（facade 保留）：stdin dispatch、doctor、session artifacts、alias
- **`core/framework-runtime/`**（提取）：closeout enforcement、execution contract、pre_tool_use_guard、runtime_view、trace I/O、live_execute、sandbox_control

| 子文件 | 位置 | 功能 |
|--------|------|------|
| `runtime_view.rs` | `core/framework-runtime/` | 运行时视图 + 连续性分类 |
| `framework_doctor.rs` | `runtime-core/framework_runtime/` | Doctor 健康检查 + 连续性审计 |
| `session_artifacts.rs` | `runtime-core/framework_runtime/` | 会话 artifact 写入 |
| `statusline.rs` | `runtime-core/framework_runtime/` | 状态行构建 |
| `prompt_compression.rs` | `runtime-core/framework_runtime/` | prompt 压缩策略 |
| `closeout_enforcement.rs` | `core/framework-runtime/` | closeout 记录评估与强制执行 |
| `execution_contract.rs` | `core/framework-runtime/` | 执行契约（前置/后置条件验证） |
| `pre_tool_use_guard.rs` | `core/framework-runtime/` | PreToolUse 守卫 |

- 快照生成：可以通过 `router-rs framework snapshot` 生成包含完整连续性视图的运行时快照只读模型。

### 9.5 framework_maint/ — 维护命令

**功能**：`router-rs framework maint ...` 维护子命令集。

子命令：`RefreshHostProjections` · `VerifyCursorHooks` · `VerifyCodexHooks` · `UpdateOneShot` · `UpdateAudit` · `CleanRustTargets` · `PrintLocalHomes` · `InstallCodexUserHooks` · `ContinuityAudit`

### 9.6 framework_profile/ — Profile 管理

**功能**：框架 Profile 编译、artifact 打包和控制平面契约描述符。

- `FrameworkProfileContract` — profile_id/capabilities/mcp_servers 等 20+ 字段
- `ProfileBundle` + `CapabilityBundle`
- 控制平面：`build_control_plane_contract_descriptors()`

---

## 10. Hook 系统

### 10.1 hook_common/ — 共享 Hook 工具

**功能**：跨宿主 hook 共享逻辑（路径守卫、证据追加、信号检测、lane 归一化）。

- 28 个 pub fn
- 子模块：`path_guard.rs`, `evidence.rs`, `lane_normalize.rs`, `hook_observation_rules.rs`

### 10.2 hook_policy/ — Hook 策略

**功能**：Bash 命令危险分类、MCP 工具安全检测、受保护路径识别。

| 子模块 | 功能 | 核心 API |
|--------|------|----------|
| `bash_guard.rs` | Bash 命令分析（正则模式匹配） | `dangerous_bash_reason()` |
| `mcp_safety.rs` | MCP 工具安全 | `dangerous_mcp_tool_reason()` |
| `evaluate.rs` | 统一策略评估 | `evaluate_hook_policy()` |
| `contract.rs` | 契约 JSON | `hook_policy_contract()` |

### 10.3 review/ — Review 引擎

**功能**：Review gate 执行、异构对抗审稿路由、输出格式 lint。

| 子模块 | 功能 |
|--------|------|
| `engine.rs` | Review gate 核心（Strict/Lite 模式） |
| `heterogeneous.rs` | 异构对抗审稿（ModelFamily 检测 + 跨族验证） |
| `output_lint.rs` | Review 输出格式 lint |
| `routing_signals.rs` | Review 路由信号 |

**ModelFamily**: Claude/Gpt/Gemini/Llama/Mistral/Deepseek

---



### 13.1 runtime_storage/ — 运行时存储

**功能**：filesystem/sqlite/memory 三后端统一抽象。

- 操作：read/write/append/exists/delete/stat
- 路径级文件锁（`acquire_runtime_path_lock`）
- SQLite WAL 模式

### 13.2 runtime_registry/ — 运行时注册表

**功能**：磁盘优先 `RUNTIME_REGISTRY.json` 加载器。

- `HookRegistryRepoGuard` — RAII 守卫
- 缓存：mtime-based OnceLock

### 13.3 stdio_transport.rs — Stdio 传输层

**功能**：并发 JSON-over-stdio 传输层。

- Worker 池：默认 8，最大 32
- 超时：默认 30s，最大 3600s
- in-flight 超时 + 批量响应刷新
- 支持 stdio `execute` operation 处理机制

### 13.4 host_entrypoint_sync.rs — 入口同步

**真源**：`core/host-projection/src/host_entrypoint_sync.rs`（runtime-core 通过 `pub use` 重导出）。

**功能**：通用 sync engine + Codex provider。

- `full_sync`（root）vs `partial_sync`（worktree）
- `HostProjectionAdapter` — 薄 adapter 表

### 13.5 host_integration/ — 安装/投影

- `install_<host>_projection` — 投影写盘
- `remove_<host>_projection` — 投影移除
- `<host>_projection_status` — 投影状态查询
- 三道闸：写盘前 validate + 写盘后 readback + manifest 路径存在性

---

