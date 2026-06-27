---
last_verified: "2026-06-27"
---

# 架构规约

## 1. 架构总则

### 1.1 核心原则

| 编号 | 原则 | 说明 |
|------|------|------|
| **P1** | 每层职责唯一，不越界 | Tool 层不含 Skill 逻辑，Runtime 层不含路由决策 |
| **P2** | 宿主差异仅存于 Host 层适配壳 | 宿主名/路径/env_var 映射全部从 `RUNTIME_REGISTRY.json` 编译期生成 |
| **P3** | 依赖方向单向向下 | Lⱼ 可依赖 Lᵢ 当 i ≤ j，禁止下层依赖上层 |
| **P4** | 禁止循环依赖 | DAG 矩阵（§3）编译期强制 |
| **P5** | 跨层通信通过 Kernel 或函数指针 | 不在高层硬编码低层细节 |
| **P6** | 五层 + Kernel 承载实质运行域 | 每层有明确的物理 crate 归属 |
| **P7** | Feature 层可插拔 | feature-gate 编译期可选，不硬编码宿主名 |
| **P8** | Kernel 完全无上层依赖 | 所有 Kernel crate（6 个）零层依赖 |
| **P9** | 基础设施碎片收敛到唯一实现 | 每项功能只应有一个定义（§6） |
| **P10** | 函数指针注册表后备语义为 no-op | 不 panic，不硬阻断 |

### 1.2 Hook 通信模型

函数指针注册表（`framework-kernel::runtime_hooks`）是 Kernel 的一部分，作为跨层通信机制被 L3–L5 消费。注册方向（高层→Kernel）与调用方向（Kernel→高层）**相反**，这是依赖方向合规的关键设计。未注册的 slot 通过 `try_hooks()` 静默走 fallback/no-op。

```
L3–L5 ──register(hooks)──→ Kernel RuntimeCoreHooks [OnceLock]
                                │
Kernel hook 事件到来 ──────→ hooks 方法调用 → L3–L5 注册的回调
```

结构：`TelemetryHooks`(5) + `HostProviderHooks`(4) + 8 独立字段。2026-06 从 17 扁平字段重构。

### 1.3 无固定阶段 Lifecycle

**Task 是底层执行引擎**。用户层：定义 todo → 执行 → 完成，关联 Goal/RFV/Evidence。Lifecycle Profile 控制行为模式：`interactive`（默认，closeout advisory）和 `loop-auto`（自动调度闭环）。

---

## 2. 五层运行时模型总览

| 层 | 职责 | 核心 crate |
|----|------|-----------|
| L5 | Runtime — Goal Engine、状态管理、QG Entry/Route、stdio 分发、session 监督、trace | `runtime-core`, `core-state`, `runtime-infra`, `runtime-storage`, `session-supervisor`, `loop-engine`, `trace-runtime`, `telemetry-emit`, `fr-exec`, `fr-contracts`, `quality-gate`, `runtime-core-contracts`, `fr-utils` |
| L4 | Tool — MCP 工具分派、浏览器工具、科研工具 | `research-harness`, `codegraph-rs`, `browser-mcp` |
| L3 | Skill — 验证技能、QG Checkers、框架技能 | `skill-layer`, all `skills/`, `runtime-core::checkers` (QG Checkers) |
| L2 | Routing — Skill 路由引擎、MCP 工具注册表、路由决策 | `routing-engine`, `routing-core`, `mcp-tool-registry`, `tool-routing-engine`, `eval-route` |
| L1 | Host — Agent 宿主适配层 | `host-projection` |
| Kernel | 跨层 — 纯抽象、策略规则、fn-pointer 注册表、通用工具、错误类型 | `framework-kernel`, `core-policy`, `core-state-utils`, `core-state-types`, `core-errors`, `telemetry-types`, `http-util`, `browser-mcp-dispatch` |

---

## 3. DAG 验证矩阵

```
         K   L1  L2  L3  L4  L5
K        ✓   -   -   -   -   -
L1       ✓   ✓   -   -   -   -
L2       ✓   ✓   ✓   -   -   -
L3       ✓   ✓   ✓   ✓   -   -
L4       ✓   ✓   ✓   ✓   ✓   -
L5       ✓   ✓   ✓   ✓   ✓   ✓
```

- Lⱼ 可依赖 Lᵢ 当 i ≤ j；Kernel 函数指针是唯一许可的跨层例外
- 禁止 L4→L5 编译期依赖 override（通过 Kernel 函数指针间接调用）
- Kernel (K) 无上层依赖 —— 所有 Kernel crate 零 L1-L5 依赖

