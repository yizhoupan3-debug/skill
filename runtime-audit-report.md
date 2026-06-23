# 运行层全面审计报告

**审计日期**: 2026-06-23  
**范围**: 11 个运行层 crate, ≈58,235 行 Rust 源码  
**阶层**: 按 6 层组织: 行为层 / 编排层 / Hook 层 / 基础设施 / 状态层 / 薄调度桥接层

---

## 0. 分层架构总览

```
┌──────────────────────────────────────────┐
│  ⑴ 行为层 (loop-engine 6,084 行)          │
│      runner / types / state / closeout    │
├──────────────────────────────────────────┤
│  ⑵ 编排层 (session-supervisor 3,034 行)   │
│      worker / process / driver / runtime  │
├──────────────────────────────────────────┤
│  ⑶ Hook 层 (host-projection + contracts)  │
│      hooks 注册表 / 事件路由 / 观测 /      │
│      出站保护 / router_rs_obs             │
├──────────────────────────────────────────┤
│  ⑷ 基础设施层                              │
│     framework-runtime (18,344) /          │
│     runtime-infra (1,675) /               │
│     runtime-storage (5,619) /             │
│     trace-runtime (1,103) /               │
│     framework-kernel (7,769) /            │
│     framework-extra                       │
├──────────────────────────────────────────┤
│  ⑸ 状态层 (core-state 7,207 行)           │
│      state_manager / task_state /         │
│      step_ledger / quality_gate / exit    │
│      gate / goal_prediction               │
├──────────────────────────────────────────┤
│  ⑹ 薄调度/桥接层                           │
│     runtime-core (2,730) /                │
│     runtime-core-contracts (1,885)        │
└──────────────────────────────────────────┘
```

---

## 一、行为层（loop-engine）

**文件**: `core/loop-engine/` — 9 文件, 6,084 行

### 1.1 `runner.rs:49` — 过期 `#[allow(dead_code)]`

```rust
#[allow(dead_code)]
pub fn default_max_depth() -> u32 { 5 }  // 实际被外部调用
```

`router-rs/src/cli/router_command_dispatch.rs:833` 调用了它。注解残留。

**严重度**: 低 · **操作**: 移除 `#[allow(dead_code)]`

### 1.2 `types.rs` 547 行 — 16 个 struct 集中于一个文件

建议将 `LoopRegistryEntry` / `LoopAction` / `LoopRunState` 拆出独立文件。

**严重度**: 低

---

## 二、编排层（session-supervisor）

**文件**: `core/session-supervisor/` — 10 文件, 3,034 行

### 2.1 模块边界清晰, 结构良好

`process` / `worker` / `driver` / `runtime` / `hooks` 各司其职。`tests.rs` 1370 行但都是测试。

### 2.2 `required_non_empty_string` 私有副本（❗跨层重复 #B）

`session-supervisor/src/runtime.rs:153-178` 与 `framework-runtime/src/json_value.rs:101-129` 几乎完全相同。应引用真源。

**严重度**: 中 · **操作**: 统一引用 `framework_runtime::json_value`

---

## 三、Hook 层（host-projection + runtime-core-contracts hook_* 模块）

### 3.1 ❗P1 `host-projection/hooks.rs` — `router_env_flags` 函数 100+ 行重复

`host-projection/src/hooks.rs:135-237` 内联定义了多个 `router_rs_*` 函数, 与 `framework-runtime/src/router_env_flags.rs` 重复。

**检测到的重复清单**:

| 函数 | framework-runtime | host-projection | 差异 |
|------|-------------------|-----------------|------|
| `router_rs_pre_goal_enabled()` | env 读取 | 内联副本 | 相同 |
| `router_rs_hook_silent_enabled()` | env 读取 | 内联副本 | 相同 |
| `router_rs_hook_outbound_context_max_bytes()` | `clamp(1024,65536)` | `unwrap_or(8192)` | **默认值不同!** |
| `router_rs_hook_state_lock_retries()` | 默认 100 | 默认 8 | **默认值已分歧!** |
| `router_rs_review_gate_stop_max_nudges_cap()` | env 读取 | 内联副本 | 相同 |
| `router_rs_env_enabled_default_true/false` | 基础函数 | 内联副本 | 相同 |

默认值已分歧（100 vs 8）—— 未来问题根源。

**严重度**: **高** · **操作**: host-projection 改为引用 `framework_runtime::router_env_flags`

### 3.2 `post_tool_call_succeeded` / `try_append_post_tool_shell_evidence`

host-projection 版本是服务定位器模式（OnceLock fn pointer wrappers），不是重复。**合理设计**。

### 3.3 `runtime-core-contracts` — 文档声明过时

