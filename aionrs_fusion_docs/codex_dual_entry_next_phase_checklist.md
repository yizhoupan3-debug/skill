# Codex 双入口下一阶段执行蓝图

> 本文件现在作为兼容视图文档存在：主线口径已经统一为
> `thin projection + Rust contract-first migration`，并从 Codex-only 双入口
> 推广到 CLI-family（`codex cli` / `claude code cli` / `gemini cli`）共享
> contract；而本文只描述 `codex_dual_entry_parity_snapshot` 仍然作为兼容
> 回归视图时的收口方式。

## 1. 下一阶段总目标

下一阶段要把已经收敛的结论继续落成可并行推进的工程蓝图：

- framework core 继续做唯一主控
- `Codex Desktop` 继续做交互宿主
- `codexcli` 继续做 Codex headless 宿主，但不再独占 CLI lane 命名中心
- `cli_common_adapter` 继续统一 CLI-family shared contract
- `codex_common_adapter` 降为 Codex compatibility view
- `cli_family_parity_snapshot` 作为 canonical CLI 回归通道
- `codex_dual_entry_parity_snapshot` 保留为低摩擦兼容回归通道
- `codex_desktop_host_adapter` 只保留 compatibility-only mirror 角色

## 2. 当前阶段完成情况

- [x] 已确定不再继续以 `aionrs` / `AionUI` 为未来主线
- [x] 已确定主控不是 `codexcli`
- [x] `framework_profile` 仍可作为唯一真源
- [x] `cli_common_adapter` / `codex_desktop_adapter` /
  `codex_cli_adapter` / `claude_code_adapter` / `gemini_cli_adapter`
  已是当前正式基线
- [x] `codex_desktop_host_adapter` / `generic_host_adapter` 只保留兼容或 fallback 价值
- [x] Rust compiler lane 已能输出 first-class Codex artifacts 与 parity snapshot

因此下一阶段不再是“是否放弃 aionrs”，而是“如何把 canonical CLI-family
contract 与 Codex dual-entry compatibility view 一起落成 adapter、artifact、
parity 和迁移顺序”。

## 3. 下一阶段波次

## Wave C1: Core Contract Normalization

- [ ] 把旧文档、artifact、测试中的 `aionrs` 主线叙事移除
- [ ] 冻结 Desktop / CLI 共用的 core contract
- [ ] 明确 common adapter 和 host-specific adapter 的边界

**交付物**

- CLI-family contract baseline with Codex compatibility view
- host-neutral field policy
- legacy migration boundary

## Wave C2: Codex Common Adapter Stabilization

- [ ] 继续收口 shared session / artifact / memory / MCP / tool policy emission
- [ ] 清理残留 host-specific duplication
- [ ] 保持 common adapter 作为 Desktop / CLI 共用输入

**交付物**

- common adapter
- shared bridge / contract emission utilities
- host adapter input contract

## Wave C3: Desktop / CLI Host Surface Cleanup

- [x] 保持 `codex_desktop_adapter` 为 canonical desktop entrypoint
- [x] 保持 `codex_cli_adapter` 为 formal headless entrypoint
- [~] 建立双入口 capability discovery
  已落地一刀：CLI-family capability discovery contract 已可对
  `codex cli` / `claude cli` / `gemini cli` 输出稳定 discovery artifact，
  且不把任何一个 CLI host 提升成 framework truth。下一刀再决定是否把
  desktop lane 一并并到同一 discovery 视图。
- [x] 给 `codex_desktop_host_adapter` 明确 alias retirement gate
- [ ] 保持两条宿主 lane 共用一份 outer contract
- [x] Claude hook / settings / subagent compatibility 保持在 thin CLI-host
  projection + parity tests，不进入 `cli_common_adapter` truth

**交付物**

- desktop adapter
- cli adapter
- dual-entry host capability contract

## Wave C4: Parity Snapshot / Regression Lane

- [ ] 继续把剩余文档 / 测试 / 脚本迁到 parity-snapshot-first 叙事
- [ ] 保持 artifact layout baseline
- [ ] 保持双入口 regression checklist

**交付物**

- parity snapshots
- regression baseline
- artifact layout contract

## Wave C5: Rust Contract Emission Lane

这一波属于 `thin projection + Rust contract-first migration` 主线中的 Rust
contract emission lane；这里记录的是 dual-entry compatibility view 的收口边界，
不再把它写成独立的 Codex-only 主线。

- [ ] 保持 Rust 输出 Desktop / CLI 共用 contract fragments
- [ ] 保持 Rust 输出 parity snapshot JSON
- [ ] 继续让 Rust 与 Python emitter 对齐 artifact layout
- [~] 为 execution controller / delegation / supervisor-state 共享 contract artifact
  保持 Rust mirror 与最小 runtime descriptor 接口
  已落地一刀：`router-rs` 与 Python artifact emitter 现在都输出
  `execution_controller_contract` / `delegation_contract` /
  `supervisor_state_contract`；`ExecutionEnvironmentService.health()` 也只在
  control-plane surface 暴露这些 descriptor，不触碰 live path
