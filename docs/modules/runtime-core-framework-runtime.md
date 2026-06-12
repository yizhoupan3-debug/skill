---
module: runtime-core::framework_runtime
lines: ~12100
layer: B1
last_verified: "2026-06-13"
---

# framework_runtime 子模块详解

runtime-core 的核心子模块，包含框架运行时的完整执行逻辑。

## 子模块索引

| 子模块 | 行数 | 功能 |
|--------|------|------|
| `mod.rs` | 1,914 | 主入口：closeout 评估、evidence append、session artifacts |
| `mod_tests.rs` | 217 | 模块测试（evidence lock order、resolve_repo_root） |
| `orchestration_controller.rs` | 1,176 | 编排控制器 |
| `trace_stream_io.rs` | 1,071 | trace 流式 I/O |
| `runtime_view.rs` | 1,055 | 运行时视图（continuity audit、workspace name） |
| `live_execute.rs` | 850 | 实时执行引擎（prompt 构建、evidence 注入） |
| `trace_attach.rs` | 798 | trace 附加 |
| `alias.rs` | 743 | framework alias 构建与分发 |
| `session_artifacts.rs` | 805 | 会话产物写入 |
| `router_command_dispatch.rs` | 837 | router 命令分发（profile、scaffold、diagnose） |
| `pre_tool_use_guard.rs` | 637 | PreToolUse 守卫评估（protected path、settings warn） |
| `framework_doctor.rs` | 646 | 框架诊断（continuity audit） |
| `stdio_dispatch.rs` | 573 | stdio 命令分发 |
| `sandbox_control.rs` | 438 | 沙箱控制 |
| `evolution_observer.rs` | 415 | 演进观察者 |
| `trace_transport.rs` | 312 | trace 传输 |
| `prompt_compression.rs` | 145 | 提示压缩 |
| `route_manifest_fallback.rs` | 147 | 路由 manifest 回退 |
| `constants.rs` | 70 | Schema 版本常量 |

## 关键数据流

1. **PreToolUse 流**: `pre_tool_use_guard.rs` → `evaluate_hook_policy`（core-policy）→ block/allow
2. **Evidence 流**: `mod.rs::append_evidence_index_merged_row` → `EVIDENCE_INDEX.json`
3. **Closeout 流**: `mod.rs::evaluate_closeout` → `closeout_enforcement` → gate verdict
4. **Command 流**: `stdio_dispatch.rs` → `router_command_dispatch.rs` → 子命令

## 已知技术债

- `is_terminal` 在 `mod.rs:1907` 和 `runtime_view.rs:1039` 重复定义
- `mod_tests.rs` 中的 `include_str!("mod.rs")` 源码检查测试非常脆弱
- `mod.rs:1203` 曾有重复嵌套 `if` 条件（已修复）