- `lib.rs:3-5` 声明 "MUST NOT depend on framework-runtime, host-projection"
- 实际 Cargo.toml 包含 `host-projection` 依赖（代码中**未使用**）
- 列举的叶子 `core-state` 不在 Cargo.toml 中

**严重度**: 低 · **操作**: 更新文档 + 移除死依赖

### 3.4 Hook 层各模块职责清晰

`hook_event_routing` / `hook_observation_rules` / `hook_outbound_protect` / `router_rs_obs` 无依赖循环。

---

## 四、基础设施层

### 4.1 ❗P1 `framework-runtime` — 20 个扁平模块需目录化

所有 20 个 `.rs` 文件平铺在 `src/` 根目录。最大文件:

| 文件 | 行数 | 测试标记 |
|------|------|---------|
| `closeout_enforcement.rs` | **1,227** | 0 |
| `execution_contract.rs` | **1,056** | 0 |
| `runtime_view.rs` | 969 | 0 |
| `trace_stream_io.rs` | 954 | 0 |
| `trace_attach.rs` | 798 | 0 |
| `live_execute.rs` | 755 | 0 |

**建议分组**:
- `trace/`: trace_attach, trace_stream_io, trace_transport
- `io/`: io_utils, json_io, json_value, stdio_op_registry
- `contracts/`: closeout_enforcement, execution_contract, pre_tool_use_guard
- `exec/`: live_execute, sandbox_control
- `infra/`: router_env_flags, constants, types, util, hooks

**严重度**: 中

### 4.2 ❗P1 `quality_gate.rs` 1874 行 零测试

`core/runtime-exit-gate/src/quality_gate.rs` — 生产代码 1874 行, `#[cfg(test)]` 标记 0 个。

包含 `framework_quality_gate` 主入口、`enforce_rfv_close_gates`、`parse_close_gates`、`read_evidence_index_artifacts_impl`、`cross_link_evidence` 等复杂逻辑——全部无测试。

**严重度**: **高** · **操作**: 拆分 + 补测试（优先级最高）

### 4.3 ❗P2 `trace-runtime` — 单文件 1103 行

`core/trace-runtime/src/lib.rs` — 单文件无子模块。

包含 `compact_trace_stream()`(~300 行)、5 个 `pub struct`、8 个 `pub fn`、20+ 私有函数、11 个 schema version 常量。

**操作**: 拆分为 `record.rs` / `compact.rs` / `digest.rs` / `cursor.rs`

### 4.4 `runtime-storage/tests.rs` 1924 行

单一测试文件 1924 行, 可拆分为 `test_backend.rs` / `test_paths.rs` 等。非生产代码问题。

**严重度**: 低

### 4.5 `runtime_view.rs` 968 行

34 个函数全部操作 `serde_json::Value`。但所有函数服务于同一调用路径，目前可接受。

---

## 五、状态层（core-state）

### 5.1 `task_state.rs` 1716 行

生产代码约 900 行 + 测试 800 行。建议拆分为 `resolve.rs` / `depth_compliance.rs` / `continuity.rs`。

**严重度**: 中

### 5.2 `state_manager/mod.rs` 1238 行

`mod.rs` 本身定义了约 800 行逻辑（`read_goal_state` 等），同时通过 `pub use` 回收 5 个子模块。应考虑将 `mod.rs` 自身的逻辑移到 `goal_ops.rs` 或新文件。

**严重度**: 中

### 5.3 ❗P1 `source_traceable_heuristic` / `EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN` 完全相同

| 项 | `runtime-exit-gate/quality_gate.rs` | `core-state/validation.rs` |
|---|---|---|
| `source_traceable_heuristic()` | 17 行 **本地定义** (第 26 行) | **17 行完全相同** (第 29 行) |
| `EXTERNAL_RESEARCH_STRICT_TRACE_MIN_LEN` | `pub const = 40` (第 20 行) | `pub const = 40` (第 6 行) |

`quality_gate.rs` 从 `core-state` 导入了 `validate_external_research_*` 但 `source_traceable_heuristic` 自己是私有副本。代码迁移残留。

**操作**: 统一到 core-state, quality_gate.rs 改为导入

### 5.4 ❗P1 `quality_gate_state_path` 用常量 vs 字面量

- `runtime-exit-gate/quality_gate.rs:347`: 使用 `QUALITY_GATE_STATE_FILENAME` 常量
- `core-state/quality_gate_ops.rs:11`: 硬编码 `"RFV_LOOP_STATE.json"`

如果常量有一天被改而字面量不更新，会产生路径不一致的 bug。

**操作**: 统一定义

### 5.5 ❗P2 `framework_quality_gate` 3 层定义