### All Crates by Layer

| 层 | Crate | 职责 |
|----|-------|------|
| L5 | `runtime-core`(~6000 行) | 平台聚合 + stdio 分发 + 上下文工程 + QG Entry/Route 单例 |
| L5 | `core-state` | Task 状态机与 Goal/RFV（组件表见下） |
| L5 | `runtime-infra` | 运行时初始化、kernel 引导、stdio 传输 |
| L5 | `runtime-storage` | 文件系统/SQLite/内存后端、路径解析 |
| L5 | `session-supervisor` | 多 Agent + RFV 闭环监督 |
| L5 | `loop-engine` | 可选自动化增强（仅 `loop-auto` profile）；RFV 闭环 |
| L5 | `trace-runtime` | Trace 录制与压紧 |
| L5 | `telemetry-emit` | 统一遥测发射原语：structured emit、MetricCounter、tracing+telemetry macros |
| L5 | `fr-exec` | LLM 实时执行、沙箱状态机 |
| L5 | `fr-contracts` | Closeout 验证、执行合约、pre-tool-use 守卫、closeout_enforcement |
| L5 | `quality-gate` | Quality Gate 合约：CheckerRegistry、GateChecker trait、场景/严重度类型、GateVerdict |
| L5 | `runtime-core-contracts` | Hook 事件路由、观测、出站保护；被 L1–L5 crates 消费 |
| L5 | `fr-utils` | JSON/IO 工具、stdio 操作域注册 |
| L5 | `framework-extra` | 编排控制面：doctor、session_artifacts、snapshot |
| L5 | `framework-maint` | L5 framework 维护：inline snapshot、maintenance commands |
| L4 | `research-harness` | 科研 Harness：paper revision loop、literature search、claims mgmt、AIGC detection（§5.1） |
| L4 | `codegraph-rs` | 代码图分析服务 |
| L4 | `browser-mcp` (tools/) | 浏览器自动化 MCP 服务 |
| L3 | `skill-layer` | Skill schema、validation、dependency mgmt |
| L3 | `runtime-core::checkers` | QG Checkers 实现：AdversarialChecker、EvidenceChecker、CorrectnessChecker、SecurityChecker、ScreenshotLayoutChecker、OverflowChecker |
| L2 | `routing-engine` | Skill 路由匹配与评分 |
| L2 | `routing-core` | 路由共享原语（trigram fuzzy 匹配） |
| L2 | `mcp-tool-registry` | 统一 MCP 工具注册表 |
| L2 | `tool-routing-engine` | Tool 路由评分与搜索 |
| L2 | `eval-route` | 路由评估框架：validate routing decisions against expected outcomes |
| L1 | `host-projection` | Hook 分派、MCP stdio 桥、投影安装、宿主适配 |
| Kernel | `framework-kernel` | 时间、根发现、JSON 操作、cli_args、runtime 注册表、runtime_hooks |
| Kernel | `core-policy` | Hook 策略、env_flags、review gate、goal 检测 |
| Kernel | `core-state-utils` | IO/path/JSONL 原语，零内部依赖 |
| Kernel | `core-errors` | 统一错误类型 `FrameworkError`，零内部依赖 |
| Kernel | `core-state-types` | Task/RFV 状态类型定义 |
| Kernel | `telemetry-types` | 纯遥测事件类型 |
| Kernel | `http-util` | HTTP 客户端工厂 |
| Kernel | `browser-mcp-dispatch` | 浏览器 MCP 分派助手（仅依赖 Kernel framework-kernel） |

### `core-state` Task 组件表

核心组件：

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
| `host-projection/` (L1) | 宿主适配层（capability, config, dispatch） |
| `framework-kernel/build.rs`, `host-projection/build.rs` | 编译期生成 |
| Kernel/L2/L3/L4/L5 其他 | **不应出现宿主名** |

### 4.3 闭集宿主

权威闭集：`claude`、`cursor`、`codex`、`opencode`。退役 id 不再使用。新宿主只需编辑 `RUNTIME_REGISTRY.json` + 重编译。

### 4.4 宿主身份传递

```
用户输入 → AGENTS.md → L2 skill routing → L5 session (HostProvider trait)
                                            ↓
                              host_provider_registry() 查找 → dispatcher → dispatch()
```

### 4.5 统一分派架构

