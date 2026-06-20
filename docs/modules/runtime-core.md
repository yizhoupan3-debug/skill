---
module: runtime-core
lines: ~20000
layer: B1
last_verified: "2026-06-19"
---

# runtime-core（B1 层 — facade crate）

框架运行时核心 facade crate（v7 拆分后保留 ~20K 行，含 contracts/ re-export 层；纯实现 ~15K），包含 CLI、RFV 循环、framework_runtime、维护命令等。提取的子 crate 见新 crate 文档。

## 职责

提供框架运行时的完整执行引擎（facade）：CLI 解析、命令分发、RFV 循环、closeout 评估、evidence 管理。从 v6.5 的 ~38K 行拆分为 facade + 5 个子 crate（`framework-runtime`、`session-supervisor`、`runtime-storage`、`trace-runtime`、`runtime-core-contracts`）。

## 顶层模块索引

| 模块 | 行数 | 功能 |
|------|------|------|
| **`framework_runtime/`** | 5,700+ | **核心运行时**（stdin dispatch、doctor、session artifacts、alias 等） |
| `cli/` | 973 | CLI 参数解析与分发 |
| `rfv_loop` | 1,800 | RFV（Review-Fix-Verify）循环完整实现 |
| `framework_maint` | 1,808 | 框架维护 CLI 子命令 |
| `stdio_transport` | 868 | stdio 传输层 |
| `codegraph_mcp/` | — | codegraph MCP 薄壳 |
| `schema_drift` | 617 | Schema 版本漂移检测 |
| `route/` | 703 | 路由元数据 |
| + 其余模块（~2K 行合计） |

### 提取的子 crate（v7）

| crate | 位置 | 行数 | 对应 runtime-core 原模块 |
|-------|------|------|--------------------------|
| **framework-runtime** | `core/framework-runtime/` | ~5K | `closeout_enforcement.rs`, `execution_contract.rs`, `pre_tool_use_guard.rs`, `runtime_view.rs`, `trace_stream_io.rs`, `trace_attach.rs`, `trace_transport.rs`, `live_execute.rs`, `sandbox_control.rs`, `evolution_observer.rs` |
| **session-supervisor** | `core/session-supervisor/` | ~5K | `session_supervisor/`（driver/worker/runtime/process/evolution_idle） |
| **runtime-storage** | `core/runtime-storage/` | ~8K | `runtime_storage/`, `background_state/` |
| **trace-runtime** | `core/trace-runtime/` | ~1K | `trace_runtime.rs` |

## 安全/防护机制

## 安全/防护机制

| 模块 | 功能 |
|------|------|
| `web_fetch_guard` | SSRF 防护：URL 域名白名单/黑名单 |
| `hook_outbound_protect` | hook 出站内容过滤 |
| `mcp_pre_guard` | MCP 工具调用预检 |

## 依赖关系

- **依赖**: `framework-kernel`、`core-policy`、`core-state`、`host-projection`（re-export）
- **被依赖**: `router-rs`（CLI binary）

## 近期变更

- v6.5: `framework_host_targets` 迁移至 `framework-kernel`，通过 `pub use` 保持兼容
- v6.5: 新增 `mod_tests.rs`（evidence lock order、resolve_repo_root、truncate_utf8 测试）
- v7: 拆分为 facade（~14K）+ 4 个子 crate：`framework-runtime`、`session-supervisor`、`runtime-storage`、`trace-runtime`
- v7: `closeout_enforcement.rs`、`execution_contract.rs`、`pre_tool_use_guard.rs`、`runtime_view.rs` 等移至 `core/framework-runtime/`
- v7: `background_state/` 移至 `core/runtime-storage/`
- v7: `session_supervisor/`（driver/worker/runtime/process/evolution_idle）移至 `core/session-supervisor/`
- v7: `trace_runtime.rs` 移至 `core/trace-runtime/`

## 已知技术债

- `framework_runtime/` 子模块 ~5,700 行（提取后缩减，仍为较大模块）
- `paper_adversarial_hook` 和 `paper_prose_hook` 在文档中未描述

## framework_runtime 子模块详解

runtime-core 的核心子模块（v7 拆分后保留 ~5,700 行），包含 stdin dispatch、doctor、session artifacts、alias 构建等。提取到 `core/framework-runtime/` 的模块见该 crate 文档。

### 子模块索引

| 子模块 | 行数 | 功能 |
|--------|------|------|
| `mod.rs` | 180 | 主入口：重新导出子模块，closeout 评估、evidence append、session artifacts |
| `mod_tests.rs` | 217 | 模块测试 |
| `alias.rs` | 743 | framework alias 构建与分发 |
| `framework_doctor.rs` | 646+ | 框架诊断（continuity audit） |
| `stdio_dispatch.rs` | 573+ | stdio 命令分发 |
| `orchestration_controller.rs` | 1,176 | 编排控制器 |
| `session_artifacts.rs` | 805 | 会话产物写入 |
| `prompt_compression.rs` | 145 | 提示压缩 |
| `route_manifest_fallback.rs` | 147 | 路由 manifest 回退 |
| `statusline.rs` | ~150 | 状态行构建 |

#### 提取至 core/framework-runtime/ 的模块

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

### 关键数据流

1. **PreToolUse 流**: `pre_tool_use_guard.rs` → `evaluate_hook_policy`（core-policy）→ block/allow
2. **Evidence 流**: `mod.rs::append_evidence_index_merged_row` → `EVIDENCE_INDEX.json`
3. **Closeout 流**: `closeout.rs::evaluate_closeout_record_file_for_task` → `closeout_enforcement` → gate verdict
4. **Command 流**: `stdio_dispatch.rs` → `router_command_dispatch.rs` → 子命令

### 已知技术债

- `is_terminal` 在 `mod.rs` 和 `runtime_view.rs`（已移至 core/framework-runtime）重复定义
- `mod_tests.rs` 中的 `include_str!("mod.rs")` 源码检查测试非常脆弱
- `orchestration_controller.rs`（1,176 行）逐渐成为新的模块热点
