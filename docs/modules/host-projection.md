---
module: host-projection
lines: ~17000
layer: B2
last_verified: "2026-06-19"
---

# host-projection（B2 层）

宿主投影层，负责将框架能力投影到四个闭集宿主（claude、cursor、codex、opencode）。

## 职责

宿主集成（install/status/remove）、hook 实现、MCP harness、入口同步。

## 顶层模块

| 模块 | 行数 | 功能 |
|------|------|------|
| `hooks` | 1,541 | 函数指针代理层（OnceLock slots，解耦 runtime-core 与宿主 hooks） |
| `host_entrypoint_sync` | 541 | 宿主入口同步 |
| `host_integration/` | 6,794 | 宿主集成核心（详见 [#projection-子模块详解](#projection-子模块详解)） |
| `hosts/` | ~15,700+ | 四宿主 hook 实现（详见 [#hosts-子模块详解](#hosts-子模块详解)） |

## hooks.rs 函数指针代理层

`hooks.rs` 通过 `OnceLock` 函数指针 slots 解耦 `runtime-core` 和 `host-projection`：

```
runtime-core (注册) → hooks.rs (OnceLock slots) → host-projection hosts (调用)
```

关键 slots: `resolve_repo_root_arg`、`write_framework_session_artifacts`、`route_task_with_manifest_fallback`、`append_evidence_index`、`check_anomalies`。

## 依赖关系

- **依赖**: `framework-kernel`、`core-policy`、`core-state`、`routing-engine`
- **被依赖**: `runtime-core`（re-export `hosts::*`）

## 近期变更

- v6.5: `projection.rs`（3,668 行）拆分为 `mod.rs` + `projection_bootstrap.rs` + `projection_host_ops.rs` + `projection_manifest.rs`
- v6.5: `mcp_stdio_harness.rs` 拆分为目录 `mcp_stdio_harness/mod.rs` + `tools.rs`
- v6.5: 新增 `hook_dispatch.rs`（统一 hook 分发 trait）和 `file_state_lock.rs`（跨宿主文件锁）
- v6.5: `claude_code_hooks` 测试提取到独立文件

## 已知技术债

- `hooks.rs` 中 `WRITE_FRAMEWORK_SESSION_ARTIFACTS` 拼写已修正（原 ARTARTIFACTS）
- Claude hooks 和 Cursor hooks 未完全使用 `hook_dispatch` 共享抽象
- `opencode_hooks` 的 `append_shell_evidence` 是空实现

## projection 子模块详解

宿主投影的核心逻辑：install/status/remove 操作、bootstrap 生成、manifest 管理。

### 文件结构

| 文件 | 行数 | 功能 |
|------|------|------|
| `mod.rs` | ~1,300 | 核心投影逻辑：`HostProjectionAdapter` 定义、四宿主适配器常量、MCP payload 生成、narrative 渲染 |
| `projection_bootstrap.rs` | 678 | 默认 bootstrap 生成与验证（`build_default_bootstrap_payload`） |
| `projection_host_ops.rs` | 741 | 各宿主 install/status/remove 实现 |
| `projection_manifest.rs` | 780 | 投影 manifest CRUD、entrypoint 渲染、Cursor MCP 服务器管理 |
| `projection_ops_trait.rs` | 60 | `HostProjectionOps` trait + `PROJECTION_OPS_REGISTRY`（v6.5 拆分产物） |

### 核心 trait

```rust
// mod.rs — HostProjectionAdapter 是元数据持有者（非分发表）
pub struct HostProjectionAdapter {
    pub tool: &'static str,
    pub host_id: &'static str,
}

// 操作分发通过 HostProjectionOps trait（projection_ops_trait.rs）实现
pub trait HostProjectionOps: Send + Sync {
    fn install(&self, args: &ProjectionArgs) -> Result<(), String>;
    fn status(&self, args: &ProjectionArgs) -> Result<ProjectionStatus, String>;
    fn remove(&self, args: &ProjectionArgs) -> Result<(), String>;
}
```

四宿主适配器常量: `KNOWN_PROJECTION_TOOLS` 静态数组（`mod.rs`）中包含 cursor、claude、opencode、codex 四项。

### 关键 pub 函数

| 函数 | 文件 | 作用 |
|------|------|------|
| `install_native_integration` | mod.rs | 主安装入口 |
| `projection_install_command` | mod.rs | CLI install 子命令 |
| `projection_status_command` | mod.rs | CLI status 子命令 |
| `canonical_tool_name` | projection_manifest.rs | 工具名标准化（含 alias 匹配） |
| `render_claude_framework_entrypoint` | mod.rs | Claude entrypoint 渲染 |
| `render_codex_framework_entrypoint` | projection_manifest.rs | Codex entrypoint 渲染 |
| `render_cursor_framework_entrypoint` | projection_manifest.rs | Cursor entrypoint 渲染 |
| `install_cursor_mcp_server` | projection_manifest.rs | Cursor MCP 服务器安装 |

### 已知技术债

- `projection_manifest.rs` 命名为 "manifest" 但包含大量 entrypoint 渲染逻辑
- 管理 MCP 键列表 `["router-rs-framework", "browser-mcp", "mcp-codegraph", "paperplain"]` 在 7 处重复
- `skills_runtime_rel_path` 计算逻辑在 3 处重复
- `framework_entrypoint_common_footer` 仅 Claude 使用，Codex/Cursor 内联了相同内容

## hosts 子模块详解

四宿主 hook 实现 + 共享抽象层。

### 共享抽象

| 文件 | 行数 | 功能 |
|------|------|------|
| **`hook_dispatch.rs`** | 514 | 统一 hook 分发 trait：`HostHookConfig` + `HostHookDispatcher` |
| **`file_state_lock.rs`** | 224 | 跨宿主文件状态锁：`FileStateLockGuard` + `HookStateConfig` |
| `host_provider.rs` | 495 | `HostProvider` trait + 注册表 + 路由别名 |
| `hook_state_common.rs` | 3 | hook state 版本适配 |

#### hook_dispatch.rs 架构

```
HostHookDispatcher trait
├── (A) 纯共享方法（默认实现）
│   ├── dispatch() — 统一路由
│   ├── handle_subagent_start/stop() — 默认 no-op
│   └── 共享工具函数（extract_prompt_text、extract_tool_name、compact_contexts 等）
├── (B) 共享+可覆盖
│   ├── handle_stop() — 默认 closeout followup 检查
│   └── handle_session_start() — 默认 operator inject
└── (C) 必须实现
    ├── handle_pre_tool_use()
    ├── handle_user_prompt_submit()
    └── handle_post_tool_use()
```

#### file_state_lock.rs

- `FileStateLockGuard`: RAII 文件锁（Unix: flock, non-Unix: create_new）
- `HookStateConfig`: 配置 host_id/state_dir_leaf/state_filename，提供 `with_state_lock` 泛型方法
- `read_stdin_json`: 带 4 MiB 限制的 stdin JSON 读取

### 四宿主 hook 实现

| 宿主 | 文件 | 行数 | 使用共享抽象 |
|------|------|------|-------------|
| Claude | `claude_code_hooks.rs` | 1,485 | 部分（独立实现 session_key、verification_command 等） |
| Claude | `claude_code_hooks_tests.rs` | 1,037 | — |
| Cursor | `cursor_hooks/` | 12,426+ | 部分（handlers、review_gate、stop、outbound、subagent） |
| Codex | `codex_hooks/` | 4,826+ | 部分（handlers、install、state、pretool） |
| OpenCode | `opencode_hooks.rs` | 498 | 完全使用 `hook_dispatch` 共享函数 |

### Provider 适配器

| 文件 | 行数 | 功能 |
|------|------|------|
| `claude_provider.rs` | 109 | Claude provider |
| `cursor_provider.rs` | 62 | Cursor provider |
| `codex_provider.rs` | 107 | Codex provider |
| `opencode_provider.rs` | 61 | OpenCode provider |

### MCP stdio harness

| 文件 | 行数 | 功能 |
|------|------|------|
| `mcp_stdio_harness/mod.rs` | ~1,470 | MCP 工具分发主循环 |
| `mcp_stdio_harness/tools.rs` | ~1,380 | 工具实现（evidence、goal、rfv、closeout、session、snapshot、web_fetch、skill_route/skill_search/skill_read/skill_route_status） |

### 已知技术债

- Claude hooks 和 Cursor hooks 未完全迁移到 `hook_dispatch` 共享抽象
- Claude hooks 有独立的文件锁实现（`ClaudeReviewStateLock`），与 `file_state_lock.rs` 重复
- `opencode_hooks` 的 `append_shell_evidence` 是空实现
- `opencode_hooks` 的 `is_reviewer_tool_name` 过于宽泛（子串匹配 "review"/"agent"/"task"）
- `opencode_hooks` 的 `classify_protected_path` 硬编码了 Claude 特定路径
