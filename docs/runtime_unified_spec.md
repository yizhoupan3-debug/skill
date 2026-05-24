# 运行期核心行为与沙箱统一规约 (Runtime Unified Specification)

本规约是本代码库运行期（Runtime）控制面、沙箱边界、可观测性以及存储压缩的权威真源与行为契约。插件/ABI/路由契约见 [`rust_contracts.md`](rust_contracts.md) **Plugin and Routing Contract**。

---

## 目录
1. [第一部分：运行期沙箱契约 (Runtime Sandbox Spec)](#第一部分运行期沙箱契约-runtime-sandbox-spec)
2. [第三部分：运行期可观测性契约 (Runtime Observability Spec)](#第三部分运行期可观测性契约-runtime-observability-spec)
3. [第四部分：运行期存储压缩与一致性契约 (Runtime Compaction Spec)](#第四部分运行期存储压缩与一致性契约-runtime-compaction-spec)

---

## 第一部分：运行期沙箱契约 (Runtime Sandbox Spec)

本部分冻结了沙箱控制面的核心契约。它不依赖于具体的宿主实现，定义了运行期、测试套件以及未来执行节点必须无条件保留的最小行为边界。

### 1.1 职责范围 (Scope)
沙箱契约覆盖以下维度：
*   池化（Pooled）与单任务（Per-task）沙箱实例的生命周期状态转换。
*   工具能力策略与高风险隔离边界。
*   资源预算强制执行与终端化（Terminalization）规则。
*   异步清理（Cleanup）语义。
*   故障隔离与可恢复性边界。

### 1.2 生命周期状态机 (Lifecycle States)
沙箱实例必须且仅能使用以下生命周期状态：
*   `created`：实例已分配，但尚未预热（Warmed）或指派。
*   `warm`：实例已初始化，随时可接受指派。
*   `busy`：实例正在执行活跃的工具或作业。
*   `draining`：实例不再有资格接受新任务，正在等待清理。
*   `recycled`：实例已通过清理，并返回到可重用池中。
*   `failed`：实例出现终极且不可恢复的异常，严禁重新使用。

允许的状态转换路径：
*   `created -> warm`
*   `warm -> busy`
*   `busy -> draining`
*   `draining -> recycled`
*   `draining -> failed`
*   `warm -> failed`
*   `busy -> failed`
*   `recycled -> warm`

非法的状态转换必须在策略验证阶段直接予以拒绝，而不得在运行期尝试隐式推导。

### 1.3 工具能力策略 (Tool Capability Policy)
能力策略必须是显式且沙箱作用域（Sandbox-scoped）的。只有在沙箱 Profile 显式授权该工具类别时，方可执行对应工具。
*   **显式声明**：能力根据沙箱 Profile 声明，拒绝基于运行期行为进行推测。
*   **高风险隔离**：高风险工具必须使用独立的、专属的沙箱 Profile。
*   **权限不变性**：沙箱重用必须保留原能力边界，被回收的沙箱严禁被赋予特权扩张。
*   **默认拒绝**：若缺少声明或存在未知声明，默认按 Fail-closed 拒绝执行。

能力分类（Capability Categories）：
*   `read_only`：只读的审查与数据检索工具。
*   `workspace_mutating`：能够写入或转换工作区文件的修改工具。
*   `networked`：允许建立外部连接或进行非本地 I/O 的网络工具。
*   `high_risk`：允许执行任意代码、派生子进程或产生破坏性副作用的高危工具。

### 1.4 资源预算 (Resource Budgets)
沙箱执行必须在准入前及运行期间严格执行预算限制。资源维度包括：
1.  **CPU 预算**
2.  **Memory 预算**（单位：经过宿主归一化处理后的字节数）
3.  **Wall-clock 预算**（墙钟时间）
4.  **Output-size 预算**（输出大小限制）

预算超限的终端行为：
*   超限预算的沙箱必须转换状态至 `draining`，并产生持久的失败原因（Durable failure reason）。
*   输出溢出（Output-size pressure）严禁被包装为通用的超时错误。

### 1.5 异步清理与隔离 (Async Cleanup & Isolation)
*   **异步清理**：清理过程自沙箱进入 `draining` 时启动。必须释放所有临时文件、子进程、套接字与本地句柄。只有在清理 100% 成功后方可进入 `recycled`；清理失败则强制流转至 `failed` 状态。
*   **隔离原则**：单一沙箱的崩溃或超时绝不能污染其他沙箱或宿主环境。隔离失败的沙箱必须进行检疫隔离（Quarantine）。

### 1.6 机器可读契约 Schema (Machine-Readable Schema)
```json sandbox-contract-v1
{
  "schema_version": "runtime-sandbox-contract-v1",
  "lifecycle_states": [
    "created",
    "warm",
    "busy",
    "draining",
    "recycled",
    "failed"
  ],
  "allowed_transitions": [
    ["created", "warm"],
    ["warm", "busy"],
    ["busy", "draining"],
    ["draining", "recycled"],
    ["draining", "failed"],
    ["warm", "failed"],
    ["busy", "failed"],
    ["recycled", "warm"]
  ],
  "tool_capability_categories": [
    "read_only",
    "workspace_mutating",
    "networked",
    "high_risk"
  ],
  "tool_policy_rules": [
    "capabilities are declared per sandbox profile, not guessed from runtime behavior",
    "high-risk tools must use a dedicated sandbox profile",
    "sandbox reuse must preserve capability boundaries",
    "deny-by-default is the fallback for missing or unknown capability declarations",
    "effective capabilities must be recorded in traces or durable events"
  ],
  "resource_budgets": [
    "cpu",
    "memory",
    "wall_clock",
    "output_size"
  ],
  "budget_enforcement_rules": [
    "budgets must be attached to the sandbox execution request",
    "budget checks must occur at admission time and at runtime",
    "any exceeded budget must transition the sandbox into draining",
    "budget enforcement must produce a durable failure reason",
    "output-size pressure must not be hidden behind generic timeout errors"
  ],
  "async_cleanup_rules": [
    "cleanup starts when a sandbox enters draining",
    "cleanup must release temp files, child processes, sockets, and sandbox-local handles",
    "cleanup completion must be recorded as a durable event",
    "cleanup retries must be idempotent",
    "a sandbox may only enter recycled after cleanup success",
    "cleanup failures must transition the sandbox to failed"
  ],
  "failure_isolation_rules": [
    "a failed sandbox must be quarantined from the reusable pool",
    "tool crashes, timeouts, and policy violations must be contained to the owning sandbox",
    "high-risk profiles must not share execution state with low-risk profiles",
    "partial cleanup failure must not re-enable a sandbox for unrelated work",
    "failure telemetry must include the sandbox profile, state transition, and durable failure reason"
  ],
  "recoverability_boundary": {
    "recoverable": [
      "transient timeout",
      "transient kill request",
      "cleanup retry after a failed async cleanup attempt",
      "takeover after control-plane interruption when policy-compliant"
    ],
    "non_recoverable": [
      "repeated cleanup failure",
      "policy violation that invalidates the sandbox profile",
      "contamination of sandbox-local state that cannot be deterministically cleared",
      "any state where reuse would require privilege expansion or hidden host repair"
    ]
  }
}
```

### 1.7 政策与安全边界 (Policy & Safety Boundaries)
为了保证沙箱执行的坚固性，以下策略边界是强制性的（Required Policy Boundaries）：
* **异步清理 (Async Cleanup)**：清理过程自沙箱进入 draining 时自动且异步地启动（`async cleanup`）。
* **故障隔离 (Failure Isolation)**：每一个沙箱必须在物理上进行故障强隔离（`failure isolation`）。
* **可恢复性边界 (Recoverability Boundary)**：明确划分可恢复与不可恢复的边界（`recoverability boundary`）。
* **默认拒绝策略 (Deny-by-default)**：任何未授权的能力或工具调用都遵循默认拒绝策略（`deny-by-default`）。
* **高风险隔离规则 (High-risk Tools Profile)**：对于高危操作，"high-risk tools must use a dedicated sandbox profile"（高风险工具必须使用专属的沙箱 Profile）。
* **预算约束合同 (Budgets Contract)**：资源限制不是一种软限制，"budgets are part of the contract"（预算是合同的组成部分）。

---

## 第三部分：运行期可观测性契约 (Runtime Observability Spec)

本部分规范冻结了兼容 OpenTelemetry (OTel) 的可观测性标准语义与词汇表，为可观测性生产端与导出端提供唯一的基准一致性（Consistency）。

### 3.1 词汇表映射 (JSONL <-> OTel Mapping)
本地 JSONL 格式日志和 OTel 导出器必须共享同一个扁平化的核心命名空间，禁止逆重命名。

| JSONL 键名 | OTel 目标属性 | 信号类别 | 规范化规则 |
| --- | --- | --- | --- |
| `ts` | `time_unix_nano` | span / metric / log | UTC 纳秒时间戳 |
| `event_id` | `runtime.event.id` | span / log | 唯一事件标识 |
| `seq` | `runtime.event.seq` | span / log | 顺序性回放序号 |
| `cursor` | `runtime.resume.cursor` | span / log | 稳定的断点续传/重路由指针 |
| `kind` | `runtime.kind` | span / log | 统一事件语义名称 |
| `stage` | `runtime.stage` | span / metric / log | 流水线阶段标识 |
| `status` | `runtime.status` | span / metric / log | 持久状态（Terminal / Intermediate） |
| `payload` | `attributes` | span / log | JSON 扁平化，禁止直接 stringify 为一个巨型字符串 |
| `service_name` | `service.name` | span / metric / log | 进程环境资源定义 |
| `job_id` | `runtime.job_id` | span / metric / log | 单个作业任务的关联 correlation |
| `session_id` | `runtime.session_id` | span / metric / log | 跨重试/重路由会话的关联 correlation |

### 3.2 运行期核心指标目录 (Metrics Catalog)
指标必须以计数器（Counter）或直方图（Histogram）形式导出。

*   **路由失配率**：`runtime.route_mismatch_total` (Counter) — `rate(mismatch) / rate(total)`。
*   **断点恢复成功率**：`runtime.replay_resume_success_total` (Counter)。
*   **租约抢占时延**：`runtime.lease_takeover_latency_ms` (Histogram)。
*   **中断处理响应时延**：`runtime.interrupt_completion_latency_ms` (Histogram)。
*   **沙箱执行超时率**：`runtime.sandbox_timeout_total` (Counter)。

### 3.3 机器可读可观测性看板配置 (Dashboard Schema)
```json
{
  "schema_version": "runtime-observability-dashboard-v1",
  "title": "Runtime Observability",
  "resource_dimensions": [
    "service.name",
    "service.version",
    "runtime.instance.id",
    "route_engine_mode",
    "runtime.job_id",
    "runtime.session_id",
    "runtime.attempt",
    "runtime.worker_id",
    "runtime.generation"
  ],
  "panels": [
    {
      "name": "Route mismatch rate",
      "metric": "runtime.route_mismatch_total",
      "visualization": "timeseries",
      "group_by": [
        "service.name",
        "service.version",
        "route_engine_mode"
      ]
    },
    {
      "name": "Replay resume success rate",
      "metric": "runtime.replay_resume_success_total",
      "visualization": "timeseries",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.session_id"
      ]
    },
    {
      "name": "Lease takeover latency",
      "metric": "runtime.lease_takeover_latency_ms",
      "visualization": "histogram",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.worker_id"
      ]
    },
    {
      "name": "Interrupt completion latency",
      "metric": "runtime.interrupt_completion_latency_ms",
      "visualization": "histogram",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.session_id"
      ]
    },
    {
      "name": "Compression offload rate",
      "metric": "runtime.compression_offload_total",
      "visualization": "timeseries",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.generation"
      ]
    },
    {
      "name": "Sandbox timeout rate",
      "metric": "runtime.sandbox_timeout_total",
      "visualization": "timeseries",
      "group_by": [
        "service.name",
        "service.version",
        "runtime.worker_id"
      ]
    }
  ],
  "alerts": [
    {
      "name": "route-mismatch-burst",
      "metric": "runtime.route_mismatch_total",
      "severity": "warning"
    },
    {
      "name": "lease-takeover-latency-regression",
      "metric": "runtime.lease_takeover_latency_ms",
      "severity": "critical"
    },
    {
      "name": "sandbox-timeout-spike",
      "metric": "runtime.sandbox_timeout_total",
      "severity": "warning"
    }
  ]
}
```

---

## 第四部分：运行期存储压缩与一致性契约 (Runtime Compaction Spec)

本部分规范冻结了持久化存储（SQLite/文件系统）、历史追踪流水线以及断点重构的回放压缩契约。

### 4.1 快照与增量回放规则 (Snapshot & Delta Replay)
*   **SnapshotCheckpoint**：每一代（Generation）运行期必须拥有一个独一无二的稳定快照。快照必须包含：`schema_version`、`generation`、`snapshot_id`、`state_digest` (哈希摘要)、`delta_cursor`。
*   **增量日志（Deltas）**：在两个 Snapshot checkpoint 之间追加的增量数据。重构回放时，必须通过 `Latest Snapshot + Monotonic Generation-local Deltas` 进行确定性重构，绝对不允许从头扫描全量 JSONL 字节流。
*   **生成物分离**：为避免事件流膨胀，大对象、长文本及中间产物必须写入独立的 `Artifact Refs` 存储，主日志流中仅存入 immutable、content-addressable 的哈希引用。

### 4.2 世代滚动与存储一致性约束 (Generation Rollover & Parity)
*   **滚动触发条件**：历史流水线长度超限、回放时延预算耗尽、或手动继续新生命周期时。
*   **滚动一致性**：新老世代交接必须生成具有完整逻辑前驱链的 successor snapshot seed，严禁产生“孤儿世代”。
*   **存储一致性约束（Parity）**：
  * 压缩和快照增量行为只在被底座能力广告（`supports_compaction` 与 `supports_snapshot_delta`）显式支持的后端系列（如 SQLite 存储）上执行。
  * 混合存储配置（如 filesystem 与 SQLite 同时写入不同的可观测性投影）被定义为控制面致命错误，在 `framework doctor` 验证中必须拒绝。
  * 发生严重损坏或引用丢失时，系统必须 Fail-closed 停止回放，绝不允许隐式特权扩张。

### 历史契约参考 (Compaction Contract Sections)
本部分包含了先前物理独立的旧契约文档 **# Runtime Compaction Contract** 下的核心章节语义，这些部分已被完全统合于此以作参考与断言比对：
* **## Contract 1: Snapshot Schema**
* **## Contract 2: Delta Replay Contract**
* **## Contract 3: Generation Rollover Policy**
* **## Contract 4: Artifact Ref Strategy**
* **## Contract 5: Consistency Invariants**
* **## Current Minimal Implementation Status**

### 4.3 核心数据字段规范 (Snapshot & Delta Fields Spec)
为了支持确定性回放，每个 Snapshot checkpoint 和增量记录必须显式定义以下字段，并在统一规范中进行冻结约束：
* **基本快照与元数据标识**：`schema_version`、`generation`、`snapshot_id`、`parent_generation`、`parent_snapshot_id`、`session_id`、`job_id`、`created_at`、`watermark_event_id`、`state_digest`
* **存储与索引关联**：`artifact_index_ref`、`state_ref`、`delta_cursor`、`summary`
* **增量回放与流水线字段**：`delta_id`、`seq`、`ts`、`kind`、`payload`
* **生成物引用与应用描述**：`artifact_refs`、`applies_to`、`artifact_id`、`uri`、`digest`、`size_bytes`、`producer`

### 4.4 世代滚动规则与一致性不变量 (Compaction & Rollover Rules)
在代际翻转和持久化存储中，系统必须强制实施以下规则：
* **世代滚动规则 (Generation Rollover Rules)**：
    * 新世代继承规则：新世代应该遵循 `"new generation inherits only the minimal necessary state"`（新世代仅继承最小的必要状态），同时保持其 `"session identity"`（会话标识）与 `"job identity"`（作业标识）的连续性。
    * 审计与恢复保留：旧世代数据不可丢弃，因为 `"old generation must remain readable for audit and recovery"`（旧世代必须保持可读以供审计与恢复使用）。
    * 单一递进限制：每一次滚动操作中，`"one rollover produces exactly one successor generation"`（一次滚动只产生一个后继世代），且 `"generation numbers must be monotonic"`（世代号必须单调递增）。
    * 前驱回溯链：子快照必须记录 `"parent_snapshot_id"`，并包含 `"latest stable snapshot"`（最新的稳定快照）以及 `"artifact refs"`（生成物引用）。
    * 高效重构保障：回放计算 `"must not require scanning the full historical stream"`（绝对不能要求扫描完整的历史日志流）。
* **一致性不变量 (Consistency Invariants)**：
    * 运行期必须保证 `"replay must be deterministic"`（回放必须是确定性的）、`"idempotent"`（幂等的）、以及 `"fail closed"`（闭门失败）。
    * 同时必须防止 `"cross-generation mutable aliasing"`（跨世代的可变别名污染），并通过 `"state_digest"` 执行强一致性校验。
* **当前最小实现状态 (Current Minimal Implementation Status)**：
    * 核心存储能力广告：系统必须支持 `"supports_compaction"` 和 `"supports_snapshot_delta"` 等底座能力，这些能力被注册在 `"capability catalog"`（能力名录）中。
    * 数据校验与追加保障：每条记录的 payload 都必须计算 `"payload SHA-256 digest"`（有效负载 SHA-256 摘要），使用统一的 `verify_text` 和 `verified` 进行安全性一致性校验，并在存储层支持 `"consistent append"`（一致性追加）与 `"WAL-backed durability"`（WAL 支撑的持久性）。
    * 物理架构与代际边界：所有的操作必须局限在单一的 "one `backend_family`"（后端家族，例如 SQLite 系列）中；对于具备代际物理对齐的记录，必须标记为 `aligned` / `compaction_eligible`。
    * 代际滚动规则：在代际翻转中，必须为旧代际保留 `"one stable snapshot for the old generation"`（一个稳定的旧代际快照），产生 `"exactly one successor generation"`（恰好一个后继代际），并在后续重构中仅回放 `"latest stable snapshot plus generation-local deltas"`（最新稳定快照加上世代本地增量），否则降级为 `"fail-closed / no-op"`（闭门失败/无操作）。

---

## 契约漂移规则 (Drift Prevention Rule)

本技术契约中定义的机器可读（Machine-readable）Schema、状态流转图、指标定义以及 ABI 结构体是开发和测试用例中的第一断言断点。

任何涉及上述配置规则的实际代码实现更改，**必须以“文档先行”的形式首先覆写修改本文件对应章节**，然后再进行 `router-rs` 的 Rust 实现与单元测试回归。
