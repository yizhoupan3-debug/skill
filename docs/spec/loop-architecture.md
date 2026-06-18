---
parent: docs/spec.md
version: loop-architecture-v3.1
status: implemented
x-do-not-delete: |
  ╔══════════════════════════════════════════════════════════════╗
  ║  本文件为 Loop Architecture 的**实现规约**。                   ║
  ║  core/loop-engine/ 已实现（~2420 LOC, 9 modules），          ║
  ║  LOOP_REGISTRY.json 已创建。                                 ║
  ║  与科研 Harness 的桥接见 docs/spec/research-harness.md §19.9。║
  ╚══════════════════════════════════════════════════════════════╝
---

> **✅ loop-engine crate 已实现**：`core/loop-engine/` ~2420 LOC，9 模块。
> LOOP_REGISTRY.json 已创建（`configs/framework/LOOP_REGISTRY.json`）。
> `router-rs loop <subcommand>` CLI 入口见 §4.6。
> 与科研 Harness 的桥接（research-aware loop）见 `docs/spec/research-harness.md` §19.9。

# Loop Architecture — Framework v8 重构规约

> 本规约定义从**交互式 my-light** 到**自动执行 loop-auto** 的框架级重构。
> 交互式入口（`/discussx` → `/planx` → `/implementx` → `/verifyx`）
> 作为执行层保留；调度层为新增子系统，不修改现有技能生命周期。

---

## 目录

