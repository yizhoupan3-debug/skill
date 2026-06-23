# ADR-010: 框架架构规范

**状态**: 终版 · **日期**: 2026-06-23 · **最后修订**: 2026-06-23 (§2/§3/§5/§8/§11 修正, Phase 1.1+2.1+2.2 落地)

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
         └── research-harness   文献检索、审稿、AIGC、LaTeX (feature-gated via router-rs)

L4      Runtime Platform (运行时平台)
         ├── runtime-core           平台聚合 + 上下文工程
         ├── framework-runtime      退出门控 + closeout
         ├── framework-extra        编排控制面（从 runtime-core 提取）
         ├── loop-engine            RFV 闭环（Goal/进度/RFV 收敛）
         ├── runtime-exit-gate      质量门控（从 runtime-core 提取）
         ├── runtime-infra          运行时基础设施（env/path/io/sync/http）
         ├── runtime-storage        SQLite 持久化
         ├── session-supervisor     Session 监督器
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
         └── test_helpers.rs

B0      Foundation Layer (基础库)
         ├── core-state              状态管理
         ├── core-policy             Hook 策略 + Review 守卫
         ├── framework-kernel        框架内核 (repo_roots, runtime_registry)
         ├── telemetry-types         遥测类型
         └── http-util               HTTP 客户端工厂 (唯一源)
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
| B0/L1/L3/L4/L5 其他 | ❌ 不应出现宿主名 | ✅ 已验证干净 |

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
| `infrastructure/` (5 文件零依赖) | 2,041 | → L4 `runtime-infra` |
| `infrastructure/stdio_transport` | 898 | → 函数参数化解耦 cli 引用后并入 runtime-infra |
| `exit_gate/` | 2,906 | → `runtime-exit-gate` crate（依赖 router_env_flags+resolve_repo_root） |
| `framework_maint/` | 1,789 | → `tools/framework-maint`（待 host_integration+cli 提取后） |
| `cli/` | 1,954 | → router-rs（依赖所有模块） |
| **核心保留** | **~6,000** | lib.rs + eval_route + hook_timing + task_command + browser_dispatch |

**提取顺序**:
**提取依赖**: host_integration → L0 + infrastructure(5文件) → L4 runtime-infra → exit_gate/framework_maint → framework_runtime/ → cli → router-rs

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
├── runtime-infra/            L4  运行时基础设施 (env/path/io/sync/http)
├── session-supervisor/       L4  Session 监督器
├── trace-runtime/            L4  运行时追踪
├── runtime-core-contracts/   L4  契约
│
├── research-harness/         L5  科研 Feature (feature-gated, router-rs research 特性)
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
| **运行时基础设施** | `infrastructure/` 中 5 个零依赖文件 | 2,041 | `runtime-infra` (L4) |
| **stdio_transport** | 对 cli 有 1 处引用，需通过函数指针解耦 | 898 | `runtime-infra` (L4) |
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
runtime-infra (4,000 行)        ← 基础设施（L4 级）
```

所有沟通路径：
- `loop-engine` 不直接操作 closeout，通过 L0 函数指针委托给 `framework-runtime`
- `framework-extra` 不访问宿主特有状态，通过 `HostProvider` trait 交互
- `runtime-core` 不包含业务逻辑，仅聚合其他子 crate 并注册到 L0

---

## 11. 验收标准

### 11.1 B0 基础层

- [x] 所有 5 个纯 B0 crate（core-state, core-policy, framework-kernel, telemetry-types, http-util）存在且 Cargo.toml 合规
- [x] 纯 B0 crate 不依赖 L 层 crate（host-projection, routing-engine, runtime-core, framework-runtime 等）
- [x] `runtime-infra` 标记为 L4 依赖层（Cargo.toml 已标注 `description = "L4 运行时基础设施"`；其实际依赖包含 L0/L1/L4 crate，归类为 L4 而非 B0）

### 11.2 DAG 依赖方向

- [x] L0→L4/L5 禁止：host-projection 不依赖 L4/L5 crate
- [x] L1/L3→L4/L5 禁止：routing-engine 不依赖 L4/L5；~~browser-mcp→runtime-core~~ **已修复**（Phase 1.1, 2026-06-23，改为 `runtime-core-contracts`）
- [x] L4→L5 应为 feature-gated：~~runtime-core→research-harness~~ **已修复**（Phase 1.1, 2026-06-23）
- [x] B0→L 层禁止：全部 B0 crate 通过
- [x] **L0→L1 DAG 违规已修复**（Phase 7, 2026-06-23）：host-projection (L0) 不再依赖 `routing-engine` (L1)。5 个路由函数通过 L4 `runtime-core` 的 fn ptr 注册解耦。`routing-engine` 已从 `host-projection/Cargo.toml` 移除。

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
- [x] 六层图（§2）与实际 crate 列表一致