| 层 | 文件 | 角色 |
|-----|------|------|
| 最底层 | `core-state/src/quality_gate.rs:18` | 精简实现（早期版本） |
| 完整层 | `runtime-exit-gate/src/quality_gate.rs:430` | 完整实现（含 harness ops、hook resolution） |
| 包装层 | `runtime-infra/src/telemetry_emit.rs:134` | 薄包装（加 telemetry,委托到 exit-gate） |

core-state 版本仍被 `host-projection/tools.rs:1030` 引用作为 fallback。

**操作**: 确定是否需要保留 core-state 版本, 或改为统一委托到 exit-gate

### 5.6 `exit_gate_evaluator.rs` — 未接入的 trait

`ExitGateEvaluator` trait + `NoopExitGateEvaluator` + `GateResult` 定义了 122 行, 零外部引用。v9 roadmap 预留代码。

**操作**: 如非近期计划, 应标记或删除

---

## 六、薄调度/桥接层

### 6.1 ❗P2 `runtime-core` — `runtime_registry` 模块过度 re-export

- `runtime-core/src/lib.rs` 约有 100+ `pub use` 从外部 crate 透传
- `pub mod runtime_registry { ... }` 块（~30 个项）**不添加任何逻辑**
- 建议让消费者直接引用 `framework_kernel::runtime_registry` 和 `core_policy::registry_review_gate`

**操作**: 移除 runtime_registry 汇总模块

### 6.2 ❗P2 `runtime-infra::router_env_flags` 不必要门面

```rust
// 理由 "avoid pulling framework-runtime into runtime-core-contracts" 不成立
// runtime-infra 已依赖 framework-runtime, runtime-core 已依赖两者
pub use framework_runtime::router_env_flags::*;
```

**操作**: 删除此文件, runtime-core 直接 `pub use framework_runtime::router_env_flags::*`

### 6.3 ❗P2 `web_fetch_guard.rs` — 3 个死函数

**确认零外部调用者**:
- `validate_and_resolve_web_fetch_url_as_strings` (第 196 行)
- `resolve_web_fetch_redirect_as_string` (第 204 行)
- `resolve_web_fetch_addresses_as_strings` (第 210 行)

只是 `reqwest::Url` / `SocketAddr` → `String` 的薄包装, 从未被调用。

**操作**: 删除

### 6.4 ❗P1 `mod_tests.rs` 断链引用

`core/runtime-core/src/framework_runtime/mod.rs:177`:
```rust
#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
```

对应文件 `mod_tests.rs` **在磁盘上不存在**。运行 `cargo test` 在依赖方会编译报错。

**操作**: 删除该 `#[path]` 引用或创建空测试文件

---

## 七、跨层系统性发现

### 7.1 代码重复矩阵

| # | 模式 | 影响层 | 出现位置 |
|---|------|--------|---------|
| **A** | `router_env_flags` 函数重复 | **Hook** | `host-projection/hooks.rs` 100+ 行 vs `framework-runtime/router_env_flags.rs` |
| **B** | `required_non_empty_string` / `optional_*` | **编排+状态+基础** | session-supervisor, core-state/step_ledger, runtime-storage/operation |
| **C** | `now_iso()` 私有包装 | **所有层** | quality_gate, status, state_manager, research-harness×3 |
| **D** | `source_traceable_heuristic` | **状态+基础** | runtime-exit-gate + core-state, 完全一致 |
| **E** | `quality_gate_state_path` | **状态+基础** | runtime-exit-gate + core-state, 常量 vs 字面量 |
| **F** | `current_local_timestamp` | **基础** | browser-mcp, session_artifacts, projection_bootstrap |
| **G** | `sha256_hex` 私有副本 | **基础** | session_artifacts → 应引用 trace_runtime |

### 7.2 ❗P1 serde_json::Value 系统性滥用

几乎所有大文件都密集操作 `serde_json::Value` 而非强类型 struct:

- `runtime_view.rs` — 34 个函数几乎全部用 `Value::as_object()`
- `execution_contract.rs` — 80 行 normalize 函数全部在 Value 域
- `closeout_enforcement.rs` — 定义强类型但仍用 `&Value` 做手动字段解构
- `trace_stream_io.rs` — 近乎全部 Value 操作
- `loop-engine/runner.rs` — `parse_discovery_output()` 手动链而非 Deserialize

**影响**: ~3,000 行密集提取代码。无类型检查、无 IDE 补全。

### 7.3 unsafe — 97% 是 env 操作

总计 137 处 `unsafe`:
- 133 处（97%）: `std::env::set_var()` / `std::env::remove_var()` 需要 unsafe
- 4 处: `session-supervisor/process.rs` 中 libc 调用（合理）
- 散布在 12+ 个文件中