4 个宿主不采用独立钩子文件。统一 `host-projection/src/hosts/` 入口（`mod.rs` → `stop_dispatch.rs` / `event_handlers.rs` / `host_extensions.rs` / etc）。宿主差异通过 `HostProvider` trait 注入，不在 handler 中 `match host_id`。

---

## 5. Feature Layer — 可插拔研究能力

### 5.1 `research-harness`

科研 Harness：paper revision loop、literature search、claims management、AIGC detection。feature-gate 编译期可选。env var 名称映射委托给 Kernel 的 `paper_prose_env_var()` / `paper_adversarial_env_var()`，宿主 id 通过函数指针参数接收。

	
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

---

## 7. Quality Gate Route (QG Route)

### 7.1 概述

QG Route 是 v10 引入的统一质量门评估系统，替代分散的旧质量门。核心模型：

```
CheckerRegistry (quality-gate, L5)
    │
    ├── In-place checkers (runtime-core::checkers, L3)
    │   ├── AdversarialChecker (GENERAL)
    │   ├── EvidenceChecker (GENERAL/CODE_REVIEW/RESEARCH)
    │   ├── CorrectnessChecker (CODE_REVIEW)
    │   ├── SecurityChecker (CODE_REVIEW)
    │   ├── ScreenshotLayoutChecker (VISUAL)
    │   ├── OverflowChecker (SLIDES)
    │   ├── ProseQcChecker (RESEARCH)
    │   ├── LiteratureGateChecker (RESEARCH)
    │   ├── StatisticalGateChecker (RESEARCH)
    │   ├── ReproducibilityChecker (RESEARCH)
    │   ├── StructureGateChecker (RESEARCH)
    │   └── FormalGateChecker (RESEARCH)
    │
    └── Extern checkers (research-harness, L4, feature-gate=research)
        ├── Asymptotic (RESEARCH)
        ├── DimensionalConsistency (RESEARCH)
        ├── Inequality (RESEARCH)
        ├── Literature (RESEARCH)
        ├── ProseQCChecker (RESEARCH)
        ├── Reproducibility (RESEARCH)
        ├── StatisticalChecker (RESEARCH)
        ├── Structure (RESEARCH)
        ├── Symbolic (RESEARCH)
        └── SympyBridge (RESEARCH)
```

### 7.2 架构组件

| 组件 | Crate | 层 | 职责 |
|------|-------|-----|------|
| `CheckerRegistry` | `quality-gate` | L5 | 场景分发的 checker 容器、注册与评估 |
| `GateChecker` trait | `quality-gate` | L5 | Checker 接口：`id()`、`scenes()`、`description()`、`check(ctx)` |
| `QG_ROUTE` 单例 | `runtime-core::qg_route` | L5 | OnceLock 持有 CheckerRegistry，注册点与评估入口 |
| `EXTERN_CHECKERS` 钩子 | `runtime-core::qg_route` | L5 | OnceLock&lt;fn&gt; 跨 crate 注册回调，避免循环依赖 |
| `QGEntry` | `runtime-core::qg_entry` | L5 | 两阶段退出门：Stage 1 防欺诈 + Stage 2 质量门 dispatch |

### 7.3 初始化流程

```
router-rs-cli (L5)
    │ set_extern_checkers(research_harness::register_qg_checkers)
    │
    ▼
runtime_core::init_hooks()
    │
    ▼
qg_route::init_qg_route()
    ├── CheckerRegistry::new()
    ├── register_checkers(&mut registry)    ← 12 in-place checkers
    └── EXTERN_CHECKERS.get()(registry)      ← 10 research checkers
```

使用与 `register_paper_hooks`、`set_browser_dispatch` 相同的 `OnceLock&lt;fn&gt;` 钩子模式，避免 runtime-core 与 research-harness 间的循环依赖。

### 7.4 评估与聚合规则

`evaluate_qg_route(scene, ctx)` 返回 `GateVerdict`：

| 严重度 | 门结果 |
|--------|--------|
| P0 / A / B (Critical/High) | ❌ Gate 失败（硬阻断） |
| Warning / C (Medium/Low/Info) | ✅ Advisory 仅 |
| 无违规 | ✅ 通过 |
| QG_ROUTE 未初始化 | ✅ 通过（fallback no-op，符合 P10） |
| 空 registry | ✅ 通过（退化行为） |

归属规则（全满足）：不依赖 L4+ 业务类型、可被 ≥2 crate 独立使用、语义不因宿主而异。L4 不得重复实现 Kernel/L3 已有功能。
