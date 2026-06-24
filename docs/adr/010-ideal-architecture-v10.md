# ADR-010: 理想框架架构真源

**状态**: 真源 · **日期**: 2026-06-24 · **性质**: 框架架构的唯一权威参考（非执行计划）

---

## 目录

1. [架构总则](#1-架构总则)
2. [八层运行时模型总览](#2-八层运行时模型总览)
3. [DAG 验证矩阵](#3-dag-验证矩阵)
4. [宿主隔离契约](#4-宿主隔离契约)
5. [L0 Kernel — 纯抽象与共享类型](#5-l0-kernel--纯抽象与共享类型)
6. [L1 IO & Persistence — 存储后端与追踪](#6-l1-io--persistence--存储后端与追踪)
7. [L2 Contracts — 验证规则与守卫合约](#7-l2-contracts--验证规则与守卫合约)
8. [L3 Execution — LLM 执行与沙箱](#8-l3-execution--llm-执行与沙箱)
9. [L4 State Management — Task Engine 与路由](#9-l4-state-management--task-engine-与路由)
10. [L5 Hook Infrastructure — 事件路由与观测](#10-l5-hook-infrastructure--事件路由与观测)
11. [L6 Orchestration — 编排与自动化闭环](#11-l6-orchestration--编排与自动化闭环)
12. [L7 Bridge / Dispatch — 平台聚合与分发](#12-l7-bridge--dispatch--平台聚合与分发)
13. [Feature Layer — 可插拔研究能力](#13-feature-layer--可插拔研究能力)
14. [基础设施层 API 参考](#14-基础设施层-api-参考)
15. [产物目录](#15-产物目录)
16. [运行时层由来](#16-运行时层由来)
17. [已知架构债务](#17-已知架构债务)
18. [设计决策日志](#18-设计决策日志)

---

## 1. 架构总则

### 1.1 核心原则

| 编号 | 原则 | 说明 |
|------|------|------|
| **P1** | 每层职责唯一，不越界 | L4 不含 Research 领域逻辑，L0 不含业务状态 |
| **P2** | 宿主差异仅存于 L0 适配壳 | 宿主名/路径/env_var 映射全部从 `RUNTIME_REGISTRY.json` 编译期生成 |
| **P3** | 依赖方向单向向下 | Lⱼ 可依赖 Lᵢ 当 i ≤ j，禁止下层依赖上层 |
| **P4** | 禁止循环依赖 | DAG 矩阵（§3）编译期强制 |
| **P5** | 跨层通信通过 L0 或函数指针 | 不在高层硬编码低层细节 |
| **P6** | L0–L7 承载实质运行域 | 每层有明确的物理 crate 归属 |
| **P7** | Feature 层可插拔 | feature-gate 编译期可选，不硬编码宿主名 |
| **P8** | L0 完全无上层依赖 | 所有 L0 crate（6 个）零 L1-L7 依赖 |
| **P9** | 基础设施碎片收敛到唯一实现 | 每项功能只应有一个定义（§14） |
| **P10** | 函数指针注册表后备语义为 no-op | 不 panic，不硬阻断 |

### 1.2 Hook 通信模型

函数指针注册表（`framework-runtime-hooks/src/lib.rs`）是 L0 的一部分，作为跨层通信机制被 L4–L7 消费。

```
L4–L7 ──register(hooks)──→ L0 RUNTIME_CORE_HOOKS [OnceLock]
                                  │
L0 hook 事件到来 ──────────────→ RuntimeCoreHooks 方法调用
                                  │
                              L4–L7 注册的回调函数
```

**两个关键性质**：
- 注册方向（高层→L0）与调用方向（L0→高层）**相反**，这是依赖方向合规的关键设计
- OnceLock 未注册的 slot 通过 `try_hooks()` 静默返回 `None`，走 fallback/no-op 路径

**RuntimeCoreHooks 结构**（`framework-runtime-hooks/src/lib.rs:106-132`）：

```rust
pub struct RuntimeCoreHooks {
    pub telemetry: TelemetryHooks,       // 5 个遥测钩子
    pub host_provider: HostProviderHooks, // 4 个宿主提供者钩子
    pub framework_goal_drive: fn(Value) -> Result<Value, String>,
    pub framework_quality_gate: fn(Value) -> Result<Value, String>,
    pub handle_session_supervisor_operation: fn(Value) -> Result<Value, String>,
    pub handle_background_state_operation: fn(Value) -> Result<Value, String>,
    pub runtime_concurrency_defaults_payload: fn() -> Value,
    pub eval_route_contract: fn() -> Value,
    pub run_eval_route: fn(...) -> Result<Value, String>,
    pub generated_artifacts_status_for_repo: fn(&Path) -> Result<String, String>,
    pub ensure_kernel_bootstrap: fn(),
}
```

钩子分为两组子结构体 + 8 个独立字段（原 17 个扁平字段，2026-06 重构降低认知负荷）。

### 1.3 无固定阶段 Lifecycle

框架**没有**固定的 discuss→plan→implement→verify 阶段（四生命周期已在 2026-06 彻底退场）。

**Task 是底层执行引擎**。用户层表现为：定义 todo → 执行 todo → 完成 todo，以及与 Goal/RFV/Evidence 等状态的关联。

Lifecycle Profile 控制每个 task 的行为模式：
- **`interactive`**（默认）：用户主导，loop engine 不可调度，closeout 为 advisory
- **`loop-auto`**：允许 loop engine 自动调度（discovery → dispatch → verify 闭环）

---

## 2. 八层运行时模型总览

运行时 crate 按依赖方向严格分为 8 层（L0→L7），上层可依赖下层，禁止下层依赖上层。

```
L7      Bridge / Dispatch         runtime-core                stdio 分发、聚合 facade
                                     router-rs,               CLI 入口

L6      Orchestration              loop-engine,               RFV 闭环、可选自动化
                                     session-supervisor,
                                     framework-extra

L5      Hook Infrastructure        host-projection,           事件路由、观测埋点、
                                     runtime-exit-gate,        fn-pointer 消费端
                                     runtime-infra,
                                     runtime-core-contracts,
                                     mcp-tool-registry

L4      State Management           core-state,                Task Engine、Goal/QG 状态机、
                                     routing-engine,           路由评估、评分、决策
                                     skill-layer

L3      Execution                  fr-exec,                   LLM 实时执行、沙箱控制、
                                     framework-runtime,        运行时视图、环境标志
                                     browser-mcp

L2      Contracts                  fr-contracts,              验证规则、守卫合约
                                     core-state-types,         纯类型定义（零依赖）
                                     runtime-core-contracts

L1      IO & Persistence           fr-utils,                  JSON/文件/存储后端、
                                     runtime-storage,          trace 录制、IO 工具
                                     trace-runtime

L0      Kernel (B0)                framework-kernel,          纯抽象、共享类型、
                                     core-policy,              策略规则、env_flags
                                     core-state-utils,         IO/path/JSONL 原语
                                     framework-runtime-hooks,  fn-pointer 注册表 (OnceLock)
                                     telemetry-types,          遥测事件类型
                                     http-util                 HTTP 客户端工厂
```

### 与用户视角层的对应关系

| 用户视角 | 运行时层 | 核心 crate |
|----------|---------|-----------|
| Feature (L5) | 依赖 L6→L7 | research-harness |
| Runtime (L4) | L3+L4+L5+L6+L7 | runtime-core, loop-engine, framework-extra |
| Tool (L3) | 独立层 | browser-mcp, codegraph-rs |
| Skill (L2) | 纯契约层 | skills/\*/SKILL.md |
| Routing (L7) | L7 (dispatch) | routing-engine, host-projection |
| Host (L0) | L5 (hook) | host-projection/hosts |
| Base (L0) | L0+L1+L2+L4 | framework-kernel, core-policy, core-state |

**Task Engine**（L4）：`core-state` 承载 Task 全生命周期。用户层表现为：定义 todo → 执行 todo → 完成 todo。

**Loop Engine**（L6）：可选自动化增强层，仅对 `loop-auto` profile task 生效。不包含 discuss/plan/implement 阶段。

---

## 3. DAG 验证矩阵

```
         L0  L1  L2  L3  L4  L5  L6  L7
L0       ✓   -   -   -   -   -   -   -
L1       ✓   ✓   -   -   -   -   -   -
L2       ✓   ✓   ✓   -   -   -   -   -
L3       ✓   ✓   ✓   ✓   -   -   -   -
L4       ✓   ✓   ✓   ✓   ✓   -   -   -
L5       ✓   ✓   ✓   ✓   ✓   ✓   -   -
L6       ✓   ✓   ✓   ✓   ✓   ✓   ✓   -
L7       ✓   ✓   ✓   ✓   ✓   ✓   ✓   ✓
```

- Lⱼ 可依赖 Lᵢ 当 i ≤ j
- **L5 函数指针注册表**是唯一许可的跨层例外（注册方向 L4→L0 与调用方向 L0→L4 相反）
- 禁止 L4→L7 编译期依赖 override（通过 L5 函数指针间接调用）

---

## 4. 宿主隔离契约

### 4.1 注册表驱动架构

所有宿主元数据从 `configs/framework/RUNTIME_REGISTRY.json` **编译期生成**：

| 生成目标 | 源字段 | 产物 |
|---------|--------|------|
| `framework-kernel/build.rs` | `host_targets.metadata.*` | `generated_host_tables.rs` |
| `host-projection/build.rs` | `host_targets.supported`, `host_providers` | `generated_host_providers.rs` |

`generated_host_tables.rs` 生成：`ALL_HOST_IDS`、`host_private_config_dir()`、`review_gate_disable_env()`、`paper_prose_env()`、`paper_adversarial_env()`、`settings_guarded_paths()`、`generated_entrypoint_paths()`、`hook_state_unreadable_tag()`、`session_namespace_env()`、`is_ephemeral_task_id()`、`host_home_dirs()`、`ALL_KNOWN_HOST_DIRS`、`EPHEMERAL_PATH_PATTERNS`、`EPHEMERAL_TASK_PREFIXES`。

`generated_host_providers.rs` 生成：provider struct 定义 + `HostLifecycle`/`HostTelemetry`/`HostProvider` trait impl（全部从注册表数据生成，无手写 provider 文件）。

### 4.2 允许宿主知识的位置

| 位置 | 说明 |
|------|------|
| `RUNTIME_REGISTRY.json` | **唯一真相源**：所有宿主元数据 |
| `host-projection/` (L5) | 宿主适配层：`capability_overrides.rs`, `config.rs`, `dispatch.rs` |
| `host-projection/host_integration/` | 投影操作（注册表驱动） |
| `framework-kernel/build.rs` | 编译期生成表格和函数 |
| `host-projection/build.rs` | 编译期生成 provider struct 和 trait impl |
| **L0/L1/L2/L4 其他** | **不应出现宿主名** |

### 4.3 闭集宿主（4 个）

权威闭集：`claude`、`cursor`、`codex`、`opencode`。

退役 id（codex-cli, codex-app, claude-desktop, antigravity 系列）不再使用。添加新宿主只需编辑 `RUNTIME_REGISTRY.json`，重编译，所有代码自动生成。

### 4.4 宿主身份传递路径

```
用户输入 → AGENTS.md → L1 skill routing → L4 session (通过 HostProvider trait)
                                            ↓
                              host_provider_registry() 查找
                                            ↓
                              provider.dispatcher() → HostHookDispatcher::dispatch()
```

### 4.5 统一分派架构

不采用 4 个独立宿主钩子文件。统一实现：

```
host-projection/src/hosts/
├── mod.rs                  统一事件分派入口
├── stop_dispatch.rs        统一 Stop 决策管道（所有宿主共用）
├── event_handlers.rs       统一 UserPromptSubmit/PostToolUse
├── host_extensions.rs      宿主差异点（TouchState/review_gate/会话密钥）
├── mcp_pre_guard.rs        PreToolUse 路径保护
├── hook_state_common.rs    状态 CRUD
├── file_state_lock.rs      文件锁
└── hook_dispatch.rs        工具路由
```

宿主差异通过 `HostProvider` trait 注入，不在 hook handler 中做 `match host_id`。

---

## 5. L0 Kernel — 纯抽象与共享类型

L0 是整个框架的地基。**6 个 crate**，全部零 L1-L7 依赖（仅依赖外部 crate 如 serde、serde_json、fs2 等）。

### 5.1 `framework-kernel`

**职责**: 时间工具、repo 根发现、JSON Value 操作、tokenizer trait、telemetry trait、运行时注册表、CLI args 结构体、stdio payload 类型。

**模块清单**:

| 模块 | 职责 |
|------|------|
| `time` | `now_iso()` UTC ISO 8601 时间戳、`current_local_timestamp()` |
| `repo_roots` | `is_framework_root()`、`resolve_repo_root()` — 框架/项目根目录发现 |
| `json_value` | 16 个 JSON Value 提取/转换函数（`json_str_opt`、`json_u64_opt`、`json_bool_opt`、`safe_slug` 等） |
| `tokenizer` | `TokenizerProvider` trait、`tokenize_query()` — tokenizer 注入点 |
| `telemetry` | `TelemetryEvent`、`TelemetryWriter` trait、`LogAggregator` — 遥测管道 |
| `runtime_registry` | 编译期生成的宿主注册表函数（从 `RUNTIME_REGISTRY.json` 生成） |
| `framework_host_targets` | 宿主目标配置结构体 |
| `cli_args` | 所有 CLI 命令的 clap Args 结构体定义（~600 行定义 + ~1000 行测试） |
| `stdio_payload_types` | stdio 协议 payload 类型：`SandboxControlRequestPayload`（21 字段）、`BackgroundControlRequestPayload`（19 字段）、`TraceMetadataWriteRequestPayload`（30 字段）等 |
| `framework_profile` | 框架 profile 打包逻辑 |
| `skill_lint` | Skill schema 校验 |
| `skill_repo` | Skill 仓库操作 |
| `router_self` | router-rs 自身信息查询 |

**关键设计**: `json_value::safe_slug()` 保留 `_`.`-`，不做大小写折叠。`json_value` 通过 `pub use json_value::*` 在 crate 根重导出。

### 5.2 `core-policy`

**职责**: Hook 策略规则、env_flags、review gate 引擎、hook_common、goal_auto_detect。

**关键模块**:

| 模块 | 职责 |
|------|------|
| `env_flags` | 所有 `ROUTER_RS_*` 环境变量的**权威解析源**：`env_enabled_default_true/false`、`router_rs_review_gate_disabled_for_host`、`router_rs_task_ledger_flock_enabled` 等 |
| `hook_common` | hook 事件的通用逻辑：goal gate、review gate、subagent 识别 |
| `review_gate_engine` | Review lane 判定逻辑 |
| `goal_auto_detect` | 复杂度检测引擎（主动检测复杂任务） |
| `hook_policy` | MCP 工具安全审查（`dangerous_mcp_tool_reason`） |

**依赖**: `core-state-utils`(L0) + `framework-kernel`(L0)。L0 内部依赖合规。

### 5.3 `core-state-utils`

**职责**: IO/path/JSONL 原语。零内部依赖。

**模块清单**:

| 模块 | 职责 |
|------|------|
| `atomic_write` | 崩溃安全文件写入（temp+rename+fsync） |
| `path_guard` | 路径安全校验（防 path traversal） |
| `json_io` | JSON 泛型读写 |
| `jsonl_maintenance` | JSONL 文件维护（压缩、truncate corrupt tail） |
| `read_bounded` | 带上限的文件读取 |
| `task_write_lock` | flock 获取（`acquire_task_ledger_lock_with_timeout`，超时由调用方传入） |
| `text_utils` | 文本工具函数 |
| `env_sync` | `unsafe fn set_env/remove_env` — 测试用环境变量操作（Rust 1.66+ unsafe） |

### 5.4 `framework-runtime-hooks`

**职责**: fn-pointer 注册表（OnceLock），跨层通信中枢。零业务逻辑。

**公开 API**:
- `register(h: RuntimeCoreHooks)` — 注册钩子（仅首次生效）
- `hooks() -> &'static RuntimeCoreHooks` — 获取已注册钩子（未注册时 panic）
- `try_hooks() -> Option<&'static RuntimeCoreHooks>` — 获取钩子引用（未注册返回 None）
- `register_hook_duplicate_check(f)` / `check_hook_duplicates(repo_root)` — hook 重复检查

### 5.5 `telemetry-types`

**职责**: 纯遥测事件类型定义（`TelemetryEvent`、`PredictionOutcomeCheck`）。零内部依赖，仅 serde。

### 5.6 `http-util`

**职责**: HTTP 客户端工厂。仅一个函数 `cached_proxy_url()` — 缓存 `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` 环境变量解析结果。零生产依赖。

---

## 6. L1 IO & Persistence — 存储后端与追踪

L1 提供持久化基础设施。3 个 crate。

### 6.1 `fr-utils`

**职责**: JSON Value 提取辅助、IO 工具、常量、类型、env_flags。

**依赖**: `core-state-utils`(L0) + `framework-kernel`(L0) + `runtime-storage`(L1)。

**关键模块**: `json_value`（扩展提取函数）、`json_io`（read/write/if-exists）、`util`（`normalized_task_registry` 等）、`stdio_op_registry`（stdio 操作域注册）、`env_flags`（运行时 env 读取）、`io_utils`、`types`、`constants`。

### 6.2 `runtime-storage`

**职责**: 文件系统/SQLite/内存后端、路径解析。

**依赖**: `core-state-utils`(L0) + `framework-kernel`(L0)。

提供统一的存储抽象层，支持 `acquire_runtime_path_lock()` 文件锁操作。

### 6.3 `trace-runtime`

**职责**: Trace 录制、压紧。

**依赖**: `framework-kernel`(L0) + `runtime-storage`(L1)。

---

## 7. L2 Contracts — 验证规则与守卫合约

L2 定义验证规则和守卫合约，无状态管理逻辑。3 个 crate。

### 7.1 `core-state-types`

**职责**: 纯类型定义。零内部依赖（仅 serde + serde_json）。

**包含类型**: `task_state_types`（`ResolvedTaskView`、`TaskPointers`、`DepthCompliance`、`TaskControlMode`、`EvidenceRollup`、`GoalCompletionGates`）、`exit_gate_types`、`goal_prediction`。

**设计**: core-state 通过 `pub use core_state_types::task_state_types::*` 保持向后兼容，消费方通过 `core_state::task_state::*` 访问这些类型。

### 7.2 `fr-contracts`

**职责**: Closeout 验证、执行合约、pre-tool-use 守卫。

**依赖**: `fr-utils`(L1) + `core-policy`(L0) + `core-state-types`(L2) + `framework-kernel`(L0) + `framework-runtime-hooks`(L0)。

**关键模块**: `closeout_enforcement`、`execution_contract`、`pre_tool_use_guard`。

### 7.3 `runtime-core-contracts`

**职责**: Hook 事件路由规则、观测埋点、出站保护、URL 守卫。

**依赖**: `core-policy`(L0) + `framework-kernel`(L0)。

**关键模块**: `hook_event_routing`（hook 事件路由表）、`observation_rules`（观测规则）、`outbound_protection`（出站保护）、`url_guard`（URL 守卫）、`web_fetch_guard`（web fetch 守卫，所有 pub 函数均有真实调用者）。

---

## 8. L3 Execution — LLM 执行与沙箱

L3 是执行层，处理 LLM 实时交互。2-3 个 crate。

### 8.1 `fr-exec`

**职责**: LLM 实时执行、沙箱状态机、运行时视图、环境标志、trace I/O。

**依赖**: `fr-utils`(L1) + `fr-contracts`(L2) + `core-state-utils`(L0) + `core-state-types`(L2) + `core-policy`(L0) + `framework-kernel`(L0) + `host-projection`(L5) + `http-util`(L0) + `runtime-storage`(L1) + `trace-runtime`(L1)。

**关键模块**:

| 模块 | 行数 | 职责 |
|------|------|------|
| `runtime_view.rs` | ~700 | `classify_runtime_continuity` — 运行时连续性分类（285 行主函数，产出 22+ 字段的 JSON blob） |
| `router_env_flags.rs` | ~390 | `ROUTER_RS_*` 连续性/续跑类环境变量解析，委托 core-policy 真源 |
| `live_execute` | — | LLM 实时执行逻辑 |
| `sandbox_control` | — | 沙箱状态机控制 |
| `trace_stream_io` | — | Trace 流式 I/O |
| `trace_attach` / `trace_transport` | — | Trace 附加和传输 |
| `evolution_observer` | — | 路由演化观测 |

**设计注意**: `router_env_flags.rs` 中的 `router_rs_task_ledger_flock_enabled()` 是对 `core-policy::env_flags` 的委托包装（增加 tracing warning），而非独立实现。

### 8.2 `browser-mcp`

**职责**: 浏览器自动化 MCP 服务（Playwright 集成）。

**依赖**: `framework-kernel`(L0) + `http-util`(L0) + `host-projection`(L5) + `runtime-core-contracts`(L2)。

### 8.3 `framework-runtime`

**职责**: L3 facade（向后兼容 re-export）。已物理拆分为 `fr-utils`(L1) + `fr-contracts`(L2) + `fr-exec`(L3)，保留为 facade 层。下游 crate 直迁到子 crate，不再依赖 facade。

---

## 9. L4 State Management — Task Engine 与路由

L4 是状态管理层，包含框架的执行引擎核心。3 个 crate。

### 9.1 `core-state` — Task Engine（底层执行引擎）

**职责**: Task 状态机、Goal/RFV/Evidence 状态聚合、step_ledger、exit gates、TASK_LEDGER 幂等事务日志。

**依赖**: `core-state-types`(L2) + `core-state-utils`(L0) + `framework-kernel`(L0)。

#### Task 状态机

TaskControlMode 状态转换：

```
Idle（无活跃 goal/rfv）
  ↓ goal_state_manage start
GoalDrive（goal drive_until_done=true）
  ↓ quality_gate_manage start
QualityGate（rfv loop_status=active）
  ↓ 两者同时激活
Conflict（不一致，需人工介入）
```

#### 核心组件

| 组件 | 路径/类型 | 职责 |
|------|----------|------|
| Active/Focus 指针 | `active_task.json` / `focus_task.json` | 决定当前执行哪个 task |
| Goal 状态 | `GOAL_STATE.json` | task 的执行策略（goal、non-goals、done_when、validation_commands） |
| 事务日志 | `TASK_LEDGER.jsonl` | 幂等写入、回放、跨会话连续性 |
| 聚合投影 | `TASK_STATE.json` | goal + rfv + evidence 单一视图 |
| Step Ledger | `STEP_LEDGER.jsonl` | 步骤级追踪 |
| Evidence | `EVIDENCE_INDEX.json` | 验证证据记录 |

#### 混合加载策略（hydrate_task_state_hybrid）

Task 状态读取采用三阶段容错：

1. 优先读取 `TASK_STATE.json` 聚合投影（快速单文件读取）
2. 回退到物理文件（`GOAL_STATE.json` + `RFV_LOOP_STATE.json` + `EVIDENCE_INDEX.json`）
3. 回放 `TASK_LEDGER.jsonl` 中 seq > last_seq 的增量事务

#### 自动指针提升（ADR-001）

当 `active_task.json` 指向的 task 无 `GOAL_STATE.json`，而 `focus_task.json` 指向的 task 有 goal 时，`maybe_promote_focus_to_active_pointer` 自动将 focus 提升为 active。

#### Goal 操作

`goal_state_manage` 支持 8 种操作：`start`、`checkpoint`、`pause`、`resume`、`complete`（不物理删除，标记 `archived: true`）、`clear`、`block`、`amend`（支持自然语言 scope change）。

#### 关键模块

| 模块 | 职责 |
|------|------|
| `state_manager/mod.rs` | `read_goal_state`（含 `annotate_goal_staleness` 注入）、`current_env_session_id` |
| `state_manager/pointer_ops.rs` | 指针读写：`read_pointer_task_id`（多层 fallback）、`read_task_pointer_pair`、`set_task_focus`、`write_focus_task_pointer_minimal` |
| `state_manager/goal_ops.rs` | Goal 状态 CRUD 操作 |
| `task_state.rs` | `ResolvedTaskView` 构建、`read_task_ledger_transactions`、env flag 缓存（`#[cfg(not(test))]` OnceLock 模式） |
| `task_state_aggregate.rs` | `sync_task_state_aggregate` — 合并 goal+rfv+evidence 写入 TASK_STATE.json |
| `task_ledger.rs` | `append_transaction` — 幂等 JSONL 事务写入（flock 保护） |
| `utils/task_write_lock.rs` | `acquire_task_ledger_repo_lock` — repo 级 flock（30s 超时） |

### 9.2 `routing-engine`

**职责**: 路由评估、信号检测、评分、路由决策。

**依赖**: `core-state-utils`(L0) — 仅 L0 依赖。

读取 `skills/SKILL_ROUTING_RUNTIME.json`，通过 trigger_hints 匹配 → 评分 → 选择 skill_path。

### 9.3 `skill-layer`

**职责**: Skill 层基础设施：schema、validation、lifecycle、dependency management。

**依赖**: `core-state`(L4)。

---

## 10. L5 Hook Infrastructure — 事件路由与观测

L5 是 Hook 基础设施层，包含事件路由、宿主扩展、MCP 桥。5 个 crate。

### 10.1 `host-projection`

**职责**: Hook 分派、宿主扩展、MCP stdio 桥、投影安装。

**依赖**: `core-state-utils`(L0) + `http-util`(L0) + `core-state`(L4) + `core-policy`(L0) + `framework-kernel`(L0) + `mcp-tool-registry`(L5)。

**关键模块**:

| 模块 | 职责 |
|------|------|
| `hosts/mod.rs` | 统一事件分派入口 |
| `hosts/stop_dispatch.rs` | 统一 Stop 决策管道 |
| `hosts/event_handlers.rs` | UserPromptSubmit/PostToolUse 处理 |
| `hosts/host_extensions.rs` | 宿主差异点 |
| `hosts/mcp_pre_guard.rs` | PreToolUse 路径保护 |
| `hosts/mcp_stdio_harness/mod.rs` | MCP stdio 桥、全局缓存（SNAPSHOT_CACHE/TASK_VIEW_CACHE OnceLock） |
| `hosts/mcp_stdio_harness/tools.rs` | MCP 工具实现（~1700 行，含 routing_evolution 480 行分析代码） |
| `hosts/file_state_lock.rs` | 跨平台文件锁 |
| `hosts/hook_dispatch.rs` | 工具路由 |
| `hosts/capability_overrides.rs` | CLI args、observation surfaces |
| `host_integration/mod.rs` | 投影操作入口 |
| `host_integration/projection/projection_bootstrap.rs` | 投影引导、task_id 生成（`build_framework_task_id`） |

### 10.2 `runtime-exit-gate`

**职责**: Quality gate RFV 循环。

**依赖**: `core-state`(L4) + `core-state-utils`(L0) + `core-policy`(L0) + `framework-kernel`(L0) + `fr-contracts`(L2) + `fr-exec`(L3) + `host-projection`(L5) + `runtime-core-contracts`(L2)。

### 10.3 `runtime-infra`

**职责**: 运行时初始化、基础 API 门面。

**依赖**: 跨多个 L0-L5 crate（作为基础设施聚合点）。

**公共 API**:

```rust
pub mod env {
    pub fn flag_enabled_default_true(name: &str) -> bool;
    pub fn flag_enabled_default_false(name: &str) -> bool;
    pub fn parse_env_usize(name: &str) -> Option<usize>;
}

pub mod path {
    pub fn is_framework_root(path: &Path) -> bool;
    pub fn resolve_framework_root() -> Option<PathBuf>;
    pub fn resolve_repo_root(arg: Option<&Path>) -> Result<PathBuf, String>;
}

pub mod time { pub fn now_iso() -> String; }
pub mod sync { pub fn file_append_lock() -> &'static Mutex<()>; }

pub mod io {
    pub fn read_json(path: &Path) -> Result<Option<Value>>;
    pub fn write_json(path: &Path, value: &Value) -> Result<()>;
    pub fn read_stdin_limited() -> Result<String, String>;
    pub fn atomic_write(path: &Path, content: &str) -> Result<()>;
}

pub mod compute {
    pub fn exponential_backoff(attempt: u32, base_secs: f64, multiplier: f64) -> f64;
    pub fn file_sha256_hex(path: &Path) -> Result<String, String>;
}
```

### 10.4 `mcp-tool-registry`

**职责**: 统一 MCP 工具注册表：discovery、routing、search。

**依赖**: `core-state-utils`(L0) — 仅 L0 依赖。

### 10.5 `runtime-core-contracts`

**职责**: Hook 事件路由规则、观测埋点、出站保护、URL 守卫。

**注意**: 此 crate 位于 L2（定义合约类型）但部分模块被 L5 消费。

---

## 11. L6 Orchestration — 编排与自动化闭环

L6 是编排层。3 个 crate。

### 11.1 `loop-engine`

**职责**: 可选自动化增强层（仅 `loop-auto` profile task 生效）。RFV 闭环：discovery → dispatch → verify。

**依赖**: `core-state-utils`(L0) + `framework-kernel`(L0) + `fr-contracts`(L2) + `core-state`(L4)。

**状态机**:

```
PENDING → PREFLIGHT → DISPATCH → RUNNING → VERIFYING → COMPLETED
                                              ↘ ESCALATED
                                                  ↓ (research escalation auto-resume)
                                              递归重入 run_loop（depth_remaining--）
```

**核心能力**:
- **RFV 收敛检测**: 读取 `RFV_LOOP_STATE.json`，检查 review→fix→verify 是否收敛
- **Goal 驱动**: PENDING→COMPLETED 状态转换
- **中断处理**: 自循环活性锁 + 超时
- **Checkpoint**: session 恢复快照

**设计约束**: loop engine 不关心宿主差异、不操作 closeout 记录、不读取宿主环境变量。它通过 L0 函数指针获取外部状态，通过 `core-state` 管理内部状态。

### 11.2 `session-supervisor`

**职责**: Session 监督器：多 Agent + RFV 闭环。

**依赖**: `core-state-utils`(L0) + `core-policy`(L0) + `core-state`(L4) + `framework-kernel`(L0) + `runtime-storage`(L1)。

### 11.3 `framework-extra`

**职责**: 编排控制面：route_manifest、跨层聚合、session artifacts。

**依赖**: 广泛（core-state, core-policy, framework-kernel, fr-utils, fr-contracts, fr-exec, framework-runtime-hooks, trace-runtime, runtime-core-contracts, runtime-storage, routing-engine, host-projection, runtime-infra）。

**关键模块**: `session_artifacts.rs`（session 产物管理、`build_task_id`）、`alias.rs`（别名构建）。

---

## 12. L7 Bridge / Dispatch — 平台聚合与分发

L7 是最顶层，负责平台聚合和 stdio 分发。2 个 crate。

### 12.1 `runtime-core`

**职责**: 平台聚合 + stdio 分发 + 上下文工程。~6,000 行核心代码。

**依赖**: 广泛（几乎所有 L0-L6 crate）。

**核心模块**:

| 模块 | 行数 | 职责 |
|------|------|------|
| `lib.rs` | ~400 | 引导、注册、re-export |
| `eval_route.rs` | ~450 | 路由评估与分发 |
| `stdio_dispatch.rs` | ~587 | 编排中枢（深度依赖 37+ 个 `crate::` 引用，不宜机械迁移） |
| `hook_timing.rs` | ~130 | Hook 计时 |
| `task_command.rs` | ~130 | 任务命令入站 |

**设计**: runtime-core 不包含业务逻辑，仅聚合其他子 crate 并注册到 L0。stdio_dispatch 因深耦合内部模块，留作架构债务长期分解。

### 12.2 `router-rs`

**职责**: CLI 入口二进制（router-rs-cli），宿主 hook/agent/subagent dispatch 总入口。

**依赖**: core-state, core-state-utils, framework-kernel, core-policy, routing-engine, skill-layer, runtime-core, host-projection, framework-extra, loop-engine, browser-mcp, research-harness(optional)。

**dispatch 模式**: `register_hook_dispatchers()` / `find_hook_dispatch()` 注册表模式（非硬编码 dispatch table）。

---

## 13. Feature Layer — 可插拔研究能力

### 13.1 `research-harness`

**职责**: 科研 Harness：paper revision loop、literature search、claims management、AIGC detection。

**编译**: 通过 feature-gate 编译期可选，无运行时 crate 依赖。

**宿主隔离**:
- env var 名称映射委托给 L0 的 `paper_prose_env_var()` / `paper_adversarial_env_var()`
- 宿主 id 通过函数指针参数接收，不做分支逻辑
- 数据库路径迁移到 `~/.router-rs/`

**L4 不含 Research 领域逻辑**:

以下内容属于 L5，**不得出现在 L4 的任何子 crate** 中：
- `ResearchMode` 枚举 (Quick/Deep)
- `infer_research_mode()` 分类器
- `external_research_phrase_signals_deep()`
- `payload_text_signals_deep_research()`
- `normalize_research_mode_token()`

架构规则：L4 不感知 `ResearchMode` 的具体含义。如需 research 分类决策，应通过 L0 函数指针注册 `fn(text: &str) -> Option<ResearchMode>` 回调，由 L5 在启用时注册。

---

## 14. 基础设施层 API 参考

基础设施层是跨所有 crate 共享的**唯一实现**集合。每项功能只应有一个定义。

### 14.1 唯一性清单

| 功能 | 唯一位置 | 说明 |
|------|---------|------|
| `env_enabled_default_true/false` | `core_policy::env_flags` | 环境标志布尔解析 |
| `repo_roots` | `framework_kernel::repo_roots` | 框架根目录发现 |
| 文件锁 (`flock`) | `host-projection/src/hosts/file_state_lock.rs` | 跨进程文件锁 |
| stdin 受限读取 (4 MiB) | `host-projection::hooks` | 带 UTF-8 校验的 stdin |
| JSON 泛型 I/O | `fr_utils::json_io` | read/write/if-exists |
| 原子写入 | `core_state::utils::atomic_write` | temp+rename+fsync |
| `now_iso()` | `framework_kernel::time` | UTC ISO 8601 时间戳 |
| HTTP 代理 URL 缓存 | `http_util::cached_proxy_url` | 缓存代理环境变量解析 |
| 追加锁 | `OnceLock<Mutex<()>>` | 进程内追加写入串行化 |
| 退避公式 | `exponential_backoff` | 几何退避计算 |

### 14.2 归属规则

判断一项功能是否属于基础设施的标准（三项全满足）：
1. **不依赖 L3+ 业务类型**（不引用 quality_gate、session、task 等）
2. **可被 2 个以上 crate 独立使用**（否则应内联到调用者）
3. **语义不因宿主而异**（不包含宿主名、不分支宿主行为）

### 14.3 L5 不得重复实现 L0/L4 已有功能

| L5 不应重复实现 | 应改为调用 |
|----------------|-----------|
| `env_enabled_default_true/false` | `core_policy::env_flags::env_enabled_default_true/false` |
| `now_iso()` 时间戳 | `framework_kernel::time::now_iso` |
| JSON 泛型 I/O | `fr_utils::json_io::*` |

---

## 15. 产物目录

```
core/
├── core-state/               L4  State Management
├── core-state-utils/         L0  IO/path/JSONL 原语（从 core-state 提取）
├── core-state-types/         L2  纯类型定义（从 core-state 提取）
├── core-policy/              L0  Kernel (策略)
├── framework-kernel/         L0  Kernel (time/tokenizer/telemetry/json_value)
├── framework-runtime-hooks/  L0  fn-pointer 注册表 (OnceLock)
├── telemetry-types/          L0  Kernel (类型)
├── http-util/                L0  Kernel (HTTP 工具)
│
├── host-projection/          L5  Hook 分派 + 宿主扩展
├── runtime-exit-gate/        L5  Quality Gate RFV
├── runtime-infra/            L5  运行时初始化
├── mcp-tool-registry/        L5  MCP 工具注册表
│
├── fr-utils/                 L1  IO 工具、常量、类型
├── fr-contracts/             L2  合约/守卫
├── fr-exec/                  L3  执行引擎
├── framework-runtime/        L3  facade（向后兼容 re-export）
├── runtime-core-contracts/   L2  Contracts (合约/守卫)
├── runtime-storage/          L1  IO & Persistence
├── trace-runtime/            L1  IO & Persistence (Trace)
│
├── routing-engine/           L4  路由评估/评分/决策
├── skill-layer/              L4  Skill 层基础设施
│
├── framework-extra/          L6  Orchestration
├── loop-engine/              L6  Orchestration (RFV)
├── session-supervisor/       L6  Orchestration
│
├── runtime-core/             L7  Bridge (调度/聚合)
├── router-rs/                L7  CLI 入口
│
├── browser-mcp/              L3  浏览器 MCP
├── research-harness/         Feature  (feature-gated, 研究能力)
│
tools/
├── codegraph-rs/             L3  代码图 MCP
├── framework-maint/                运维工具
└── evolution-rs/                   路由演化
```

---

## 16. 运行时层由来

原「L4 运行时平台」中的 crate 经过 2026-06 重构已分解到 L0–L7 八层：

| 原模块 | 旧行数 | 现归属 |
|--------|--------|--------|
| `host_integration/` | 5,707 | L5 host-projection |
| `framework_runtime/` 本地模块 | 6,800 | L6 framework-extra |
| `infrastructure/` (5 文件) | 2,041 | L5 runtime-infra |
| `infrastructure/stdio_transport` | 898 | 并入 runtime-infra |
| `exit_gate/` | 2,906 | L5 runtime-exit-gate |
| `framework_maint/` | 1,789 | tools/framework-maint |
| `cli/` | 1,954 | router-rs (L7) |
| `closeout_enforcement.rs` | 1,227 | L2 fr-contracts |
| `execution_contract.rs` | 1,056 | L2 fr-contracts |
| `runtime_view.rs` | 969 | L3 fr-exec |
| `hooks.rs` | 133 | L0 framework-runtime-hooks |

**框架-runtime 拆分**: `framework-runtime` → `fr-utils`(L1) + `fr-contracts`(L2) + `fr-exec`(L3)，保留为向后兼容 facade。

**core-state 拆分**: `core-state-utils`(L0) 和 `core-state-types`(L2) 从 core-state 提取。core-state 通过 re-export 保持向后兼容。

---

## 17. 已知架构债务

### 17.1 并发安全（P1）

| 编号 | 问题 | 位置 |
|------|------|------|
| D1 | `router_rs_task_ledger_flock_enabled()` 三份拷贝 | core-state-utils、core-state、core-policy |
| D2 | 两套独立 flock 实现（超时策略不同） | core-state/utils vs core-state-utils |
| D3 | 指针读取多层 fallback TOCTOU | pointer_ops.rs:33-148 |
| D4 | `truncate_corrupt_tail` 无锁修改 | jsonl_maintenance.rs:19-92 |
| D5 | `task_ledger.rs` is_file() TOCTOU 窗口 | task_ledger.rs:110-175 |

### 17.2 代码重复（P1）

| 编号 | 问题 | 位置 |
|------|------|------|
| D6 | `safe_slug` 两处实现语义不一致 | json_value.rs vs projection_bootstrap.rs |
| D7 | `build_task_id` / `build_framework_task_id` 完全重复 | projection_bootstrap.rs vs session_artifacts.rs |
| D8 | pointer_ops tasks 数组 upsert 逻辑重复 | write_focus_task_pointer_minimal vs set_task_focus |

### 17.3 函数膨胀（P2）

| 编号 | 问题 | 位置 |
|------|------|------|
| D9 | `classify_runtime_continuity` 单函数 285 行 | fr-exec/runtime_view.rs:229-514 |
| D10 | `routing_evolution` 480 行内联在 tools.rs | host-projection/tools.rs:1214-1695 |
| D11 | `cli_args.rs` 1632 行（测试与代码混杂） | framework-kernel/cli_args.rs |
| D12 | Payload 类型过度 Option 化（21 字段全 Option） | stdio_payload_types.rs:64-135 |

### 17.4 API 一致性（P2）

| 编号 | 问题 | 位置 |
|------|------|------|
| D13 | Guard 同名异义（LockGuard vs RepoLockGuard） | task_ledger.rs vs task_write_lock.rs |
| D14 | Tool domain 缺 `is_tool_stdio_op` 谓词 | stdio_op_registry.rs |
| D15 | `host_home_is_set` 硬编码 match 4 host_id | host_integration/mod.rs:206-215 |
| D16 | `pub use roots::*` 命名空间污染 | host_integration/mod.rs:270 |

### 17.5 其他（P2-P3）

| 编号 | 问题 | 位置 |
|------|------|------|
| D17 | env var 缓存 `#[cfg(not(test))]` 模式重复 4 次 | task_state.rs:25-59 |
| D18 | `current_env_session_id` 全量扫描 env vars | state_manager/mod.rs:88-98 |
| D19 | OnceLock 全局缓存不可在测试间重置 | mcp_stdio_harness/mod.rs:133-136 |
| D20 | `looks_same_identity` substring 匹配误判 | task_state.rs:955-964 |
| D21 | 测试弱断言 `assert!(x.is_empty() \|\| !x.is_empty())` | framework-runtime-hooks/src/lib.rs:217 |
| D22 | `http-util` crate 仅一个函数，可考虑合并 | http-util/src/lib.rs |

---

## 18. 设计决策日志

### D-001: 八层运行时模型

**决策**: crate 按依赖方向严格分为 8 层（L0→L7）。
**理由**: 消除循环依赖，确保每层职责唯一不越界。L5 函数指针注册表是唯一许可的跨层例外。
**来源**: ADR-010 §2, P1-P10 原则。

### D-002: Task Engine 为底层执行引擎

**决策**: Task 是框架的底层执行引擎，不是可选组件。四生命周期（discussx/planx/implementx/verifyx）已彻底退场。
**理由**: 用户层表现为 定义 todo → 执行 todo → 完成 todo。Loop engine 是运行在 Task 之上的可选增强。
**来源**: AGENTS.md §Task Engine, MIGRATION.md。

### D-003: 注册表驱动宿主隔离

**决策**: 所有宿主元数据从 `RUNTIME_REGISTRY.json` 编译期生成。添加新宿主只需编辑注册表。
**理由**: 消除 per-host provider 文件的硬编码，跨层生产代码中零宿主硬编码。
**来源**: ADR-010 §4, Round8 重构。

### D-004: REVIEW_GATE 全 advisory 化

**决策**: REVIEW_GATE Stop 在所有宿主上为 advisory-only（仅 followup_message nudge），不 hard-block。
**理由**: 深度 review 是 skill 层行为，不是 hook 层硬约束。interactive profile 下 suppress review nudge 和 spawn-first。
**来源**: AGENTS.md, MIGRATION.md。

### D-005: framework-runtime 物理拆分

**决策**: framework-runtime → fr-utils(L1) + fr-contracts(L2) + fr-exec(L3)，保留 facade。
**理由**: 消除 L2→L5 逆向依赖，下游 crate 直迁到子 crate。
**来源**: ADR-010 §11.1a。

### D-006: core-state 拆分

**决策**: core-state-utils(L0) 和 core-state-types(L2) 从 core-state 提取。
**理由**: core-state-utils 是零内部依赖的 IO 原语，core-state-types 是零内部依赖的纯类型，两者独立以减少耦合。
**来源**: ADR-010 §11.1b。

### D-007: 基础设施唯一性

**决策**: 每项基础设施功能只应有一个定义。
**理由**: 防止重复实现导致行为不一致和维护负担。
**来源**: ADR-010 §14, 三项全满足归属规则。

### D-008: L4 不含 Research 领域逻辑

**决策**: ResearchMode、infer_research_mode 等研究关键词全清除出 L4。
**理由**: 保持 L4 作为通用状态管理层的纯净性。需要时通过 L0 函数指针注入回调。
**来源**: ADR-010 §13。

### D-009: Session 作用域管理

**决策**: Goal state 仅作用于当前对话 session，不做跨对话持久化。新 session 首次 `goal_state_manage start` 创建新 state。
**理由**: 避免残留状态污染新会话。
**来源**: AGENTS.md §会话级作用域。

### D-010: Confirmed-only 输出

**决策**: Review 类 skill 最终用户可见输出只包含 confirmed findings。rejected 和 hallucinated 不出现。
**理由**: 防止幻觉 findings 误导用户。
**来源**: AGENTS.md §Review 通用协议。

### D-011: Goal complete 不物理删除

**决策**: `complete` 不再物理删除 GOAL_STATE.json，改为标记 `archived: true`。
**理由**: 保留历史状态供审计和恢复，避免不可逆操作。
**来源**: Goal 全周期治理（2026-06-23）。

### D-012: 闭集宿主收敛

**决策**: 权威闭集为 `codex`、`claude`、`cursor`、`opencode`。
**理由**: 简化维护面，统一为 4 个注册表驱动的宿主。
**来源**: MIGRATION.md。
