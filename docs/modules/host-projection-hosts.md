---
module: host-projection::hosts
lines: ~15700
layer: B2
last_verified: "2026-06-13"
---

# hosts 子模块详解

四宿主 hook 实现 + 共享抽象层。

## 共享抽象

| 文件 | 行数 | 功能 |
|------|------|------|
| **`hook_dispatch.rs`** | 514 | 统一 hook 分发 trait：`HostHookConfig` + `HostHookDispatcher` |
| **`file_state_lock.rs`** | 224 | 跨宿主文件状态锁：`FileStateLockGuard` + `HookStateConfig` |
| `host_provider.rs` | 495 | `HostProvider` trait + 注册表 + 路由别名 |
| `hook_state_common.rs` | 3 | hook state 版本适配 |

### hook_dispatch.rs 架构

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

### file_state_lock.rs

- `FileStateLockGuard`: RAII 文件锁（Unix: flock, non-Unix: create_new）
- `HookStateConfig`: 配置 host_id/state_dir_leaf/state_filename，提供 `with_state_lock` 泛型方法
- `read_stdin_json`: 带 4 MiB 限制的 stdin JSON 读取

## 四宿主 hook 实现

| 宿主 | 文件 | 行数 | 使用共享抽象 |
|------|------|------|-------------|
| Claude Code | `claude_code_hooks.rs` | 1,485 | 部分（独立实现 session_key、verification_command 等） |
| Claude Code | `claude_code_hooks_tests.rs` | 1,037 | — |
| Cursor | `cursor_hooks/` | 12,426+ | 部分（handlers、review_gate、stop、outbound、subagent） |
| Codex | `codex_hooks/` | 4,826+ | 部分（handlers、install、state、pretool） |
| OpenCode | `opencode_hooks.rs` | 498 | 完全使用 `hook_dispatch` 共享函数 |

## Provider 适配器

| 文件 | 行数 | 功能 |
|------|------|------|
| `claude_provider.rs` | 109 | Claude Code provider |
| `cursor_provider.rs` | 62 | Cursor provider |
| `codex_provider.rs` | 107 | Codex provider |
| `opencode_provider.rs` | 61 | OpenCode provider |

## MCP stdio harness

| 文件 | 行数 | 功能 |
|------|------|------|
| `mcp_stdio_harness_dir/mod.rs` | 1,600 | MCP 工具分发主循环 |
| `mcp_stdio_harness_dir/tools.rs` | 900+ | 工具实现（evidence、goal、rfv、closeout、session、snapshot、web_fetch、skill_route/skill_search/skill_read/skill_route_status） |

## 已知技术债

- Claude hooks 和 Cursor hooks 未完全迁移到 `hook_dispatch` 共享抽象
- Claude hooks 有独立的文件锁实现（`ClaudeReviewStateLock`），与 `file_state_lock.rs` 重复
- `opencode_hooks` 的 `append_shell_evidence` 是空实现
- `opencode_hooks` 的 `is_reviewer_tool_name` 过于宽泛（子串匹配 "review"/"agent"/"task"）
- `opencode_hooks` 的 `classify_protected_path` 硬编码了 Claude 特定路径