- [ ] 保持 Rust lane 不直接宣称已完成 live runtime kernel / state machine 改写；
  允许先把 execution entry 收口成 replaceable kernel seam

**交付物**

- Rust parity snapshot emitter
- Rust shared contract emission utilities
- Python / Rust parity tests

## Wave C5.5: Runtime Kernel Control-Plane Lane

这一波不是 runtime rewrite，而是把已经冻结的 runtime contracts 开始落成
真正的控制面。

- [~] 把 execution path 收口成单一 kernel seam，但不伪造成“Rust live kernel
  已完成”
  已落地一刀：`ExecutionEnvironmentService` 现在拥有单一 `ExecutionKernel`
  入口，`runtime.py` 不再内联 dry/live 分支与 Agno run-response serialization。
  已落地第二刀：live execution 现在优先进入 `router-rs --execute-json`，Python
  只保留 dry-run 与 live 失败时的 compatibility fallback。当前更安全的 Rust
  slice 已先把 alias retirement inventory summary 收进 shared contract/parity lane；
  已落地第三刀：live Python fallback 的退休准备状态也已进入一等 shared artifact，
  会外显 primary/fallback authority、控制开关与退休阻塞条件；已落地第四刀：
  `compatibility_live_response_serialization` 的 response shape / invariant 也已
  固定成 shared contract artifact，覆盖 live primary / compatibility fallback /
  dry-run 三种 response surface；
  删除 live fallback 仍属于后续 runtime control-flow lane，不应和 multi-end
  改造并轨推进
- [~] 把 route diff / reporting / rollback 语义继续从文档推进到实现
  已落地一刀：`RouteDiffReport` 的 compare path 已进入 `router-rs`，Python
  不再本地重算 Rust 侧 mismatch vocabulary。已落地第二刀：route mode /
  rollback / primary-authority policy 也已进入 `router-rs`，Python 不再本地
  硬编码 `python_route_required` 与 primary result engine。下一刀收窄为把残余
  result shaping / metadata rehydration 继续往 Rust 收
- [~] 把 durable background job state 从“能持久化”推进到更完整的 run-manager
  语义
  已落地一刀：pending takeover reservation 已进入 durable state contract，
  restart 后不再无声丢失 replacement intent。下一刀是把 lease/cursor cleanup 与
  run-manager recovery 再继续统一
- [~] 建立 resumable stream bridge seam，而不是只靠 ad-hoc polling
  已落地第一刀：trace JSONL 现在提供 replayable `seq` / `cursor` /
  resume-window seam；且 runtime 已有 in-memory `RuntimeEventBridge`
  producer/consumer surface。已落地第二刀：runtime 现在还有 versioned poll
  transport descriptor，host 可先 discover seam 再 subscribe。已落地第三刀：
  runtime 现在还有 `describe_runtime_event_handoff(...)` 与 persisted JSON
  binding artifact，可把 transport descriptor 与 replay/checkpoint anchor 一起
  交给另一个 host/bridge。下一刀是把它推进到更强的 non-filesystem
  remote/host transport boundary，而不是重复再做一套本地桥
- [~] 建立 unified store/checkpointer seam，为 compaction / replay 做准备
  已落地第一刀：filesystem `RuntimeCheckpointer` 已统一 trace path /
  resume manifest / background-state path 描述与写入边界。下一刀是把后端做成
  可替换 family，而不是继续散落在 runtime/service 里手写路径。已落地第二刀：
  durable background state 与 checkpointer 已共享同一 backend-family 抽象，
  但 concrete backend 仍只有 filesystem
- [ ] 让 sandbox lifecycle 更接近 `runtime_sandbox_contract.md`
- [x] 已引入 background `multitask_strategy` 的首个控制面切片
  (`reject` / `interrupt`)
- [x] `scripts/route.py` 已收口成 Rust route/search transport shim，不再保留
  平行 Python scorer 作为日常 CLI 路由权威
- [x] background `interrupt` replacement 已引入 pending takeover reservation，
  先占下一个 session owner 再完成 preempt，减少 release-then-requeue race
- [x] background admission 已不再依赖 `Semaphore._value`，而是改成显式 admitted
  job count contract

**交付物**

- runtime control-plane checkpoints
- background run-manager semantics
- stream / persistence seam plan
- trace replay/resume seam
- Rust wrapper-surface convergence note
- sandbox lifecycle implementation backlog

## Wave C6: Legacy Deprecation Lane

- [ ] 标记 `aionrs_companion_adapter` 为 legacy
- [ ] 标记 `aionui_host_adapter` 为 legacy
- [ ] 收缩旧 compatibility / rollback 叙事
- [ ] 明确最终移除前的保留边界

