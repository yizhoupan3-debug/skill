# 架构规约

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
| **P9** | 基础设施碎片收敛到唯一实现 | 每项功能只应有一个定义（§6） |
| **P10** | 函数指针注册表后备语义为 no-op | 不 panic，不硬阻断 |

### 1.2 Hook 通信模型

函数指针注册表（`framework-runtime-hooks`）是 L0 的一部分，作为跨层通信机制被 L4–L7 消费。注册方向（高层→L0）与调用方向（L0→高层）**相反**，这是依赖方向合规的关键设计。未注册的 slot 通过 `try_hooks()` 静默走 fallback/no-op。

```
L4–L7 ──register(hooks)──→ L0 RuntimeCoreHooks [OnceLock]
                                │
L0 hook 事件到来 ──────────→ hooks 方法调用 → L4–L7 注册的回调
```

结构：`TelemetryHooks`(5) + `HostProviderHooks`(4) + 8 独立字段。2026-06 从 17 扁平字段重构。

### 1.3 无固定阶段 Lifecycle

**Task 是底层执行引擎**。用户层：定义 todo → 执行 → 完成，关联 Goal/RFV/Evidence。Lifecycle Profile 控制行为模式：`interactive`（默认，closeout advisory）和 `loop-auto`（自动调度闭环）。

---

## 2. 八层运行时模型总览

| 层 | 职责 | 核心 crate |
|----|------|-----------|
| L7 | Bridge / Dispatch — stdio 分发、聚合 facade | `runtime-core`, `router-rs` |
| L6 | Orchestration — RFV 闭环、可选自动化 | `loop-engine`, `session-supervisor`, `framework-extra` |
| L5 | Hook Infrastructure — 事件路由、MCP 桥、fn-pointer 消费 | `host-projection`, `runtime-exit-gate`, `runtime-infra`, `mcp-tool-registry`, `runtime-core-contracts` |
| L4 | State Management — Task Engine、路由、skill-layer | `core-state`, `routing-engine`, `skill-layer` |
| L3 | Execution — LLM 实时执行、沙箱 | `fr-exec`, `browser-mcp`, `framework-runtime`(facade) |
| L2 | Contracts — 验证规则、守卫合约、纯类型 | `fr-contracts`, `core-state-types`, `runtime-core-contracts` |
| L1 | IO & Persistence — 存储后端、trace | `fr-utils`, `runtime-storage`, `trace-runtime` |
| L0 | Kernel — 纯抽象、策略规则、fn-pointer 注册表 | `framework-kernel`, `core-policy`, `core-state-utils`, `framework-runtime-hooks`, `telemetry-types`, `http-util` |




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

- Lⱼ 可依赖 Lᵢ 当 i ≤ j；L5 函数指针是唯一许可的跨层例外
- 禁止 L4→L7 编译期依赖 override（通过 L5 函数指针间接调用）

### All Crates by Layer

| 层 | Crate | 职责 |
|----|-------|------|
| L7 | `runtime-core`(~6000 行) | 平台聚合 + stdio 分发 + 上下文工程 |
| L7 | `router-rs` | CLI 入口二进制，宿主 hook/agent dispatch 总入口 |
| L6 | `loop-engine` | 可选自动化增强（仅 `loop-auto` profile）；RFV 闭环 |
| L6 | `session-supervisor` | 多 Agent + RFV 闭环监督 |
| L6 | `framework-extra` | 编排控制面：doctor、session_artifacts、snapshot |
| L5 | `host-projection` | Hook 分派、MCP stdio 桥、投影安装 |
| L5 | `runtime-exit-gate` | Quality gate RFV 循环 |
| L5 | `runtime-infra` | 运行时初始化、基础 API 门面 |
| L5 | `mcp-tool-registry` | 统一 MCP 工具注册表 |
| L5 | `runtime-core-contracts` | L2 定义被 L5 消费 |
| L4 | `core-state` | Task 状态机与 Goal/RFV（组件表见下） |
| L4 | `routing-engine` | Skill 路由匹配与评分 |
| L4 | `skill-layer` | Skill schema、validation、dependency mgmt |
| L3 | `fr-exec` | LLM 实时执行、沙箱状态机 |
| L3 | `browser-mcp` | 浏览器自动化 MCP 服务 |
| L3 | `framework-runtime` | L3 facade（向后兼容 re-export, 已拆分至 fr-*） |
| L2 | `fr-contracts` | Closeout 验证、执行合约、pre-tool-use 守卫 |
| L2 | `core-state-types` | 纯类型定义，零内部依赖 |
| L2 | `runtime-core-contracts` | Hook 事件路由、观测、出站保护 |
| L1 | `fr-utils` | JSON/IO 工具、stdio 操作域注册 |
| L1 | `runtime-storage` | 文件系统/SQLite/内存后端、路径解析 |
| L1 | `trace-runtime` | Trace 录制与压紧 |
| L0 | `framework-kernel` | 时间、根发现、JSON 操作、cli_args、runtime 注册表 |
| L0 | `core-policy` | Hook 策略、env_flags、review gate、goal 检测 |
| L0 | `core-state-utils` | IO/path/JSONL 原语，零内部依赖 |
| L0 | `framework-runtime-hooks` | fn-pointer 注册表 (OnceLock)，跨层通信中枢 |
| L0 | `telemetry-types` | 纯遥测事件类型 |
| L0 | `http-util` | HTTP 客户端工厂 |