**操作**: 包装 `unsafe fn set_env(key: &str, val: &str)` 消除 ~100 处重复

### 7.4 非测试代码 706 个 unwrap()/expect()

- 非测试: 295 `unwrap()` + 411 `expect()` = 706 个 panic 调用
- `closeout_enforcement.rs` 生产代码中 15+ `expect("evaluate closeout")`
- `pre_tool_use_guard.rs` 生产代码 10 个 `expect("evaluate")` / `expect("approve")`

**操作**: 用 `anyhow::Context` + `?` 替代, 或优先 `if let Some(x) = ...` 模式

### 7.5 非问题（合理设计）

- ~25 个 OnceLock 全局变量 — 标准 Rust 惰性初始化
- `is_terminal()` 多个变体 — 服务于不同目的（str match / enum branch / status check）
- `post_tool_call_succeeded` host-projection 版本 — 服务定位器模式, 正确
- `closeout` 跨 3 个 crate — 分层清晰（core logic / wrappers / schema consultation）

---

## 八、优先级行动清单

### 🔴 立即（P0）

| # | 问题 | 文件 |
|---|------|------|
| 1 | `mod_tests.rs` 断链 | `runtime-core/framework_runtime/mod.rs:177` |
| 2 | `quality_gate.rs` 1874 行零测试 | `runtime-exit-gate/quality_gate.rs` |

### 🟡 本周（P1）

| # | 问题 | 跨层影响 | 操作 |
|---|------|---------|------|
| 1 | `source_traceable_heuristic` 重复 | **状态+基础** | 统一到 core-state |
| 2 | `quality_gate_state_path` 常量 vs 字面量 | **状态+基础** | 统一定义 |
| 3 | `router_env_flags` host-projection 100+ 行重复 | **Hook** | host-projection 引用真源 |
| 4 | `framework_quality_gate` 3 层定义 | **状态+基础** | 确定唯一入口 |
| 5 | `closeout_enforcement.rs` 1227 行 → 子目录 | **基础设施** | 拆 closeout/ |
| 6 | `execution_contract.rs` 1056 行 → 子目录 | **基础设施** | 拆 contracts/ |
| 7 | serde_json::Value → 强类型（runtime_view / execution_contract / closeout_enforcement） | **基础设施** | 定义 struct |
| 8 | `framework-runtime` 20 扁平模块目录化 | **基础设施** | 建议分组 |

### 🟢 月度（P2）

| # | 问题 | 操作 |
|---|------|------|
| 1 | `web_fetch_guard.rs` 3 个死函数 | 删除 |
| 2 | `exit_gate_evaluator.rs` 未使用 | 标记或删除 |
| 3 | `runtime-core` `runtime_registry` 过度 re-export | 移除汇总模块 |
| 4 | `runtime-infra::router_env_flags` 不必要门面 | 删除文件 |
| 5 | `required_non_empty_string` 4 份副本 | 统一引用 |
| 6 | `now_iso()` 7 个私有包装 | 改用 pub use |
| 7 | `current_local_timestamp` 6 处定义 | 统一引用 |
| 8 | unsafe env 操作统一 | 包装 set_env() |
| 9 | `trace-runtime` 单文件 1103 行 | 拆分子模块 |
| 10 | `runtime-core-contracts` 文档 + 死依赖 | 更新 |

---

## 九、Crate 健康度矩阵

| Crate | 行数 | 文件 | 主要问题 | 健康度 |
|-------|------|------|---------|--------|
| framework-runtime | 18,344 | 20 | 扁平无子目录、4 个 >900 行无测试、serde_json::Value 泛滥 | 🟡 需重构 |
| runtime-exit-gate | 2,785 | 4 | quality_gate 1874 行零测试 | 🔴 高风险 |
| runtime-core | 2,730 | 12 | 过度 re-export、mod_tests 断链 | 🟡 需修复 |
| runtime-core-contracts | 1,885 | 10 | web_fetch_guard 死函数、文档过期 | 🟢 需清理 |
| runtime-storage | 5,619 | 16 | tests.rs 1924 行组织不当 | 🟢 结构良好 |
| runtime-infra | 1,675 | 6 | router_env_flags 门面多余 | 🟢 轻量 |
| loop-engine | 6,084 | 9 | types.rs 可拆分 | 🟢 结构良好 |
| session-supervisor | 3,034 | 10 | required_non_empty_string 副本 | 🟢 结构良好 |
| framework-kernel | 7,769 | 18 | cli_args 617 行无测试 | 🟢 依赖反转正确 |
| core-state | 7,207 | 22 | task_state 1716 行、重复定义 | 🟡 部分需重构 |
| trace-runtime | 1,103 | 1 | 单文件 1103 行 | 🟡 需拆分 |