**交付物**

- legacy deprecation notes
- migration guardrails
- removal readiness checklist

## 4. Workstream 拆分

## Workstream A: Framework Core Contract

- [ ] 保持 `framework_profile` 为唯一真源
- [ ] 如需扩字段，先走 host-neutral contract，再走 adapter 投影
- [ ] 保持 Desktop / CLI 共用真源而不是各自派生

## Workstream B: Common Adapter

- [ ] 收口 shared projection
- [ ] 收口 shared artifact layout
- [ ] 避免 Desktop / CLI 重复持有同类编译逻辑

## Workstream C: Desktop Host Lane

- [ ] 强化 desktop-specific thread / automation / interactive contract
- [ ] 不把 Desktop 私有语义写回 common adapter

## Workstream D: CLI Host Lane

- [ ] 建立 batch / cron / CI / non-interactive contract
- [ ] 不让 CLI 私有语义反向绑定 framework core

## Workstream E: Artifact / Memory / Approval Bridge

- [ ] 统一 artifact emission
- [ ] 统一 memory mounts
- [ ] 统一 approval / loadout 语义

## Workstream F: Parity / Regression

- [ ] 维护 Desktop / CLI parity snapshots
- [ ] 保持 `codex_dual_entry_parity_snapshot` 作为 regression baseline
- [ ] 把 `upgrade_compatibility_matrix` 压成 secondary compatibility inventory / smoke artifact
- [ ] 建立 legacy vs current contract diff 辅助

## Workstream G: Rust Compiler Lane

- [ ] 维护 Rust `framework_profile` contract 镜像
- [ ] 维护 Rust first-class parity snapshot artifacts
- [ ] 维护 Rust first-class shared contract emission
- [ ] 仅把 Rust bundle 保留为 compatibility surface，而不是主消费面

## 5. 多 Agent 并行开发矩阵

原则：

- framework core contract 由主集成人统一收口
- sidecar 只拿写集清晰的 lane
- adapter、parity、Rust lane 可以并行

### 可立即并行的 lane

1. Agent A: Common Adapter Lane
   - 负责 `codex_common_adapter`
   - 不改 framework core 真源

2. Agent B: Desktop / CLI Host Lane
   - 负责 `codex_desktop_adapter` / `codex_cli_adapter` 边界
   - 负责 desktop host alias 退役门槛
   - 不改 common contract 真源

3. Agent C: Parity Snapshot Lane
   - 负责 Desktop / CLI parity snapshots、regression baseline
   - 负责 compatibility matrix 降级到 secondary inventory
   - 不改宿主私有实现

4. Agent D: Rust Contract Lane
   - 负责 Rust shared contract emission / parity
   - 不做 runtime rewrite，但可以推进 runtime-adjacent compiler /
     control-plane seams

5. Agent E: Legacy Deprecation Lane
   - 负责旧文档、旧叙事、旧 checklist 的迁移收口
   - 不新增 legacy 投资

6. Agent F: Runtime Control-Plane Lane
   - 负责 background run semantics / trace / stream / persistence seams
   - 不改 framework truth
   - 不把宿主 adapter 变成 runtime kernel

## 6. 推荐执行顺序

1. 保持 `framework_profile` 为唯一真源
2. 先稳住 common adapter 与 shared contract
3. 再收口 desktop / cli 正式 surface 与 alias 退出门槛
4. 并行推进 parity lane、Rust lane、runtime control-plane lane
5. 最后收口 legacy deprecation

## 7. 阶段验收线

- [ ] `Codex Desktop` 和 `codexcli` 正式消费同一份 outer contract
- [ ] framework core 仍然是唯一主控
- [ ] `codexcli` 没有被提升成框架真源
- [ ] `codex_desktop_host_adapter` 只剩 compatibility-only mirror 角色
- [ ] Desktop / CLI 有稳定 parity snapshots 和 regression baseline
- [ ] 旧 `aionrs` / `AionUI` 路线被明确降级

## 8. 明确不在下一阶段做的事

- [ ] 不回到 `aionrs` / `AionUI` 主线规划
- [ ] 不把 `codexcli` 变成主控
- [ ] 不为了 CLI 入口牺牲 Desktop / CLI 双入口复用
- [ ] 不把 Rust 化误解成“把 framework core 下沉进 CLI runtime”

## 9. 下一阶段的唯一成功定义

如果下一阶段做完，你拿到的不应该是“一个新的单宿主 CLI 架构”，而应该是：

- 一个宿主无关的 framework core
- 一个 `codex_common_adapter`
- 一个 `codex_desktop_adapter`
- 一个 `codex_cli_adapter`
- 一个只保留 compatibility-only mirror 角色的 `codex_desktop_host_adapter`
- 一套以 `codex_dual_entry_parity_snapshot` 为主的 Desktop / CLI parity / regression 机制
- 一条持续扩展但不越界的 Rust contract lane
