---
module: runtime-core::framework_runtime
lines: ~5700
layer: B1
last_verified: "2026-06-16"
---

# framework_runtime 子模块详解

runtime-core 的核心子模块（v7 拆分后保留 ~5,700 行），包含 stdin dispatch、doctor、session artifacts、alias 构建等。提取到 `core/framework-runtime/` 的模块见该 crate 文档。

## 子模块索引

| 子模块 | 行数 | 功能 |
|--------|------|------|
| `mod.rs` | 1,900+ | 主入口：closeout 评估、evidence append、session artifacts |
| `mod_tests.rs` | 217 | 模块测试 |
| `alias.rs` | 743 | framework alias 构建与分发 |
| `framework_doctor.rs` | 646+ | 框架诊断（continuity audit） |
| `stdio_dispatch.rs` | 573+ | stdio 命令分发 |
| `orchestration_controller.rs` | 1,176 | 编排控制器 |
| `session_artifacts.rs` | 805 | 会话产物写入 |
| `prompt_compression.rs` | 145 | 提示压缩 |
| `route_manifest_fallback.rs` | 147 | 路由 manifest 回退 |
| `statusline.rs` | ~150 | 状态行构建 |

### 提取至 core/framework-runtime/ 的模块

| 原模块 | 迁移至 |
|--------|--------|
| `runtime_view.rs` | `core/framework-runtime/runtime_view.rs` |
| `trace_stream_io.rs` | `core/framework-runtime/trace_stream_io.rs` |
| `trace_attach.rs` | `core/framework-runtime/trace_attach.rs` |
| `trace_transport.rs` | `core/framework-runtime/trace_transport.rs` |
| `evolution_observer.rs` | `core/framework-runtime/evolution_observer.rs` |
| `sandbox_control.rs` | `core/framework-runtime/sandbox_control.rs` |
| `live_execute.rs` | `core/framework-runtime/live_execute.rs` |
| `pre_tool_use_guard.rs` | `core/framework-runtime/pre_tool_use_guard.rs` |
| `closeout_enforcement.rs` | `core/framework-runtime/closeout_enforcement.rs` |
| `execution_contract.rs` | `core/framework-runtime/execution_contract.rs` |

## 关键数据流

1. **PreToolUse 流**: `pre_tool_use_guard.rs` → `evaluate_hook_policy`（core-policy）→ block/allow
2. **Evidence 流**: `mod.rs::append_evidence_index_merged_row` → `EVIDENCE_INDEX.json`
3. **Closeout 流**: `mod.rs::evaluate_closeout` → `closeout_enforcement` → gate verdict
4. **Command 流**: `stdio_dispatch.rs` → `router_command_dispatch.rs` → 子命令

## 已知技术债

- `is_terminal` 在 `mod.rs` 和 `runtime_view.rs`（已移至 core/framework-runtime）重复定义
- `mod_tests.rs` 中的 `include_str!("mod.rs")` 源码检查测试非常脆弱
- `orchestration_controller.rs`（1,176 行）逐渐成为新的模块热点