1. [问题陈述](#1-问题陈述)
2. [新 Profile 体系](#2-新-profile-体系)
3. [架构变更](#3-架构变更)
4. [循环调度引擎](#4-循环调度引擎)
5. [状态持久化](#5-状态持久化)
6. [安全模型](#6-安全模型)
7. [验证门控](#7-验证门控)
8. [Loops 协调](#8-loops-协调)
9. [Loop 模式 Catalog](#9-loop-模式-catalog)
10. [OpenCode 适配](#10-opencode-适配)
11. [Comprehension Debt 防御](#11-comprehension-debt-防御)
12. [迁移路径](#12-迁移路径)
13. [向后兼容](#13-向后兼容)
14. [未解决的问题](#14-未解决的问题)

---

## 1. 问题陈述

现有 `lifecycle_profile: my-light` 的设计目标与无人值守循环根本冲突：

| my-light 行为 | 对自动化循环的影响 |
|-------------|------------------|
| closeout advisory（无硬拦） | 循环无法可靠判断"是否完成" |
| REVIEW_GATE suppressed | 无独立验证者检查循环产出 |
| spawn-first nudge disabled | 不自动配 reviewer |
| Stop 不写 GOAL_CONTINUE | 跨会话状态丢失 |
| `disable_spawn_first_nudge: true` | 默认不 spawn 子代理 |

**核心矛盾**：my-light 的每个"轻量化设计"都是自动化循环需要的"安全门控"。

---

## 2. 新 Profile 体系

### 2.1 Profile 对比

| Profile | 用途 | closeout | REVIEW_GATE | spawn-first | 调度能力 |
|---------|------|----------|-------------|-------------|----------|
| `my-light`（废弃，不推荐） | 旧交互式，保留向后兼容 | advisory | suppressed | disabled | 拒绝被调度 |
| `interactive` | 取代 my-light，人工在回路中 | advisory | suppressed | disabled | 拒绝被调度 |
| `loop-auto` | 无人值守循环 | **hard-block** | **mandatory** | **强制** | **完整** |
| `loop-supervised`（v8.1+） | 循环 + 人工审批门控 | hard-block | mandatory | 强制 | 完整 |

> `interactive` 的行为与旧 `my-light` 完全一致（suppressed / disabled），
> 别名映射不会改变现有用户体验。`loop-supervised` 的设计推迟到 v8.1+。

### 2.2 `loop-auto` Profile

```yaml
profile: loop-auto
closeout_enforcement: hard-block         # 证据不全时阻止完成
review_gate: mandatory                    # 必须配独立 reviewer
spawn_first_nudge: true                   # 自动 spawn subagent
goal_continuation: auto-checkpoint        # Stop 时自动写 checkpoint
cost_budget:
  tokens_per_run: 200000                  # 软限，见 §14-6
  daily_budget: 1000000
kill_switch: loop-kill-enabled
verification_required: true
loop_capable: true
escalation:
  on_closeout_fail: "record_and_skip"
  on_verify_fail: "retry_max_2"
  on_budget_exceeded: "escalate"
  on_unexpected_error: "escalate"
```

### 2.3 `interactive` Profile

```yaml
profile: interactive
closeout_enforcement: advisory
review_gate: suppressed
spawn_first_nudge: false
goal_continuation: manual-boards-only
loop_capable: false
```

### 2.4 Profile→Runtime 传播路径

Loop Runner **不从 GOAL_STATE.lifecycle_profile 读取 profile**。
它直接从 LOOP_REGISTRY.json 读字段，在内存中持有 `ProfileConfig` 结构体。

```rust
// core/loop-engine/src/runner.rs
// PREFLIGHT 阶段执行
fn preflight_profile_check(entry: &LoopRegistryEntry) -> Result<(), LoopError> {
    match entry.profile.as_str() {
        "interactive" | "my-light" => Err(LoopError::ProfileMismatch(
            "interactive/my-light profile is not schedulable. \
             Use loop-auto for unattended execution."
        )),
        "loop-auto" => Ok(()),
        other => Err(LoopError::UnknownProfile(other.to_string())),
    }
}
```

**Profile 向下传播链**：

```
LOOP_REGISTRY.json → LoopRunner 读 profile 字段
                      ↓
                   ProfileConfig 内存结构体
                      ├─ closeout_enforcement → VERIFYING 阶段直接
                      │   传给 evaluate_closeout_record() 的上层调用方
                      │   （Runner 自身决定是否 hard-block）
                      ├─ cost_budget → 用于软性 budget 检查
                      └─ verification_required → closeout 聚合条件
```

关键决策：**Loop Runner 是 `loop-auto` 强制的唯一执行者。** Runner 自己读
registry 后直接 enforce，不依赖宿主 hook 中 `lifecycle_profile` 的
passthrough 逻辑。closeout enforcement 的"强制"不存在于宿主 hook 中，
而存在于 Loop Runner 的 VERIFYING 阶段。

---

## 3. 架构变更

### 3.1 调度层与执行层严格分离

```
┌──────────────────────────────────────────────────────────┐
│                    调度层（Loop Engine）                     │
│   cron/GitHub Actions → runner → dispatcher → verifier     │
│   纯控制平面。不 spawn agent、不写代码。                    │
│   只产 action list + verification 结果。                    │
└──────────────────────┬───────────────────────────────────┘
                       │
         spawn subagent│ 直接通过 CLI 子进程调用 opencode
         (命令行)      │ 不经 implementx，旁路 lane split 规则
                       │
                       ▼
┌──────────────────────────────────────────────────────────┐
│                    执行层（opencode CLI）                   │
│   单一宿主（v8.0 hardcoded opencode）                       │
│   执行 handoff prompt → 写文件 → 运行验证命令               │
│   产出 lane-notes + evidence + closeout record              │
└──────────────────────────────────────────────────────────┘
```

### 3.2 Crate 拓扑（已实现）

```
core/loop-engine/ (~2420 LOC, 9 modules)   ← 已实现并编译
├── runner.rs                               ← 主运行循环 + phase 状态机（495 LOC）
│   ├── preflight_profile_check()           ← profile 校验（interactive/my-light 拒绝调度）
│   ├── run_loop()                          ← 入口：acquire lock → run_loop_inner → release
│   └── run_loop_inner()                    ← Discovering → Preflight → Dispatching → Running → Verifying
├── types.rs                                ← 全部核心类型定义（485 LOC）
│   ├── LoopPhase enum                      ← Pending/Discovering/Preflight/Dispatching/Running/Verifying/Completed/Escalated/Interrupted
│   ├── SafetyLevel enum                    ← L1ReportOnly/L2AssistedFix/L3Unattended
│   ├── LoopProfileConfig                   ← 从 RUNTIME_REGISTRY.json 加载
│   ├── LoopRegistryEntry / LoopRegistryRoot ← LOOP_REGISTRY.json 反序列化
│   ├── LoopAction / LoopActionRecord
│   ├── LoopCloseoutAggregate / AggregateActionEntry
│   └── LoopRunState / CurrentRun / DiscoveryResult / CircuitBreaker
├── state.rs                                ← 状态持久化（208 LOC）
│   ├── read_loop_state / write_loop_state  ← 原子写入 LOOP_RUN_STATE.json
│   ├── create_initial_state / start_new_run / finish_run
│   ├── transition_phase / update_heartbeat
│   └── loop_state_path / lock_path / kill_signal_path / closeout_path
├── safety.rs                               ← scope-based safety 分配（212 LOC）
│   ├── parse_safety_level / assign_safety_for_file / assign_safety_for_action
│   ├── resolve_conflict
│   └── path_matches（支持 **/ 和 * 通配符）
├── kill_switch.rs                          ← 锁 + kill 信号（206 LOC）
│   ├── acquire_lock / release_lock / read_lock_info
│   ├── write_kill_signal / clear_kill_signal / is_kill_signal_active
│   └── LoopLock / LockInfo types
├── dispatcher.rs                           ← action 执行（229 LOC）
│   ├── build_handoff                       ← 模板渲染
│   ├── resolve_subagent_binary             ← ROUTER_RS_SUBAGENT_BIN / which opencode
│   ├── run_action_sync / run_action_dry_run
│   └── check_scope_compliance              ← git diff 越界检测
├── closeout.rs                             ← 验证门控（364 LOC）
│   ├── verify_closeout_value / verify_closeout_with_evidence
│   ├── verify_evidence_index / read_action_record
│   └── build_aggregate
├── report.rs                               ← 报告渲染（195 LOC）
│   ├── render_loop_report / write_loop_report
│   └── render_action_section
└── lib.rs                                  ← pub 导出全部 API（26 LOC）
```

**依赖关系**：

```
router-rs → runtime-core → core/loop-engine
                            └── 无外部依赖（仅 serde, chrono, std）
                            └── 不依赖 host-projection（无 MCP tool 注册）
                            └── 不依赖 core-state（LoopActionRecord 自包含）
```

**未实现的 v8 设计项**（标记 deferred）：
- `loop-supervised` profile（v8.1+）
- OTel 映射与核心指标计数器（v8+）
- 多机器锁（v8.1+）
- `loop report` 子命令的 `--json` 与 `--html` 格式输出
│                                            + scope 冲突仲裁
│                                            + git diff 越界检测（仅报告）
├── kill_switch.rs                          ← 紧急停止（poll loop，无独立线程）
├── closeout.rs                             ← LoopActionRecord + 聚合
│                                            直接调用 evaluate_closeout_record()
├── report.rs                               ← LOOP_REPORT.md 渲染
└── types.rs                                ← LoopRegistryEntry, LoopAction,
                                              LoopActionRecord, LoopCloseoutAggregate

core-state/                                 ← 扩展
├── loop_types.rs (NEW)                     ← LoopActionRecord 类型
│                                             嵌入 CloseoutRecord（不修改现有类型）
└── ...                                     ← 其余不变

runtime-core/src/cli/
└── loop_cli.rs (NEW)                       ← router-rs loop {run,status,kill}

host-projection/src/hosts/                  ← 最小变更
└── ...                                     ← my-light → interactive 映射
```

### 3.3 依赖关系

```
router-rs → runtime-core → core/loop-engine → framework-runtime
                                            → (closeout_enforcement 直接调用)
```

`loop-engine` 不依赖 `host-projection`（无 MCP tool 注册），
不依赖 `core-state`（LoopActionRecord 是自包含类型），
不依赖 `runtime-storage`（不使用 background_state）。

### 3.4 锁模型

单层 `.loop-active` 文件锁。无 SQLite TTL lock，无独立锁线程。

```
路径: <repo_root>/.loop-active
内容: {"loop_id": "daily-triage", "run_id": "run-xxx", "acquired_at": "..."}

行为:
  - Loop Runner 在 PENDING→DISCOVERING 时写入
  - Loop Runner 在 COMPLETED/ESCALATED/INTERRUPTED 时删除
  - Runner 启动时检测是否已有 .loop-active：
    如果存在且未过期（<1h）→ 报错退出
    如果存在且已过期（≥1h）→ 覆盖并记录 warning
   - 创建方式：`O_CREAT | O_EXCL` 原子创建，非原子 write（避免 TOCTOU 竞态）
   - opencode CLI 启动时检测此文件（v8.1）：
     如果存在 → 打印告警并以只读模式运行
```

**为什么没有 action 级别的锁**：action 间的 scope_paths 被 handoff 模板
强制 disjoint（§4.4）。如果两个 action 的 scope overlap，是 discovery
分配阶段的 bug，不是锁来掩盖的问题。action 间偶发的 git 冲突成本
（需手动解决）远低于一个 SQLite TTL 锁系统的维护成本。

### 3.5 Subagent 执行（已实现，`core/loop-engine/src/dispatcher.rs`）

v8.0 硬编码 opencode CLI（`resolve_subagent_binary`）：

```rust
// core/loop-engine/src/dispatcher.rs — 实际代码

pub fn resolve_subagent_binary() -> Result<String, LoopError> {
    // 1. ROUTER_RS_SUBAGENT_BIN 环境变量
    // 2. which opencode
}

pub fn run_action_sync(repo_root, loop_id, run_id, action, timeout) -> Result<SubagentResult> {
    let handoff = build_handoff(action, loop_id, run_id);
    let child = Command::new(binary).args(["-p", &handoff])...;
    // 同步等待，5s 轮询 kill 信号
    // timeout 后 child.kill()
}

pub fn check_scope_compliance(repo_root, action) -> Vec<String> {
    // git diff --name-only --diff-filter=ACMR
    // 过滤 scope_paths 外的文件
}
```

**越界检测只报告不做恢复**（已实现）：发现越界后记录到 SubagentResult，不 `git reset --hard`。
        match child.wait_timeout(Duration::from_secs(5))? {
            Some(status) => break status,
            None => {
                if kill_switch_armed() {
                    child.kill()?;
                    child.wait()?;
                    return Err(SpawnerError::Killed);
                }
                if Instant::now() > deadline {
                    child.kill()?;
                    child.wait()?;
                    return Err(SpawnerError::Timeout(timeout));
                }
            }
        }
    };

    let output = child.stdout.take().unwrap();
    Ok(SubagentResult::from_reader(output, status.success()))
}
```

**v8.1** 将 `run_action` 提取为 `SubagentExecutor` trait，加入多宿主支持。
当前单宿主硬编码不构成抽象障碍——v8.1 提取 trait 时不需要改动调用方。

### 3.6 Kill / 超时机制

使用 `std::process::Child` 的跨平台 API，无独立 watchdog 线程：

| 机制 | 实现 |
|------|------|
| 超时 | `wait_timeout(Duration)` 返回后检查 deadline |
| kill 信号 | 主线程的 5s poll loop 检查 kill 文件 |
| 进程终止 | `child.kill()`（跨平台，无需 unsafe） |
| 进程崩溃 | `wait_timeout` 返回非 0 exit code |

同步单线程模式已经覆盖所有 kill/超时场景，不需要独立线程或 libc。

---

## 4. 循环调度引擎

### 4.1 Loop 注册

`configs/framework/LOOP_REGISTRY.json`：

```json
{
  "schema_version": "loop-registry-v1",
  "loops": [
    {
      "loop_id": "daily-triage",
      "profile": "loop-auto",
      "trigger": {
        "type": "cron",
        "schedule": "0 */6 * * *",
        "timezone": "UTC"
      },
      "skill": "loop-daily-triage",
      "scope_based_safety": {
        "src/**/*.rs": "L2-assisted-fix",
        "*.md": "L3-unattended",
        "Cargo.toml": "L1-report-only"
      },
      "default_safety": "L1-report-only",
      "scope_conflict_resolution": "split",
      "cost_budget": {
        "tokens_per_run": 200000,
        "daily_tokens": 1000000
      },
      "notification": {
        "on_escalation": "issue",
        "on_completion_L3": "commit-only"
      }
    }
  ]
}
```

无独立 schema 文件。`LOOP_REGISTRY.json` 的 schema 由 Rust 编译期
反序列化保证（`#[derive(Deserialize)]` + `#[serde(deny_unknown_fields)]`）。
与现有 `RUNTIME_REGISTRY.json` 的治理方式一致。

### 4.2 状态机

```
                   ┌──────────────┐
      cron/webhook →   PENDING    │  写入 .loop-active
                   └──────┬───────┘
                          │
                          ▼
                   ┌──────────────┐
                   │ DISCOVERING  │  跑 loop skill discovery prompt
                   │              │  opencode CLI 同步调用
                   │              │  5s 间隔检查 kill 信号
                   └──────┬───────┘
                          │ discover() → action list
                          ▼
                   ┌──────────────┐
                   │  PREFLIGHT   │  profile 校验、scope 安全级别分配
                   │              │  scope 冲突仲裁、budget 预检
                   └──────┬───────┘
                          │
                          ▼
                   ┌──────────────┐
                   │ DISPATCHING  │  遍历 action 列表
                   │              │  L1 → skipped（写 report）
                   │              │  L2/L3 → spawn opencode CLI
                   └──────┬───────┘
                   ┌──────┴──────────┐
                   ▼                 ▼
            ┌──────────┐     ┌──────────┐
            │ RUNNING  │     │ SKIPPED  │  L1 / budget
            │          │     └──────────┘
            │ 同步执行  │
            │ kill 检查 │
            └─────┬────┘
                  │ all actions complete
                  ▼
            ┌──────────────┐
            │ VERIFYING    │  读各 action 的 closeout record
            │              │  直接调用 evaluate_closeout_record()
            │              │  聚合判定 pass/partial/fail
            └──────┬───────┘
            ┌──────┴─────────┐
            ▼                ▼
     ┌───────────┐    ┌───────────┐
     │ COMPLETED │    │ ESCALATED │  连续失败 → 暂停
     │           │    │           │  开 Issue 通知
     │ └→ report │    │ └→ issue  │
     │ └→ 删锁   │    │ └→ 删锁   │
     └───────────┘    └───────────┘

     ┌──────────────┐
     │ INTERRUPTED  │  kill 信号 / 超时
     │              │  部分完成 action → overall_status = "partial"
     │ └→ 删锁      │  report 列出已完成和未完成的 action
     └──────────────┘
```

**增量发现限制**：Loop Runner 不处理 subagent 的增量发现。subagent 的
`next_action` 写入 `unconsumed_findings[]`，留给下一轮 DISCOVERING。

**去重**：每个 finding 携带 `finding_hash`（SHA256），Runner 在聚合时跳过
已出现的 hash，避免同一 finding 每轮重复报告。去重缓存存活期为一次运行周期。

### 4.3 执行模型（已实现 — `runner.rs:run_loop()`）

```
Loop Runner（router-rs loop run / 直接调用 run_loop）

  1. preflight_profile_check(entry)
     → 拒绝 interactive/my-light 调度
     → 加载 LoopProfileConfig

  2. run_loop(ctx: RunContext)
     → PENDING: acquire_lock (.loop-active 原子创建)
     → DISCOVERING: discover_actions(entry, repo_root)
        生成 Vec<LoopAction>（含 scope_paths + safety 级别）
     → PREFLIGHT: assign_safety_levels + check_budget_preflight
     → DISPATCHING: 遍历 action 列表
        每个 L2/L3 action → run_action_sync(opencode CLI, 超时, kill 轮询)
        每个 L1 action → AggregateActionResult::Skipped
     → RUNNING: 过渡阶段
     → VERIFYING:
        read_action_record → verify_closeout_with_evidence → build_aggregate
        断路器逻辑：consecutive_failures ≥ 3 → 暂停
     → COMPLETED/ESCALATED:
        render_loop_report → write_loop_report
        release_lock (.loop-active 删除)

  3. router-rs loop status --loop-id <id>    [见 §4.6]
     → 读取 LOOP_RUN_STATE.json

  4. router-rs loop kill --loop-id <id>      [见 §4.6]
     → write_kill_signal → 下次 poll 检测后终止
```

### 4.6 router-rs CLI 接入点

`router-rs` 的 `loop` 子命令组（注册在 `core/router-rs/src/`）：

```
router-rs loop run --loop-id <id> [--dry-run] [--timeout <secs>]
    → 调用 core/loop-engine::runner::run_loop()

router-rs loop status --loop-id <id>
    → 读取并展示 LOOP_RUN_STATE.json

router-rs loop kill --loop-id <id>
    → write_kill_signal
    → 触发下次 poll 中断

router-rs loop list
    → 列出 LOOP_REGISTRY.json + 每个 loop 的当前状态
```

**CLI 输入的必经校验**：
1. `loop_id` 必须非空，匹配 `^[a-z0-9_-]+$`
2. `--timeout` 取值范围 [30, 3600] 秒
3. `--dry-run` 时跳过 acquire_lock

### 4.4 DISPATCHING Handoff 模板

```text
## Objective
<单一目标>

## Scope (HARD)
- Write scope: <action 路径>
- Forbidden: 不得修改 scope 外的任何文件

## Action
- 文件修改 + 运行验证命令

## Closeout
- 写入 changed_files
- 运行验证命令并记录输出
- 写入 evidence 到 artifacts/loop/<loop-id>/evidence/<action-id>/
- 结果写入 artifacts/closeout/<action-id>.json

## Safety
- 每 10000 tokens 检查 kill 信号（软防线）
- Kill 信号文件: <repo_root>/.loop-kill/<loop-id>
```

### 4.5 Scope 越界检测

```rust
fn check_scope_compliance(repo_root: &Path, action: &LoopAction) -> Vec<String> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=ACMR"])
        .current_dir(repo_root)
        .output()?;
    let changes: Vec<String> = parse_file_list(&output.stdout);
    changes.into_iter()
        .filter(|f| !action.scope_paths.iter().any(|s| f.starts_with(s)))
        .collect()
}
```

越界检测**只报告不做恢复**。发现越界后：
- action 标记为 `ScopeViolation`
- 越界文件列表记录到 LOOP_REPORT.md
- **不运行 git reset --hard**（不销毁任何数据）
- L3 action 发现越界时降级为 L2：commit + branch（不 merge），
  待人工审核后再合并
- 下次循环时人工清理越界变更

---

## 5. 状态持久化

### 5.1 真源

```
artifacts/loop/<loop-id>/
├── LOOP_RUN_STATE.json    ← 唯一真源。runner 读/写，phase 决策依据
├── evidence/<action-id>/  ← 各 action 的证据目录
├── reports/<run-id>.md    ← LOOP_REPORT.md（从 LOOP_RUN_STATE 渲染）

artifacts/closeout/<action-id>.json
   ← 每个 action 的 CloseoutRecord。loop 不修改现有 closeout 路径
```

**不使用 background_state**。v8.0 LOOP_RUN_STATE 是唯一状态文件。
background_state 集成在 v8.1 考虑（如果需要）。

**一致性**：`loop status` 等只读操作通过 atomic rename 读取 LOOP_RUN_STATE，
但 DISPATCHING 阶段写入时可能读到不完整快照。接受 stale read（监控工具的
正常行为），不额外加锁。

### 5.2 LOOP_RUN_STATE.json

```json
{
  "schema_version": "loop-run-state-v1",
  "loop_id": "daily-triage",
  "profile": "loop-auto",
  "phase": "discovering",
  "last_heartbeat": "2026-06-16T06:00:00Z",
  "current_run": {
    "run_id": "run-20260616-0600",
    "started_at": "2026-06-16T06:00:00Z",
    "discovery": {
      "actions_found": 3,
      "actions": [
        {"id": "a1", "type": "dependency-update", "scope": ["Cargo.toml"], "safety": "L1"},
        {"id": "a2", "type": "fix-warning", "scope": ["src/cli.rs"], "safety": "L2"}
      ]
    },
    "unconsumed_findings": [
      {"finding_hash": "sha256:abc123", "source_action": "a2", "finding": "相邻文件也有相同 deprecated API"}
    ],
    "dispatch": {
      "a1": "skipped",
      "a2": "running"
    },
    "closeout_aggregate": null,
    "report_path": null
  },
  "history": [
    {"run_id": "run-20260615-1800", "phase": "completed", "result": "success"}
  ],
  "circuit_breaker": {
    "consecutive_failures": 0,
    "kill_switch_armed": false,
    "kill_switch_triggered_at": null
  },
  "last_refreshed_at": "2026-06-16T06:00:05Z"
}
```

### 5.3 LoopActionRecord（新类型，不修改 CloseoutRecord）

每个 action 的 closeout 写入 `artifacts/closeout/<action-id>.json`，
使用新类型 `LoopActionRecord`，**不修改现有 `CloseoutRecord`**。

```rust
/// 新类型，嵌入现有 CloseoutRecord。不碰现有 schema。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopActionRecord {
    pub schema_version: String,          // "loop-action-record-v1"
    pub loop_id: String,
    pub run_id: String,
    pub action_id: String,
    pub safety_level: String,
    pub closeout: CloseoutRecord,        // 嵌入现有类型，直接传给 evaluate_closeout_record
}
```

Loop Runner 在 VERIFYING 阶段读 `LoopActionRecord.closeout` 传给
`evaluate_closeout_record()`。现有 `evaluate_closeout_record_with_context`
的 `task_id` 匹配逻辑不变——`CloseoutRecord` 自身的 `task_id` 会被
subagent 填写为 `action_id`，runner 做聚合时按 `LoopActionRecord.loop_id`
匹配而非按 `task_id`。

```rust
// core/loop-engine/src/closeout.rs

fn verify_run(run: &CurrentRun) -> VerificationResult {
    let mut aggregate = LoopCloseoutAggregate::new(&run.run_id);
    for action in &run.discovery.actions {
        if action.safety == "L1" {
            aggregate.skip(action);
            continue;
        }
        let record_path = closeout_path(&run.run_id, &action.id);
        let action_record: LoopActionRecord = read_json(&record_path)?;
        let response = evaluate_closeout_record(&action_record.closeout);
        if !response.closeout_allowed {
            return aggregate.fail(&action.id, response.violations);
        }
        aggregate.pass(&action.id, &action_record);
    }
    aggregate.done()
}
```

### 5.4 LoopCloseoutAggregate

```json
{
  "schema_version": "loop-closeout-aggregate-v1",
  "run_id": "run-20260616-0600",
  "loop_id": "daily-triage",
  "overall_status": "pass",
  "actions": [
    {
      "action_id": "a1",
      "safety_level": "L1",
      "execution": "skipped",
      "closeout_path": null
    },
    {
      "action_id": "a2",
      "safety_level": "L2",
      "execution": "committed",
      "closeout_path": "artifacts/closeout/a2-20260616-0600.json",
      "verification": "pass",
      "commit_sha": "abc1234",
      "merged": false
    }
  ],
  "escalated": false,
  "partial": false
}
```

`overall_status`：

| 值 | 含义 |
|----|------|
| `pass` | 所有 action 完成，无中断 |
| `partial` | 部分 action 完成（kill/超时中断） |
| `fail` | 验证失败 |
| `escalated` | 因 budget/未知错误升级给人 |

### 5.5 证据路径

| 场景 | 路径 | 写入者 |
|------|------|--------|
| subagent 执行 | `artifacts/loop/<loop-id>/evidence/<action-id>/` | subagent |
| runner 操作 | `artifacts/loop/<loop-id>/evidence/runner/` | runner |
| closeout | `artifacts/closeout/<action-id>.json` | subagent |

---

## 6. 安全模型

### 6.1 Scope-Based Safety

| 级别 | 行为 | 适合 |
|------|------|------|
| **L1 report-only** | 发现 + 报告，不改文件 | `Cargo.toml` 变更、安全审计 |
| **L2 assisted-fix** | 修改 + 验证 + commit（不 merge） | `src/**/*.rs` 修复 |
| **L3 unattended** | 修改 + 验证 + commit（不 merge） | `*.md`、`*.json` 格式化 |

`scope_conflict_resolution`：

| 值 | 行为 |
|----|------|
| `split`（默认） | 拆为独立 action，避免不同安全级别互相污染 |
| `strictest` | 取最高安全级别 |
| `report` | 仅报告不执行 |

### 6.2 越界检测（仅报告）

subagent 执行完成后，`git diff --name-only` 检测 scope 外的修改。
发现越界后：
- action → `ScopeViolation` 状态
- 越界文件清单记录到 LOOP_REPORT.md
- **不运行 git reset --hard**（不销毁数据）
- 不阻止其他 action 的验证

### 6.3 断路器

| 条件 | 执行 | 恢复 |
|------|------|------|
| 连续 3 次运行失败 | 暂停循环，开 Issue | 人工恢复。成功运行后计数器重置为 0 |
| action 超时（>600s） | `child.kill()` | 下一轮重试 |
| `kill_switch_armed == true` | poll loop 检测后终止 | 人工 disarm |

### 6.4 Kill Switch

```bash
ROUTER_RS_LOOP_KILL=daily-triage router-rs loop kill
ROUTER_RS_LOOP_KILL_ALL=1 router-rs loop kill --all
router-rs loop status --kill-switch
```

**生命周期（fire-and-forget）**：
1. `loop kill` 写入 kill 信号文件（`<repo_root>/.loop-kill/<loop-id>`）
2. Runner poll loop（每 5s）检测到文件 → 终止当前运行 → **删除 kill 文件**
3. 下一轮正常运行
4. 若当前无运行，文件保留但启动时 Runner 检测到后立即拒绝启动并删除

**单机有效**。多机器 kill 开关未实现（§14-1）。信号延迟最多 5s（poll 间隔）。

---

## 7. 验证门控

### 7.1 Loop Runner 强制执行

VERIFYING 阶段直接调用 `evaluate_closeout_record()`（Rust 函数），
不走 MCP tool、不走宿主 hook。Runner 根据返回值自行决定 complete。

```rust
// core/loop-engine/src/closeout.rs
fn verify_loop_run(aggregate: &LoopCloseoutAggregate) -> VerificationResult {
    for action in &aggregate.actions {
        if action.execution == "skipped" { continue; }
        if let Some(path) = &action.closeout_path {
            let record: LoopActionRecord = read_json(path)?;
            let response = evaluate_closeout_record(&record.closeout);
            if !response.closeout_allowed {
                return VerificationResult::ActionFailed { ... };
            }
        }
    }
    VerificationResult::Pass
}
```

**不存在"宿主无 hook → advisory"的降级路径。**

### 7.2 验证合约

单 action 通过条件：

```
□ EVIDENCE_INDEX 非空
□ verification_status != "not_run"
□ 全部 verification.commands exit_code == 0
□ changed_files 非空（或明确说明无需改文件）
□ reviewer lane 已执行
□ 无 high-severity blocker
```

Loop 整体通过条件：

```
□ 全部 L2/L3 action 通过各自 closeout
□ L1 action 已报告
□ 无 failed/escalated action
□ kill_switch 未激活
```

---

## 8. Loops 协调

| 场景 | 行为 |
|------|------|
| 两个 loop 修改不同文件 | 并行执行（git 处理多文件提交） |
| 两个 loop 修改同一文件 | **.loop-active 互斥**，后者排队 |
| loop vs 交互式 | opencode CLI 启动时检测 .loop-active → 告警 + 只读模式（v8.1） |
| 同一 loop 两轮重叠 | `multitask_strategy: reject` |

**.loop-active 覆盖范围**：仅单机。多机器场景未实现（§14-1）。

---

## 9. Loop 模式 Catalog

### 9.1 Skill Frontmatter

```yaml
---
name: loop-daily-triage
routing_layer: L0
loop_only: true
loop:
  cadence: "every 6h"
  default_safety: L1
  discovery_prompt: |
    扫描过去 6 小时的新 Issue 和依赖更新，
    按严重程度排序。
---
```

`routing_layer` vs `loop_only`：

| `routing_layer` | `loop_only` | 行为 |
|-----------------|-------------|------|
| L0 | true | 被索引，`routing_owner` 不匹配（仅 runner 可调度） |
| L0 | false | 既走路由也走 runner |
| undefined | — | 仅通过 registry 发现 |

### 9.2 开箱模式

| Skill | Cadence | 安全级别 | 产出 |
|-------|---------|---------|------|
| loop-daily-triage | 6h | L1 | 分类报告 |
| loop-pr-babysitter | 5-15m | L2 | CI 看护 |
| loop-ci-sweeper | 5-15m | L2 | CI 重试 |
| loop-dependency-sweeper | 6h-1d | L1 | 依赖报告 |
| loop-changelog-drafter | 1d / tag | L3 | CHANGELOG.md |
| loop-post-merge-cleanup | 1d | L2 | 清理 TODO/FIXME |
| loop-issue-triage | 2h-1d | L1 | 分类 + 分配建议 |
| loop-stale-cleanup | 1w | L1 | 过期分支报告 |
| **loop-research-barrier** | **按需（on escalation）** | **L2** | **barrier report + candidate list（§19.9.1, spec/research-harness.md）** |
| **loop-hypothesis-test** | **按配置** | **L2** | **hypothesis verification result（§19.9.1, spec/research-harness.md）** |
| **loop-literature-watch** | **1w** | **L1** | **new papers digest（§19.9.1, spec/research-harness.md）** |
| **loop-claim-refresh** | **1w-2w** | **L1** | **drift detection report（§19.9.1, spec/research-harness.md）** |

**research-aware loop 详细契约**（与 `docs/spec/research-harness.md` §19.9 共享真源）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `research_enabled` | bool | 是否启用 barrier → research escalation |
| `research.barrier_threshold` | u64 | 连续失败 N 次后触发 |
| `research.escalation_target` | string | 固定为 "autoresearch" |
| `research.max_research_time_min` | u64 | 研究阶段最长耗时 |
| `research.auto_resume` | bool | 研究产出候选后是否自动恢复循环 |
| `research.require_human_approval` | bool | 是否需要在候选方案上人工确认 |

**LOOP_REGISTRY.json 扩展示例**：

```json
{
  "loop_id": "my-experiment",
  "profile": "loop-auto",
  "research_enabled": true,
  "research": {
    "barrier_threshold": 3,
    "escalation_target": "autoresearch",
    "max_research_time_min": 30,
    "auto_resume": true,
    "require_human_approval": false
  },
  "trigger": {
    "type": "manual"
  },
  "scope_based_safety": {
    "src/**/*.rs": "L2-assisted-fix"
  },
  "default_safety": "L1"
}
```

---

## 10. OpenCode 适配

| 限制 | loop 中的应对 |
|------|-------------|
| 无 batch/cron/CI | OS cron / GitHub Actions 触发 CLI |
| 无 worktree | `.loop-active` 防冲突 |
| 无 session supervisor | 独立 CLI 进程，完整运行后退出 |
| fail-open hook | 安全门控独立于宿主 hook（Runner 自身 enforce） |
| JS/TS 插件 | loop-engine 纯 Rust，无关 |

**Subagent 执行**：v8.0 硬编码 `opencode` binary。发现顺序：

```
ROUTER_RS_SUBAGENT_BIN → which opencode → error
```

**部署**：

```bash
# crontab
0 */6 * * * cd /repo && router-rs loop run --loop-id daily-triage
```

---

## 11. Comprehension Debt 防御

LOOP_REPORT.md 从 `LoopCloseoutAggregate` 渲染，后者从 closeout records
聚合。机器读 closeout JSON、人读 report。

```markdown
# Loop Report: daily-triage | 2026-06-16 06:00 UTC

## Summary
- 3 actions, 2 dispatched, 1 L1 report-only. Overall: PASS

## Actions
### a1: upgrade clap 4.5→4.6 (L1)
- Scope: Cargo.toml. Report only.
- Recommendation: `cargo update -p clap`

### a2: fix deprecation in src/cli.rs (L2 - committed)
- Scope: src/cli.rs
- Fix: `StructOpt::from_args()` → `Parser::parse()`
- Verification: cargo test PASS (142/142)
- Commit: abc1234 (not merged)

## Unconsumed Findings
- cli_utils.rs 也有相同 deprecation（下次循环重新发现）

## Lock
- .loop-active: 06:00→06:05. No conflicts.
```

L3 commit message：`[loop: daily-triage run-20260616-0600]`。
Branch（L2）：`loop/<loop-id>/<run-id>/<action-id>`。

---

## 12. 迁移路径

### ✅ v8.0（已完成）

- 新增 `core/loop-engine/` crate（~2420 LOC, 9 模块，全部已实现）
- `my-light` 保留为 deprecated 别名（不删除）
- 新增 `interactive` + `loop-auto` profile
- `RUNTIME_REGISTRY.json` 新增 `lifecycle_profiles` 条目
- 各宿主 hook `"my-light"` → `is_interactive_profile()` 封装
- Loop Runner 拒绝 interactive 调度
- `LOOP_REGISTRY.json` 已创建

### ✅ v8.1（已完成）

- 全部调度状态机已实现（runner.rs: run_loop / run_loop_inner）
- dispatcher.rs：build_handoff + run_action_sync（硬编码 opencode CLI）
- state.rs：原子写入 LOOP_RUN_STATE.json
- safety.rs：scope-based L1/L2/L3 分配 + glob 路径匹配
- kill_switch.rs：`.loop-active` 锁 + kill signal poll loop
- closeout.rs：LoopActionRecord + verify_closeout_with_evidence + build_aggregate
- report.rs：LOOP_REPORT.md 渲染与写入

### v8.2（待实现）

- skills/ 开箱 loop 模式
- SubagentExecutor trait + 多宿主
- 跨宿主部署文档
- `loop-supervised` profile 设计审批门控

---

## 13. 向后兼容

| 旧 | 新 | 策略 |
|----|----|------|
| `my-light` | `my-light`（deprecated） | 别名映射，行为不变 |
| `artifacts/current/<task>/GOAL_STATE.json` | 不变 | 交互式路径不变 |
| `/implementx` | 不变 | Loop Runner 不经过 |
| my-light hook 代码 | `is_interactive()` | 函数封装，不改逻辑 |
| `CloseoutRecord` | 不变 | Loop 使用新 `LoopActionRecord` |
| `background_state` | 不变 | v8.0 不使用 |
| `evaluate_closeout_record_with_context` | 不变 | task_id 匹配逻辑不改 |

---

## 14. 未解决的问题 & 已解决的问题

### ✅ 已解决

| 问题 | 解决方案 | 位置 |
|------|---------|------|
| runner.rs + dispatcher.rs（opencode 集成） | 已实现 | `core/loop-engine/src/runner.rs`, `dispatcher.rs` |
| `.loop-active` 锁 + kill switch poll loop | 已实现 | `core/loop-engine/src/kill_switch.rs` |
| LoopActionRecord + closeout 聚合 | 已实现 | `core/loop-engine/src/closeout.rs`, `types.rs` |
| 越界 git diff 检测 + LOOP_REPORT.md 渲染 | 已实现 | `core/loop-engine/src/dispatcher.rs` (`check_scope_compliance`), `report.rs` |
| 断路器（连续 N 次失败暂停） | 已实现 | `core/loop-engine/src/runner.rs` (`consecutive_failures ≥ 3`) |
| Research-aware loop 模式（barrier escalation） | 已设计 | `docs/spec/research-harness.md` §19.9 |

### 待解决

1. **多机器协调**：`.loop-active` 仅单机。跨 NFS 场景未实现。
2. **跨 Repo 循环**：每个 repo 独立 cron job。无统一调度器。
3. **安全级别动态适应**：仅静态声明。动态适应 v8.2+。
4. **回滚机制**：L3 提交后无自动回滚。v8.1 可加 checkpoint tag。
5. **LLM 模型选择**：不支持。v8.2 在 frontmatter 加 `model_hint`。
6. **Token 预算 soft**：无法精确追踪 token。硬限制用 wall-clock timeout。
7. **`loop-supervised` profile**：审批门控推迟到 v8.1。
8. **多宿主 subagent**：v8.0 仅 opencode。多宿主在 v8.1 加 trait 后支持。

---



*本规约是 `docs/spec.md` 的延伸文档。科研 Harness 桥接见 `docs/spec/research-harness.md` §19.9。*
