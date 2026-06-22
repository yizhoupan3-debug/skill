# ADR-010: 框架架构规范

**状态**: 终版 · **日期**: 2026-06-23

---

## 目录

1. [架构总则](#1-架构总则)
2. [六层模型](#2-六层模型)
3. [依赖方向 DAG](#3-依赖方向-dag)
4. [宿主隔离契约](#4-宿主隔离契约)
5. [B0 基础层](#5-b0-基础层)
6. [L4 运行时平台](#6-l4-运行时平台)
7. [L5 Feature 层](#7-l5-feature-层)
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
P3. 依赖方向单向向下（Lⱼ → Lᵢ 当 i < j，B0 为特例）
P4. 禁止循环依赖
P5. 跨层通信通过共享类型（B0）或函数指针（L0 注册表），不在高层硬编码低层细节
P6. L4 运行时平台承载实质运行域（RFV、编排、上下文、持久化、门控），
    不属于宿主特有逻辑或 Feature 领域代码
P7. L5 Feature 层可插拔（feature-gate），不硬编码宿主名或环境变量
P8. B0 基础层完全无 L 层依赖
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

## 2. 六层模型

```
L5      Feature Layer (领域插件层)
         └── research-harness   文献检索、审稿、AIGC、LaTeX

L4      Runtime Platform (运行时平台)
         ├── runtime-core           平台聚合 + 上下文工程
         ├── framework-runtime      退出门控 + closeout
         ├── loop-engine            RFV 闭环（Goal/进度/RFV 收敛）
         ├── runtime-storage        SQLite 持久化
         ├── trace-runtime          运行时追踪
         └── runtime-core-contracts  契约与类型

L3      Tool Layer (工具层)
         ├── browser-mcp            浏览器 MCP server
         └── tools/codegraph-rs     CodeGraph MCP server

L2      Skill Layer (技能契约层)
         └── skills/<name>/SKILL.md

L1      Routing Layer (意图路由层)
         ├── skill routing    routing-engine (L1, serde/regex)
         └── tool routing     host-projection/hosts/hook_dispatch.rs (L0)

L0      Host + Hook Layer (宿主适配层)
         ├── host-projection/
         │   ├── hosts/              统一 Hook 分派 + 宿主扩展
         │   ├── hooks.rs            OnceLock 函数指针注册表
         │   └── mcp_stdio_harness/  MCP stdio 桥
         ├── host_entrypoint_sync.rs
         ├── infra/                  B0 级基础设施（§10）
         └── test_helpers.rs

B0      Foundation Layer (基础库)
         ├── core-state              状态管理
         ├── core-policy             Hook 策略 + Review 守卫
         ├── framework-kernel        框架内核 (repo_roots, runtime_registry)
         ├── telemetry-types         遥测类型
         └── http-util               HTTP 工具
```

### 2.1 统一 Hook 分派

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
         B0  L0  L1  L3  L4  L5
B0       ✓   -   -   -   -   -
L0       ✓   ✓   -   -   -   -
L1       ✓   -   ✓   -   -   -
L3       ✓   -   -   ✓   -   -
L4       ✓   ✓   ✓   ✓   ✓   (fg)
L5       ✓   ✓   -   -   ✓   ✓

(fg) = feature-gated。L4→L5 通信通过 L0 的函数指针间接完成。
```

禁止：
- L0 → L4/L5
- L1/L3 → L4/L5
- B0 → 任何 L 层
- L4 → L5 编译期硬依赖

---

## 4. 宿主隔离契约

宿主逻辑 = 硬编码宿主名 / `ROUTER_RS_{HOST}_*` 环境变量 / 宿主特有分支。

| 允许位置 | 说明 |
|---------|------|
| `host-projection/src/hosts/host_extensions.rs` | 宿主差异扩展 |
| `host-projection/src/hosts/hook_dispatch.rs` | 事件归一化 |
| `framework-kernel/runtime_registry.rs` | 数据驱动映射 |
| 所有 B0/L1/L3/L4/L5 | ❌ 不应出现宿主名 |

宿主身份传递路径：
```
用户输入 → AGENTS.md → L1 skill routing → L4 session (通过 HostProvider trait)
```

---

## 5. B0 基础层

| crate | 职责 | 依赖 |
|-------|------|------|
| core-state | step_ledger, task_state, quality_gate, goal_drive | 无 workspace dep |
| core-policy | Hook 策略、Review 守卫、权限豁免 | core-state, framework-kernel |
| framework-kernel | 路由注册表、遥测、分词器、**repo_roots** | telemetry-types |
| telemetry-types | 遥测事件类型 | 仅 serde |
| http-util | HTTP 工具、**cached_client** | 无 workspace dep |

---

## 6. L4 运行时平台

### 6.1 子域架构

```
                    ┌─────────────────────────────────────────────┐
                    │              L4 Runtime Platform             │
                    │                                              │
 ┌───────────┐      │  ┌──────────────┐  ┌────────────────────┐   │
 │ L0 hooks  │      │  │ loop-engine  │  │ framework-runtime  │   │
 │ (fn ptrs) │◄─────│──│ RFV + Goal   │  │ 退出门控           │   │
 └───────────┘      │  │ Checkpoint   │  │ closeout/证据      │   │
                    │  └──────┬───────┘  │ json_io (唯一实现) │   │
                    │         │          └────────────────────┘   │
                    │         ▼                                    │
                    │  ┌──────────────────┐  ┌────────────────┐   │
                    │  │ runtime-core     │  │ runtime-storage│   │
                    │  │ 平台聚合 (~6K)   │  │ SQLite 持久化  │   │
                    │  │ eval_route       │  └────────────────┘   │
                    │  │ 上下文工程       │                        │
                    │  │ host_integration │  ┌────────────────┐   │
                    │  │ CLI 分发         │  │ trace-runtime  │   │
                    │  └──────────────────┘  │ 运行时追踪      │   │
                    │                        └────────────────┘   │
                    │  ┌──────────────────┐                       │
                    │  │ core-contracts   │  ← 所有子 crate 共享 │
                    │  └──────────────────┘                       │
                    └─────────────────────────────────────────────┘
```

### 6.2 runtime-core 拆分

| 模块 | 行数 | 目标 |
|------|------|------|
| `host_integration/` | 5,707 | → L0 host-projection（零 `crate::` 依赖） |
| `framework_runtime/` 本地模块 | 6,800 | → `framework-extra` 新 crate（70% 引用是 extern crate re-export） |
| `infrastructure/` (5 文件零依赖) | 2,041 | → B0 `runtime-infra` |
| `infrastructure/stdio_transport` | 898 | → 函数参数化解耦 cli 引用后并入 runtime-infra |
| `exit_gate/` | 2,906 | → `runtime-exit-gate` crate（依赖 router_env_flags+resolve_repo_root） |
| `framework_maint/` | 1,789 | → `tools/framework-maint`（待 host_integration+cli 提取后） |
| `cli/` | 1,954 | → router-rs（依赖所有模块） |
| **核心保留** | **~6,000** | lib.rs + eval_route + hook_timing + task_command + browser_dispatch |

**提取顺序**:
**提取依赖**: host_integration → L0 + infrastructure(5文件) → B0 runtime-infra → exit_gate/framework_maint → framework_runtime/ → cli → router-rs

---

## 7. L5 Feature 层

`research-harness`，通过 feature-gate 编译期可选。

### 7.1 插件协议

```toml
research-harness = { path = "../research-harness", optional = true }
[features]
research = ["dep:research-harness"]
```

### 7.2 宿主隔离

- env var 名称映射委托给 L0 的 `paper_prose_env_var()` / `paper_adversarial_env_var()`
- 宿主 id 通过函数指针参数接收，不做分支逻辑

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
├── core-state/               B0  状态管理
├── core-policy/              B0  Hook 策略
├── framework-kernel/         B0  框架内核 + repo_roots (唯一源)
├── telemetry-types/          B0  遥测类型
├── http-util/                B0  HTTP 客户端工厂 (唯一源)
│
├── host-projection/          L0  宿主适配 + 统一 hook + infra/
├── routing-engine/           L1  路由算法
│
├── runtime-core/             L4  平台聚合 (~6K)
├── framework-runtime/        L4  退出门控 + json_io (唯一源)
├── framework-extra/          L4  编排控制面 (从 runtime-core 提取)
├── runtime-exit-gate/        L4  质量门控 (从 runtime-core 提取)
├── loop-engine/              L4  RFV 闭环
├── runtime-storage/          L4  SQLite 持久化
├── runtime-infra/            B0  基础设施 (合并碎片, §10)
├── trace-runtime/            L4  运行时追踪
├── runtime-core-contracts/   L4  契约
│
├── research-harness/         L5  科研 Feature
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
| HTTP 客户端工厂 | `http-util`（统一源） | 缓存 `reqwest::Client` |
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
    pub fn cached_client(timeout: Duration) -> &'static Client;
}
```

---

## 10. Runtime-Core 内部结构

`runtime-core` 是 L4 的平台聚合入口。它的**最终结构**如下：

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
| **运行时基础设施** | `infrastructure/` 中 5 个零依赖文件 | 2,041 | `runtime-infra` (B0) |
| **stdio_transport** | 对 cli 有 1 处引用，需通过函数指针解耦 | 898 | `runtime-infra` (B0) |
| **退出质量门控** | `exit_gate/` | 2,906 | `runtime-exit-gate` (L4) |
| **编排控制面** | `framework_runtime/` 本地模块 | 6,800 | `framework-extra` (L4) |
| **CLI 分发** | `cli/` | 1,954 | `router-rs` (L7) |
| **运维工具** | `framework_maint/` | 1,789 | `tools/framework-maint` |

### 10.4 提取后的最终 L4 平台

```
runtime-core (~6,000 行)        ← 编排核心
framework-runtime (9,620 行)    ← 退出门控 + json_io
loop-engine (2,943 行)          ← RFV 闭环
runtime-storage (5,620 行)      ← SQLite 持久化
trace-runtime (1,103 行)        ← 运行时追踪
runtime-core-contracts (1,531)  ← 契约与类型
framework-extra (6,800 行)      ← 编排控制面（新）
runtime-exit-gate (2,906 行)    ← 质量门控（新）
runtime-infra (4,000 行)        ← 基础设施（B0 级）
```

所有沟通路径：
- `loop-engine` 不直接操作 closeout，通过 L0 函数指针委托给 `framework-runtime`
- `framework-extra` 不访问宿主特有状态，通过 `HostProvider` trait 交互
- `runtime-core` 不包含业务逻辑，仅聚合其他子 crate 并注册到 L0

---

## 11. 验收标准