### `core-state` Task 组件表

`TaskControlMode：Idle → GoalDrive → QualityGate → Conflict`。核心组件：

| 组件 | 职责 |
|------|------|
| Active/Focus 指针 | 决定当前执行哪个 task |
| GOAL_STATE.json | goal、non-goals、done_when、validation_commands |
| TASK_LEDGER.jsonl | 幂等事务日志（flock 保护）、跨会话连续性 |
| TASK_STATE.json | goal+rfv+evidence 聚合投影 |
| STEP_LEDGER.jsonl | 步骤级追踪 |
| EVIDENCE_INDEX.json | 验证证据记录 |


所有宿主元数据从 `configs/framework/RUNTIME_REGISTRY.json` 编译期生成：

| 生成源 | 生成目标 | 产物 |
|--------|---------|------|
| `host_targets.metadata.*` | `framework-kernel/build.rs` | `generated_host_tables.rs` — `ALL_HOST_IDS`、host config/review gate/session env 等函数 |
| `host_targets.supported`, `host_providers` | `host-projection/build.rs` | `generated_host_providers.rs` — provider struct + trait impl（无手写 provider 文件） |

### 4.2 允许宿主知识的位置

| 位置 | 说明 |
|------|------|
| `RUNTIME_REGISTRY.json` | **唯一真相源** |
| `host-projection/` (L5) | 宿主适配层（capability, config, dispatch） |
| `framework-kernel/build.rs`, `host-projection/build.rs` | 编译期生成 |
| L0/L1/L2/L4 其他 | **不应出现宿主名** |

### 4.3 闭集宿主

权威闭集：`claude`、`cursor`、`codex`、`opencode`。退役 id 不再使用。新宿主只需编辑 `RUNTIME_REGISTRY.json` + 重编译。

### 4.4 宿主身份传递

```
用户输入 → AGENTS.md → L1 skill routing → L4 session (HostProvider trait)
                                            ↓
                              host_provider_registry() 查找 → dispatcher → dispatch()
```

### 4.5 统一分派架构

4 个宿主不采用独立钩子文件。统一 `host-projection/src/hosts/` 入口（`mod.rs` → `stop_dispatch.rs` / `event_handlers.rs` / `host_extensions.rs` / etc）。宿主差异通过 `HostProvider` trait 注入，不在 handler 中 `match host_id`。

---

## 5. Feature Layer — 可插拔研究能力

### 5.1 `research-harness`

科研 Harness：paper revision loop、literature search、claims management、AIGC detection。feature-gate 编译期可选。env var 名称映射委托给 L0 的 `paper_prose_env_var()` / `paper_adversarial_env_var()`，宿主 id 通过函数指针参数接收。

**L4 不含 Research 领域逻辑**：`ResearchMode`、`infer_research_mode()` 等属于 L5，不得出现在 L4 子 crate 中。通过 L0 函数指针注册 `fn(text) -> Option<ResearchMode>` 回调。

---

## 6. 唯一性清单与归属规则

| 功能 | 唯一位置 | 说明 |
|------|---------|------|
| `env_enabled_default_true/false` | `core_policy::env_flags` | 环境标志布尔解析 |
| `repo_roots` | `framework_kernel::repo_roots` | 框架根目录发现 |
| 文件锁 (`flock`) | `host-projection::file_state_lock` | 跨进程文件锁 |
| stdin 受限读取 | `host-projection::hooks` | 带 UTF-8 校验 |
| JSON 泛型 I/O | `fr_utils::json_io` | read/write/if-exists |
| 原子写入 | `core_state::utils::atomic_write` | temp+rename+fsync |
| `now_iso()` | `framework_kernel::time` | UTC ISO 8601 时间戳 |
| HTTP 代理 URL 缓存 | `http_util::cached_proxy_url` | 代理环境变量解析 |
| 退避公式 | `exponential_backoff` | 几何退避计算 |

归属规则（全满足）：不依赖 L3+ 业务类型、可被 ≥2 crate 独立使用、语义不因宿主而异。L5 不得重复实现 L0/L4 已有功能。
