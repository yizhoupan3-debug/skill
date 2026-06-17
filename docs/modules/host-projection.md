---
module: host-projection
lines: ~17000
layer: B2
last_verified: "2026-06-16"
---

# host-projection（B2 层）

宿主投影层，负责将框架能力投影到四个闭集宿主（claude-code、cursor、codex、opencode）。

## 职责

宿主集成（install/status/remove）、hook 实现、MCP harness、入口同步。

## 顶层模块

| 模块 | 行数 | 功能 |
|------|------|------|
| `hooks` | 1,541 | 函数指针代理层（OnceLock slots，解耦 runtime-core 与宿主 hooks） |
| `host_entrypoint_sync` | 541 | 宿主入口同步 |
| `host_integration/` | 6,794 | 宿主集成核心（详见 [host-projection-projection.md](host-projection-projection.md)） |
| `hosts/` | ~15,700+ | 五宿主 hook 实现（详见 [host-projection-hosts.md](host-projection-hosts.md)） |

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
- v6.5: `mcp_stdio_harness.rs` 拆分为目录 `mcp_stdio_harness_dir/mod.rs` + `tools.rs`
- v6.5: 新增 `hook_dispatch.rs`（统一 hook 分发 trait）和 `file_state_lock.rs`（跨宿主文件锁）
- v6.5: `claude_code_hooks` 测试提取到独立文件

## 已知技术债

- `hooks.rs` 中 `WRITE_FRAMEWORK_SESSION_ARTIFACTS` 拼写已修正（原 ARTARTIFACTS）
- Claude hooks 和 Cursor hooks 未完全使用 `hook_dispatch` 共享抽象
- `opencode_hooks` 的 `append_shell_evidence` 是空实现
