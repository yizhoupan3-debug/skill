# ADR-010: 框架架构规范

**状态**: 终版 · **日期**: 2026-06-23 · **最后修订**: 2026-06-23 (§2 八层模型重写, 余节适应修订)

---

## 目录

1. [架构总则](#1-架构总则)
2. [八层运行时模型](#2-八层运行时模型)
3. [依赖方向 DAG](#3-依赖方向-dag)
4. [宿主隔离契约](#4-宿主隔离契约)
5. [运行时层映射](#5-运行时层映射)
6. [运行时层—由来](#6-运行时层由来)
7. [Feature 层](#7-feature-层)
8. [产物目录](#8-产物目录)
9. [基础设施层](#9-基础设施层)
10. [Runtime-Core 内部结构](#10-runtime-core-内部结构)
11. [验收标准](#11-验收标准)

---

## 1. 架构总则

### 1.1 核心原则

```
P1. 每层职责唯一，不越界
P2. 宿主差异仅存于 L0 适配壳
P3. 依赖方向单向向下（Lⱼ → Lᵢ 当 i <= j，L0 为最底层）
P4. 禁止循环依赖
P5. 跨层通信通过共享类型（L0）或函数指针（L5 注册表），不在高层硬编码低层细节
P6. L0–L7 运行时层承载实质运行域（Kernel/IO/Contracts/Execution/State/Hook/Orchestration/Bridge）
P7. Feature 层可插拔（feature-gate），不硬编码宿主名或环境变量
P8. L0 完全无上层依赖
P9. 基础设施碎片必须收敛到唯一实现（§10）
P10. 函数指针注册表的后备语义为空操作（no-op），而非 panic 或硬阻断
```

### 1.2 Hook 通信模型

函数指针注册表（`host-projection/src/hooks.rs`）是 L0 的一部分，作为跨层通信机制被 L4 消费。

```
L4 ──register_*()──→ L0 hooks.rs [OnceLock 注册表]
                         │
L0 hook 事件到来 ──→ hook_dispatch.rs → 代理函数 → L4 注册的回调
                         │
                     infra/ env/  解析环境标志
                     infra/ json_io/  序列化
                     infra/ stdin_reader/  读取 stdin
```

两个关键性质：
- 注册方向（L4→L0）与调用方向（L0→L4）相反，这是依赖方向合规的关键
- OnceLock 未注册的 slot 静默返回 no-op，不 panic 不硬阻断

---

## 2. 八层运行时模型

运行时 crate 按依赖方向严格分为 8 层（L0→L7），上层可依赖下层，禁止下层依赖上层。

```
L7      Bridge / Dispatch         runtime-core                stdio 分发、聚合 facade

L6      Orchestration              session-supervisor,        多 Agent + RFV 闭环
                                    loop-engine,
                                    framework-extra

L5      Hook Infrastructure        host-projection/hooks,     事件路由、观测埋点、
                                    runtime-exit-gate,          fn-pointer 消费端
                                    runtime-core-contracts/
                                      hook_* + router_rs_obs

L4      State Management           core-state,                Goal/QG/Task 状态机、
                                    routing-engine              step ledger、路由决策

L3      Execution                  fr-exec,                   LLM 执行、沙箱控制、
                                    framework-runtime           运行时视图、环境标志
                                      (facade → fr-exec)

L2      Contracts                  fr-contracts,              验证规则、守卫合约
                                    core-state-types,           纯类型定义（L2 共享）
                                    runtime-core-contracts

L1      IO & Persistence           fr-utils,                  JSON/文件/存储后端、
                                    runtime-storage,            trace 录制、IO 工具
                                    trace-runtime

L0      Kernel (B0)                framework-kernel,          纯抽象、共享类型、
                                    core-policy,                策略规则、时间工具
                                    core-state-utils,           IO/path/JSONL 原语
                                    framework-runtime-hooks,    fn-pointer 注册表 (OnceLock)
                                    telemetry-types,            遥测事件类型
                                    http-util                   HTTP 客户端工厂
```

> **2026-06-23 修订说明**: framework-runtime 已物理拆分为 fr-utils(L1) + fr-contracts(L2) + fr-exec(L3)，
> framework-runtime 保留为向后兼容 facade。core-state-utils(L0) 和 core-state-types(L2) 从 core-state 提取。
> routing-engine 归入 L4。framework-runtime-hooks (L5) 的 OnceLock fn-pointer 注册表是 ADR §1.2 许可的跨层例外。

### 2.1 与用户视角层的对应关系

原文档的「六层模型」是从功能视角出发的垂直分层（Feature→Runtime→Tool→Skill→Routing→Host）。
八层模型是从 crate 依赖视角出发的水平分层（L0→L7），两者正交共存：

```
用户视角     运行时层            核心 crate
─────────────────────────────────────────
L5 Feature → 依赖 L6→L7        research-harness
L4 Runtime → L3+L4+L5+L6+L7   runtime-core, loop-engine, framework-extra
L3 Tool    → 独立层             browser-mcp, codegraph-rs
L2 Skill   → 纯契约层           skills/<name>/SKILL.md
L1 Routing → L7 (dispatch)     routing-engine, host-projection
L0 Host    → L5 (hook)         host-projection/hosts
L0 (Base) → L0+L1+L2+L4       framework-kernel, core-policy, core-state
```

### 2.2 统一 Hook 分派

不采用 4 个独立宿主钩子文件。统一实现方案：

```
hosts/mod.rs               统一事件分派入口
├── hosts/stop_dispatch.rs     统一 Stop 决策管道（所有宿主共用一个）
├── hosts/event_handlers.rs    统一 UserPromptSubmit/PostToolUse
├── hosts/host_extensions.rs   宿主差异点（TouchState/review_gate/会话密钥）
├── hosts/mcp_pre_guard.rs     PreToolUse 路径保护（统一所有宿主）
├── hosts/hook_state_common.rs 状态 CRUD
├── hosts/file_state_lock.rs   文件锁
└── hosts/hook_dispatch.rs     工具路由
```

宿主差异通过 `HostProvider` trait 注入，不在 hook handler 中做 `match host_id`。

### 2.2 Loop Engine — RFV 闭环

`loop-engine` 不包含 discuss/plan/implement 阶段。它的状态机：

```
PENDING → PREFLIGHT → DISPATCH → RUNNING → VERIFYING → COMPLETED
                                              ↘ ESCALATED
```

核心能力：
- **RFV 收敛检测**: 读取 `RFV_LOOP_STATE.json`，检查 review→fix→verify 是否收敛
- **Goal 驱动**: PENDING→COMPLETED 状态转换
- **中断处理**: 自循环活性锁 + 超时
- **Checkpoint**: session 恢复快照

loop-engine 不关心宿主差异、不操作 closeout 记录、不读取宿主环境变量。
它通过 L0 函数指针获取外部状态，通过 `core-state` 管理内部状态。

---

## 3. DAG 验证

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
- 禁止 L4→L7 编译期依赖 override（通过 L5 函数指针间接调用）

---

## 4. 宿主隔离契约

宿主逻辑 = 硬编码宿主名 / `ROUTER_RS_{HOST}_*` 环境变量 / 宿主特有分支。

### 4.1 注册表驱动架构（Round8+ 完整实施）

所有宿主元数据从 `configs/framework/RUNTIME_REGISTRY.json` **编译期生成**：

| 生成目标 | 源字段 | 产物 |
|---------|--------|------|
| `framework-kernel/build.rs` | `host_targets.metadata.*` (全部) | `generated_host_tables.rs`: `ALL_HOST_IDS`, `host_private_config_dir()`, `review_gate_disable_env()`, `paper_prose_env()`, `paper_adversarial_env()`, `settings_guarded_paths()`, `generated_entrypoint_paths()`, `hook_state_unreadable_tag()`, `session_namespace_env()`, `is_ephemeral_task_id()`, `host_home_dirs()`, `ALL_KNOWN_HOST_DIRS`, `EPHEMERAL_PATH_PATTERNS`, `EPHEMERAL_TASK_PREFIXES` |
| `host-projection/build.rs` | `host_targets.supported`, `host_targets.host_providers`, `host_targets.metadata.*` | `generated_host_providers.rs`: provider struct 定义 + `HostLifecycle`/`HostTelemetry`/`HostProvider` trait impl（全部从注册表生成） |
| CLI hook/agent dispatch | `host_provider_registry()` | `register_hook_dispatchers()` / `register_agent_dispatchers()` 注册表模式 |

### 4.2 允许宿主知识的位置

| 位置 | 说明 | 状态 |
|------|------|------|
| `RUNTIME_REGISTRY.json` | 唯一真相源 | ✅ 所有宿主元数据 |
| `host-projection/` (L0) | 宿主适配层 | ✅ `capability_overrides.rs` (CLI args, observation surfaces), `config.rs`, `dispatch.rs` |
| `host-projection/host_integration/` | 投影操作 | ✅ 已注册表驱动 |
| `framework-kernel/build.rs` | 编译期生成 | ✅ 生成表格 + 函数 |
| `host-projection/build.rs` | 编译期生成 | ✅ 生成 provider struct + trait impl |
| L0/L1/L2/L4 其他 | ❌ 不应出现宿主名 | ✅ 已验证干净 |

### 4.3 宿主身份传递路径

```
用户输入 → AGENTS.md → L1 skill routing → L4 session (通过 HostProvider trait)
                                            ↓
                              host_provider_registry() 查找
                                            ↓
                              provider.dispatcher() → HostHookDispatcher::dispatch()
```

### 4.4 添加新宿主清单

编辑 `RUNTIME_REGISTRY.json` 的唯一字段：
1. `host_targets.supported` 添加 host_id
2. `host_targets.metadata.<host_id>` 添加所有 20+ 个字段
3. `host_targets.host_providers.<host_id>` 添加 Rust 模块路径
4. `all_known_host_dirs` 添加目录
5. 重编译 → 所有代码自动生成

---

## 5. 运行时层映射

当前 crate 在八层模型中的归属：

| 八层 | crate | 职责 |
|------|-------|------|
| **L0** Kernel | `core-policy` | Hook 策略、Review 守卫、env_flags |
| | `framework-kernel` | 时间工具、telemetry trait、tokenizer trait、repo_roots、json_value |
| | `core-state-utils` | IO/path/JSONL 原语（atomic_write, path_guard, json_io, task_write_lock） |
| | `framework-runtime-hooks` | fn-pointer 注册表 (OnceLock)，跨层通信中枢 |
| | `telemetry-types` | 遥测事件类型 |
| | `http-util` | HTTP 客户端工厂 |
| **L1** IO & Persistence | `fr-utils` | JSON Value 提取、IO 工具、常量、类型、env_flags |
| | `runtime-storage` | 文件系统/SQLite/内存后端、路径解析 |
| | `trace-runtime` | Trace 录制、压紧 |
| **L2** Contracts | `fr-contracts` | Closeout 验证、执行合约、工具守卫 |
| | `core-state-types` | 纯类型定义（task_state_types, exit_gate_types, goal_prediction） |
| | `runtime-core-contracts` | hook 事件路由、观测规则、出站保护、URL 守卫 |
| **L3** Execution | `fr-exec` | LLM 实时执行、沙箱状态机、运行时视图、环境标志、trace I/O |
| | `framework-runtime` | L3 facade（向后兼容 re-export） |
| **L4** State | `core-state` | Goal/QG/Task 状态机、step_ledger、exit gates |
| | `routing-engine` | 路由评估、信号检测、评分、路由决策 |
| **L5** Hook | `host-projection` | Hook 分派、宿主扩展、MCP stdio 桥（依赖 core-state L4） |
| | `runtime-exit-gate` | Quality gate RFV 循环 |
| | `runtime-core-contracts/hook_*` | 事件路由规则、观测埋点 |
| **L6** Orchestration | `loop-engine` | RFV 闭环 |
| | `session-supervisor` | Session 监督器 |
| | `framework-extra` | 编排控制面 |
| **L7** Bridge | `runtime-core` | 平台聚合 + stdio 分发 + 上下文工程 |
| | `runtime-infra` | 运行时初始化、stdio 传输 |

> **说明**：framework-runtime 已物理拆分为 fr-utils(L1) + fr-contracts(L2) + fr-exec(L3)，
> 保留为向后兼容 facade。core-state-utils(L0) 和 core-state-types(L2) 从 core-state 提取。
> routing-engine 归入 L4。framework-runtime-hooks (L5) 的 OnceLock fn-pointer 注册表是 §1.2 许可的跨层例外。

---

## 6. 运行时层—由来

原「L4 运行时平台」中的 crate 经过 2026-06 重构已分解到 L0–L7 八层：

| 原模块 | 旧行数 | 现归属 |
|--------|--------|--------|
| `host_integration/` | 5,707 | L5 host-projection |
| `framework_runtime/` 本地模块 | 6,800 | → 新 crate `framework-extra`（L6 编排层） |
| `infrastructure/` (5 文件) | 2,041 | L5 `runtime-infra` |
| `infrastructure/stdio_transport` | 898 | 并入 runtime-infra |
| `exit_gate/` | 2,906 | L5 `runtime-exit-gate` |
| `framework_maint/` | 1,789 | → `tools/framework-maint`（运维工具） |
| `cli/` | 1,954 | → router-rs（入口 CLI） |
| `closeout_enforcement.rs` | 1,227 | → `closeout/` 子目录（L2 Contracts） |
| `execution_contract.rs` | 1,056 | → `contracts/` 子目录（L2 Contracts） |
| `runtime_view.rs` | 969 | L3 Execution |
| `hooks.rs` | 133 | → L5 `framework-runtime-hooks` 独立 crate |

**重构受益**:
- 消除 L2→L5 逆向依赖 (`router_rs_obs` 移出 contracts)
- `json_value` 16 函数统一至 L0 framework-kernel
- `quality_gate` 超 1800 行子目录拆分 + `framework_quality_gate` 三→一统一
- `runtime-infra/router_env_flags` 不必要门面删除
- `runtime-core/router_env_flags` 直连 framework-runtime

---

## 7. Feature 层

`research-harness` 通过 feature-gate 编译期可选，无运行时 crate 依赖。

### 7.2 宿主隔离

- env var 名称映射委托给 L0 的 `paper_prose_env_var()` / `paper_adversarial_env_var()`（从 RUNTIME_REGISTRY.json 生成）
- 宿主 id 通过函数指针参数接收，不做分支逻辑
- L5 不包含宿主特定路径（`.claude/`, `.cursor/` 等）——数据库路径已迁移为 `~/.router-rs/`（Round8）

### 7.3 不重复实现 L0/L4 已有功能

L5 不得自行实现以下基础设施——直接调用 L0/L4 的统一版本：

| L5 不应重复实现 | 应改为调用 |
|----------------|-----------|
| `env_enabled_default_true/false` | `core_policy::env_flags::env_enabled_default_true/false` |
| `now_iso()` 时间戳 | `loop_engine::state::now_iso` |
| JSON 泛型 I/O | `framework_runtime::json_io::*` |
| `ROUTER_RS_OPERATOR_INJECT` 检查 | `host_projection::hooks::router_rs_operator_inject_globally_enabled()` |

### 7.4 L4 不得包含 Research 领域逻辑

以下内容属于 L5，**不得出现在 L4 的任何子 crate** 中：

| 被禁内容 | 当前违规位置 |
|---------|-------------|
| `ResearchMode` 枚举 (Quick/Deep) | `framework-runtime/src/live_execute.rs` |
| `infer_research_mode()` 分类器 | 同上，含 14 个 research 关键词 |
| `external_research_phrase_signals_deep()` | 同上 |
| `payload_text_signals_deep_research()` | 同上 |
| `normalize_research_mode_token()` | 同上 |

**架构规则**: L4 不感知 `ResearchMode` 的具体含义。如果 L4 需要 research 分类决策，
应通过 L0 函数指针注册一个 `fn(text: &str) -> Option<ResearchMode>` 回调，
由 L5 在启用时注册。L4 只接收 `Some(mode)` 或 `None`，不包含 research 关键词或分类枚举。

---

## 8. 产物目录

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
│
├── fr-utils/                 L1  IO 工具、常量、类型（从 framework-runtime 提取）
├── fr-contracts/             L2  合约/守卫（从 framework-runtime 提取）
├── fr-exec/                  L3  执行引擎（从 framework-runtime 提取）
├── framework-runtime/        L3  facade（向后兼容 re-export）
├── runtime-core-contracts/   L2  Contracts (合约/守卫)
├── runtime-storage/          L1  IO & Persistence
├── trace-runtime/            L1  IO & Persistence (Trace)
│
├── routing-engine/           L4  路由评估/评分/决策
├── runtime-infra/            L5  Hook 层初始化
├── runtime-core/             L7  Bridge (调度/聚合)
│
├── framework-extra/          L6  Orchestration
├── loop-engine/              L6  Orchestration (RFV)
├── session-supervisor/       L6  Orchestration
│
├── research-harness/         L5  Feature (feature-gated, router-rs research 特性)
│
├── browser-mcp/              L3  浏览器 MCP
├── router-rs/                L7  CLI 入口
│
tools/
├── codegraph-rs/             L3  代码图 MCP
├── framework-maint/            运维工具 (从 runtime-core 提取)
└── evolution-rs/               路由演化
```

---

## 9. 基础设施层

基础设施层是跨所有 crate 共享的**唯一实现**集合。任何功能只应有一个定义：

| 功能 | 唯一位置 | 说明 |
|------|---------|------|
| `env_enabled_default_true/false` | `core_policy::env_flags` | 环境标志布尔解析 |
| `repo_roots` (`is_framework_root`, `resolve_repo_root`) | `framework_kernel::repo_roots` | 框架根目录发现 |
| 文件锁 (`flock`) | `host-projection/src/hosts/file_state_lock.rs` | 跨进程文件锁 |
| stdin 受限读取 (4 MiB) | `host-projection/src/hooks.rs` | 带 UTF-8 校验的 stdin |
| JSON 泛型 I/O | `framework-runtime/src/json_io.rs` | read/write/if-exists |
| 原子写入 (temp+rename+fsync) | `core-state/src/utils/atomic_write.rs` | 崩溃安全的文件写入 |
| `now_iso()` | `framework-kernel`（统一源） | UTC ISO 8601 时间戳 |
| HTTP 代理 URL 缓存 | `http-util`（统一源） | 缓存 `HTTPS_PROXY`/`HTTP_PROXY` 环境变量解析结果（`cached_proxy_url()`） |
| 追加锁 `OnceLock<Mutex<()>>` | 合并到 `runtime-infra::sync::file_append_lock` | 进程内追加写入串行化 |
| 退避公式 | `runtime-infra::compute::exponential_backoff` | 几何退避计算 |

### 9.1 基础设施的归属规则

判断一项功能是否属于基础设施的标准（三项全满足）：
1. **不依赖 L3+ 业务类型**（不引用 quality_gate、session、task 等）
2. **可被 2 个以上 crate 独立使用**（否则应内联到调用者）
3. **语义不因宿主而异**（不包含宿主名、不分支宿主行为）

### 9.2 runtime-infra crate 的公共 API

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

pub mod time {
    pub fn now_iso() -> String;
}

pub mod sync {
    pub fn file_append_lock() -> &'static Mutex<()>;
}

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

pub mod http {
    /// 缓存代理 URL。检查 HTTPS_PROXY → HTTP_PROXY → ALL_PROXY。
    /// 实际实现在独立的 `http-util` crate 中。
}
```

---

## 10. Runtime-Core 内部结构

`runtime-core` 是 L7 Bridge 层的平台聚合入口。它的**最终结构**如下：

### 10.1 核心保留 (~6,000 行)

属于 L4 核心编排职责、不可移动的部分：

| 文件 | 行数 | 职责 |
|------|------|------|
| `lib.rs` | ~400 | 引导、注册、re-export |
| `eval_route.rs` | ~450 | 路由评估与分发 |
| `hook_timing.rs` | ~130 | Hook 计时 |
| `task_command.rs` | ~130 | 任务命令入站 |
| `browser_dispatch_hook.rs` | ~30 | 浏览器 dispatch |
| `review_gate_cli.rs` | ~10 | Review gate CLI |

### 10.2 应迁出到 L0 的部分：`host_integration/`

`host_integration/`（~5,700 行）包含宿主特有的投影操作。它的内在耦合为零——不引用 `runtime-core` 内部的任何 `crate::` 路径，只通过 `framework_kernel` 和 `host_projection` 的外部 API。应整体迁移到 L0 `host-projection`。

### 10.3 应迁出到独立 crate 的部分

| 目标 | 内容 | 行数 | 目标 crate |
|------|------|------|-----------|
| **运行时基础设施** | `infrastructure/` 中 5 个零依赖文件 | 2,041 | `runtime-infra` (L5) |
| **stdio_transport** | 对 cli 有 1 处引用，需通过函数指针解耦 | 898 | `runtime-infra` (L5) |
| **退出质量门控** | `exit_gate/` | 2,906 | `runtime-exit-gate` (L4) |
| **编排控制面** | `framework_runtime/` 本地模块 | 6,800 | `framework-extra` (L4) |
| **CLI 分发** | `cli/` | 1,954 | `router-rs` (L7) |
| **运维工具** | `framework_maint/` | 1,789 | `tools/framework-maint` |

### 10.4 提取后的最终 L7 Bridge 层

```
runtime-core (~6,000 行)        ← 编排核心
framework-runtime (9,620 行)    ← 退出门控 + json_io
loop-engine (2,943 行)          ← RFV 闭环
runtime-storage (5,620 行)      ← SQLite 持久化
trace-runtime (1,103 行)        ← 运行时追踪
runtime-core-contracts (1,531)  ← 契约与类型
framework-extra (6,800 行)      ← 编排控制面（新）
runtime-exit-gate (2,906 行)    ← 质量门控（新）
runtime-infra (4,000 行)        ← 基础设施（L4 级）
```

所有沟通路径：
- `loop-engine` 不直接操作 closeout，通过 L0 函数指针委托给 `framework-runtime`
- `framework-extra` 不访问宿主特有状态，通过 `HostProvider` trait 交互
- `runtime-core` 不包含业务逻辑，仅聚合其他子 crate 并注册到 L0

---

## 11. 验收标准

### 11.1 L0 Kernel

- [x] 所有 L0 crate（core-policy, framework-kernel, core-state-utils, telemetry-types, http-util）存在且 Cargo.toml 合规
- [x] L0 crate 不依赖 L1–L7 crate（runtime-storage, runtime-core, framework-runtime, host-projection 等）
- [x] `runtime-infra` 标记为 L5 启动层

### 11.1a framework-runtime 拆分（2026-06-23）

- [x] fr-utils (L1) 存在：json_value, json_io, types, constants, stdio_op_registry, io_utils, util, env_flags, hooks
- [x] fr-contracts (L2) 存在：closeout_enforcement, execution_contract, pre_tool_use_guard
- [x] fr-exec (L3) 存在：live_execute, sandbox_control, runtime_view, router_env_flags, trace_stream_io, trace_attach, trace_transport, evolution_observer
- [x] framework-runtime 保留为 L3 facade，所有 pub mod re-export 到子 crate
- [x] 下游 crate（runtime-exit-gate, loop-engine, runtime-infra, framework-extra, research-harness）已直迁到子 crate，不再依赖 facade
- [x] runtime-core (L7) 保留 framework-runtime facade 依赖（L7→L3 合法），作为二级 re-export 聚合入口

### 11.1b core-state 拆分（2026-06-23）

- [x] core-state-utils (L0) 存在：atomic_write, path_guard, json_io, read_bounded, task_write_lock, jsonl_maintenance
- [x] core-state-utils 不依赖任何内部 crate（零内部依赖）
- [x] core-state-types (L2) 存在：task_state_types, exit_gate_types, goal_prediction
- [x] core-state-types 仅依赖 serde + serde_json（零内部依赖）
- [x] core-state 通过 re-export 保持向后兼容（core_state::utils::*, core_state::goal_prediction::*, core_state::task_state::*）

### 11.1c routing-engine 分级（2026-06-23）

- [x] routing-engine 归入 L4 State Management
- [x] routing-engine 仅依赖 core-state-utils (L0)（原依赖 core-state → 改为 core-state-utils）

### 11.2 DAG 依赖方向

- [x] L0 crate 无上层依赖：core-state-utils, framework-kernel, core-policy, framework-runtime-hooks, telemetry-types, http-util 均仅依赖同层或外部 crate
- [x] L1/L2 crate 不依赖 L3+：fr-utils(L1), runtime-storage(L1), trace-runtime(L1), fr-contracts(L2), core-state-types(L2), runtime-core-contracts(L2) 合规
- [x] L5 host-projection 依赖 core-state(L4)：**合规**（L5→L4 按 DAG 矩阵允许），已从旧版 L0 错误标注修正
- [x] framework-runtime-hooks 降级为 L0：纯 fn-pointer 注册表 (OnceLock)，零业务逻辑，L0→L0 无违规
- [x] ~~browser-mcp→runtime-core~~ **已修复**（Phase 1.1, 2026-06-23，改为 `runtime-core-contracts`）
- [x] ~~runtime-core→research-harness~~ **已修复**（Phase 1.1, 2026-06-23，改为 feature-gated）
- [x] **host-projection→routing-engine DAG 违规已修复**（Phase 7, 2026-06-23）：5 个路由函数通过 L4 `runtime-core` 的 fn ptr 注册解耦

### 11.3 宿主隔离

- [x] 宿主名映射已完全迁移至 `configs/framework/RUNTIME_REGISTRY.json` 生成：`framework-kernel/build.rs` 在编译时读取注册表，生成 `generated_host_tables.rs`，包含 `host_private_config_dir()`、`review_gate_disable_env()`、`settings_guarded_paths()`、`generated_entrypoint_paths()`、`is_ephemeral_task_id()` 以及 `ALL_KNOWN_HOST_DIRS`、`EPHEMERAL_PATH_PATTERNS` 等常量（Round8, 2026-06-23）。添加新宿主只需编辑注册表 <code>→</code> 重新编译，自动保持同步。
- [x] **per-host provider 文件彻底消除**（Round8, 2026-06-23）：删除 `cursor_provider.rs`、`claude_provider.rs`、`opencode_provider.rs`、`codex_provider.rs`。所有 `HostLifecycle`/`HostTelemetry`/`HostProvider`/`HostCapabilities` 数据（共 ~180 行纯数据）推进 `RUNTIME_REGISTRY.json` 的 `host_targets.metadata`，由 `host-projection/build.rs` 编译期生成完整 provider struct 定义和 trait impl。剩余的逻辑函数（`build_driver_args`、`extract_observation_surfaces`）按能力独立到 `capability_overrides.rs`，内部以 host_id match 分派，消除 per-host 文件命名。
- [x] `schema_drift.rs` 的 cursor 特有逻辑提取到 L0（Phase 5, 2026-06-23：`host-projection/src/hosts/host_extensions/schema_drift.rs`）
- [x] `runtime-core/lib.rs` cursor/codex 宿主扩展注册封装（Round7 #3+#4：通过 `register_host_hooks()` 封装，codex duplicate check 作为标准 L4→L4 fn ptr 注册留在 init 序列中）
- [x] `framework_doctor.rs` `cursor-stop-` 前缀抽象：`is_ephemeral_task_id()` 已迁移至注册表生成（Round7 #8 + Round8, 2026-06-23：前缀从 `RUNTIME_REGISTRY.json` `ephemeral_task_prefixes` 生成）
- [x] **宿主分派 CLI dispatch table 消除**（Round8, 2026-06-23）：`router_command_dispatch.rs` 中的 `dispatch_hook_command` 和 `dispatch_agent_command` 不再使用 `const DISPATCH_TABLE` 硬编码，改为 `register_hook_dispatchers()` / `find_hook_dispatch()` 注册表模式。`codex/` 子模块（`install.rs`, `mod.rs`）和 `host_extensions/install.rs` 已删除，Codex CLI hooks 安装已被通用投影机制取代。
- [x] **命名残差清理**（Round8, 2026-06-23）：`evidence.rs` 中 3 个 codex 私有函数重命名为泛型名；`framework-runtime/hooks.rs` 中 `CodexHookDuplicateCheckFn` → `HookDuplicateCheckFn`；`framework-profile/mod.rs` 中 `build_codex_artifact_bundle` → `build_profile_artifact_bundle`；`stdio_op_registry.rs` 中 `"compile_codex_profile_artifacts"` 别名已移除。
- [x] **上层硬编码宿主路径迁移**（Round8, 2026-06-23）：`hook_policy.rs` 中 `PROTECTED_GENERATED_PATHS` 从硬编码 codex 列表改为 `protected_generated_paths()` 动态遍历 `ALL_HOST_IDS`；`tool_safety_rules.rs` 中 `CROSS_HOST_SURFACES` 从硬编码 `.codex/hooks.json` 改为 `host_home_dirs()` 动态遍历；`worktree_auto_save.rs` 中 `host_config_dir()` match 改为 `host_private_config_dir()` 注册表函数；`dev_exempt.rs` 中豁免列表补全全部宿主目录；`driver.rs` 工作树默认路径从 `.claude/worktrees` 迁移为 `.router-rs/worktrees`；`research-harness/hub.rs` 数据库路径从 `.claude/` 迁移为 `.router-rs/`。
- [x] **`impl_host_config!` 宏内部硬编码消除**（Round8, 2026-06-23）：`hook_state_unreadable_tag` 和 `session_namespace_env` 的 4-宿主 match 替换为 `framework_kernel::runtime_registry::hook_state_unreadable_tag()` 和 `session_namespace_env()` 生成函数。对应字段已加入 `RUNTIME_REGISTRY.json` 的 `host_targets.metadata.*`。
- [x] **桥接函数去重 + 跨层宿主硬编码清除**（Round8, 2026-06-24）：`projection_adapter()` 与 `projection_adapter_for_raw()` 合并；`mcp_host_display_label()` 委托 `host_log_label()`；`canonical_tool_name` 错误消息 fallback 改为 `ALL_HOST_IDS`；`runtime-exit-gate/schema_drift.rs` 中 `snapshot_cursor_hooks_json()` 泛化为 `snapshot_host_hooks_json_for(host_id)`；`router_command_dispatch.rs` 中 hook/agent dispatcher 注册改为 `ALL_HOST_IDS` 动态遍历；`framework_profile/mod.rs` 中 `codex_profile` 兼容遗留从 `"codex"` 硬编码改为 `ALL_HOST_IDS[0]`；`runtime-core/lib.rs` 删除 `register_host_hooks()` 空壳调用；`runtime-core/stdio_dispatch.rs` 中 `build_codex_artifact_bundle` 改为已重命名的 `build_profile_artifact_bundle`。
- [x] **工具层宿主硬编码清除**（Round8, 2026-06-24）：`tools/framework-maint/src/maint.rs` 中 `codex_home_path()`/`cursor_home_path()`/`claude_home_path()` 三个函数合并为泛型 `host_home_path(host_id)`；`verify_cursor_hooks()`/`verify_claude_projection()`/`verify_codex_hooks()`/`verify_opencode_projection_scope()` 中所有 `.cursor/`/`.claude/`/`.codex/`/`.opencode/` 路径改为 `host_private_config_dir(host_id)` 从注册表获取；`print_local_homes()` 改为遍历 `host_home_dirs()` 动态生成；`INSTALL_SCOPES_BY_TOOL` + `projection_install_scopes_for_tool()` 改为 `install_scopes(host_id)` 注册表生成函数；`update_one_shot()` 中 `for tool in ["claude"]` 改为 `ALL_HOST_IDS` 动态遍历。
- [x] **schema_drift 数据模型泛化**（Round8, 2026-06-24）：`SchemaDriftBaseline.cursor_hooks` 字段重命名为 `host_hooks`；`fallback_cursor_hooks_json()` 重命名为 `fallback_host_hooks_json()`；所有 `cursor_hooks` 引用清除。`install_scopes` 字段已加入 `RUNTIME_REGISTRY.json` 和 `RUNTIME_REGISTRY_SCHEMA.json`，build.rs 生成 `install_scopes(host_id)` 函数。
- [x] **跨层生产代码宿主硬编码彻底清除**（Round8, 2026-06-24）：`VerifyCursorHooks`/`VerifyCodexHooks` CLI 子命令合并为 `VerifyHostHooks { host_id }`；`CursorHookCommand` 死代码删除；`maint.rs` 中 `.filter(|t| t != "codex")`、`codex_home_path()`/`cursor_home_path()`/`claude_home_path()` 包装器、`host_homes` HashMap、`verify_host_projection(&fw, "claude")` 硬编码全部替换为 `ALL_HOST_IDS` 动态遍历或注册表函数。当前跨层（非 host-projection）生产代码中**零宿主硬编码**。

### 11.4 Runtime-Core 拆分

- [x] `host_integration/` → L0 host-projection
- [x] `infrastructure/` → runtime-infra
- [x] `exit_gate/` → runtime-exit-gate
- [x] `framework_runtime/` 部分 → framework-extra（`route_manifest_fallback` 已迁移（Phase 4, 2026-06-23）并删除 runtime-core 孤儿 re-export；`stdio_dispatch` 因深耦合 runtime-core 内部 37+ 个 `crate::` 引用，不宜机械迁移——留作架构债务，见下文）
- [x] `cli/` → router-rs
- [x] `framework_maint/` → tools/framework-maint

> **关于 stdio_dispatch：** 该模块（587 行）是 runtime-core 的编排中枢，深度依赖 `goal_drive`、`closeout_enforcement`、`execution_contract`、`route`、`runtime_storage`、`session_supervisor`、`trace_runtime`、`kernel_bootstrap` 等 runtime-core 内部模块。不做机械迁移。长期方案应是逐步分解为更薄的注册式分派器。

### 11.5 基础设施唯一性

- [x] `env_enabled_default_*` — 唯一源 `core-policy::env_flags`
- [x] `repo_roots` — 唯一源 `framework-kernel::repo_roots`
- [x] `now_iso()` — 唯一源 `framework-kernel::time`（7 处 local fn 全部委托到该源）
- [x] `atomic_write` — 唯一源 `core-state::utils::atomic_write`
- [x] `read_stdin_limited` — 唯一源 `host-projection::hooks`

### 11.6 L5 研究隔离

- [x] `ResearchMode`、`infer_research_mode` 等研究关键词已全清除出 L4
- [x] L5 通过 L0 函数指针注册表回注 hook → runtime-core 无 research-harness 编译期依赖

### 11.7 ADR 文档完整性

- [x] 产物目录（§8）与实际一致，含 session-supervisor
- [x] 八层运行时图（§2）与实际 crate 列表一致

### 追加：Goal 全周期治理（2026-06-23）

**背景**：基于全面审计（P1×4、P2×6），重构 Goal 生命周期管理系统。

**关键决策**：
1. `complete` 不再物理删除 GOAL_STATE.json，改为标记 `archived: true`
2. `TASK_POINTERS.json` 读取增加 `tasks[0]` 回退和 `task_registry.json` 回退
3. `completion_gates` 兼容数组格式（旧数据）
4. 新增 `amend` 操作，支持自然语言 scope change
5. 复杂度检测引擎 `goal_auto_detect` 主动检测复杂任务
6. Stop 管线增加磁盘驱动的 `done_when` 比对
7. 模型侧 `has_structured_goal_contract` 扩展为 regex + 复杂度分析双模式
