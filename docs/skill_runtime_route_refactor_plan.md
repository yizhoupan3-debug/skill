# Skill Runtime/Route Refactor Plan

## 0. Decision

本次重构的目标不是“让 `gsd` 更好触发”，而是彻底取消对显式 `gsd` 的依赖。

默认体系应该收敛成：

```text
规范 -> 计划 -> 实施 -> 验证
```

这四步是 runtime/route 的默认执行协议，不是某个 skill，不是某个 controller，也不需要用户显式说 `gsd`、`推进到底`、`别停` 才启动。

`autopilot` 保留为用户手动调用的 command，不纳入本次默认体系重构；本计划不修改 `.codex/skills/autopilot/SKILL.md`，也不把 autopilot 合并进 skill owner 体系。

## 1. Diagnosis

当前系统复杂的根因是三类抽象混在一起：

| Layer | 应该负责 | 当前问题 |
|---|---|---|
| `runtime protocol` | 所有任务的默认执行闭环 | 被写进 `execution-controller-coding` / GSD 姿态里 |
| `route` | 选择最窄 owner、gate、overlay，并产生 route context | 把 `gsd/推进到底` 直接 boost 到 controller |
| `skill` | 领域能力或专门 gate/overlay | 混入通用执行哲学、command alias、全局闭环规则 |
| `command` | 用户显式进入的模式 | 和 skill owner / runtime contract 互相引用 |

结果是同一件事被表达成三份：

1. `execution-controller-coding` 的 trigger / GSD posture。
2. router scoring 里的 `gsd_execution_markers()` owner boost。
3. framework profile / runtime registry 里的 controller contract 或 command owner。

这违反减法原则：系统不是更本质，而是多了多个“中枢”。

## 2. Target Architecture

重构后只保留四个必要抽象。

### 2.1 Runtime Protocol

Runtime protocol 是唯一默认执行协议：

```text
规范 -> 计划 -> 实施 -> 验证
```

职责：

- `规范`: 收敛对象、动作、约束、交付物、成功标准。
- `计划`: 选择最窄 owner、必要 gate/overlay、最小路径、验证路径。
- `实施`: 按最小 delta 执行，不扩大抽象。
- `验证`: 用测试、命令、截图、产物或明确 blocker 关闭任务。

原则：

- 它默认存在，不靠 trigger。
- 它不是 skill，不参与 owner 竞争。
- 它不替代 domain owner。
- 它只定义执行闭环和证据要求。

### 2.2 Route Context

Route 不应该把执行姿态变成 owner boost，而应该产出上下文标记。

建议 route context 字段：

```json
{
  "execution_protocol": "four_step",
  "verification_required": true,
  "evidence_required": true,
  "supervisor_required": false,
  "delegation_candidate": false,
  "route_reason": "narrowest_domain_owner"
}
```

用户说“推进到底 / 别停 / 直接干完 / 给验证证据”时，只能影响：

- `verification_required`
- `evidence_required`
- `continue_safe_local_steps`

不能影响：

- `selected_skill`
- `canonical_owner`
- `routing_layer`

### 2.3 Skill Owner

Skill 只回答一个问题：

> 这个任务最窄、最懂领域的 owner 是谁？

Skill 不应该重复写：

- 通用执行四步。
- “不要偷懒”这类全局质量规则。
- command alias。
- 全局 runtime continuity 规则。
- GSD / autopilot 姿态。

保留的 skill 类型：

| Type | Role |
|---|---|
| `owner` | 领域实现或分析能力 |
| `gate` | 来源、证据、产物、delegation 前置门 |
| `overlay` | 可选质量约束，不替代 owner |

### 2.4 Explicit Command

Command 是用户显式进入的模式，不是默认 skill owner。

本次边界：

- `autopilot` 保留为 command。
- 不修改 autopilot stub。
- 不把 autopilot 改成默认执行协议。
- 不用 autopilot 的 owner 设计反推默认 skill 体系。

## 3. Concrete Refactor Plan

### Phase 1: Establish Four-Step Protocol Truth

修改目标：

- `skills/SKILL_FRAMEWORK_PROTOCOLS.md`
- `scripts/skill-compiler-rs/src/main.rs`
- `docs/framework_profile_contract.md`
- `scripts/router-rs/src/framework_profile.rs`

动作：

1. 将 `Detect -> Plan -> Execute -> Verify` 改成 canonical `规范 -> 计划 -> 实施 -> 验证`。
2. 明确该协议是 runtime/route 默认协议，不是 skill。
3. skill compiler 的 quick checklist 改成四步协议语言。
4. profile contract 中从 `execution_controller_contract` 语义迁移到 `execution_protocol_contract`。

兼容策略：

- 可以先保留旧字段名作为 compatibility projection。
- 新语义必须移除 `primary_owner: execution-controller-coding`。
- 新 contract 只表达协议、证据、continuity，不表达 controller owner。

### Phase 2: Demote `execution-controller-coding`

修改目标：

- `skills/execution-controller-coding/SKILL.md`
- generated routing artifacts after compiler run

动作：

1. 移除 `gsd/get shit done/推进到底/autopilot` trigger hints。
2. 移除 `$autopilot` 是 controller alias 的描述。
3. `session_start: required` 改为 `n/a` 或至少 `preferred` 以下。
4. `routing_priority: P0` 降级，避免默认抢 L0。
5. 描述收窄为“显式 supervisor / shared continuity / 多 lane 集成收口”。

保留触发：

- `.supervisor_state.json`
- `共享 continuity`
- `多 lane 集成`
- `长运行周期`
- `主线程集成`
- `integration supervisor`

禁止触发：

- “推进到底”
- “别停”
- “直接干完”
- “给我验证证据”
- `gsd`
- `get shit done`
- `autopilot`

### Phase 3: Move GSD/Completion Semantics Into Route Context

修改目标：

- `scripts/router-rs/src/main.rs`

动作：

1. 删除或停用 `gsd_execution_markers()` 对 `execution-controller-coding` 的 score boost。
2. 将 completion/continue markers 改成 route context，不改变 owner。
3. `subagent-delegation` 不再因为 `gsd_posture` 被压分。
4. controller 只在真实 supervisor markers 出现时加分。

目标行为：

| Query Signal | Expected Behavior |
|---|---|
| “按 PRD 直接实现并验证” | 选最窄 domain owner，带 `verification_required` |
| “推进到底，别停” | 不选 controller，只增强执行闭环 |
| “维护 .supervisor_state.json，多 lane 集成” | 可以选 controller |
| “这个 bug 根因未知” | 先走 evidence/debug gate |

### Phase 4: Reframe `anti-laziness`

修改目标：

- `skills/anti-laziness/SKILL.md`

动作：

1. 移除 `gsd/get shit done` 触发。
2. 从“强执行姿态”改成“验证阶段质量 overlay”。
3. `session_start` 不应默认常驻；除非出现明确质量风险。
4. 只在以下信号触发：
   - 无证据完成声明。
   - 反复失败但不换策略。
   - 明显猜测。
   - 用户明确说“别糊弄 / 严格落实 / 没有验证”。

### Phase 5: Reduce Core Surface

修改目标：

- `skills/SKILL_TIERS.json`
- `configs/framework/FRAMEWORK_SURFACE_POLICY.json`
- generated manifests

动作：

1. 分离“framework core capability”和“default loaded skills”。
2. 从 default/core 中移除执行姿态 skill。
3. core 只保留真正的 framework gates：
   - routing framework support
   - source gate
   - artifact gate
   - evidence gate
   - delegation gate
4. domain owner 默认不常驻，靠 route 精确命中。

目标：

- 默认面更小。
- skill 更像能力库，不像常驻人格集合。
- execution protocol 由 runtime 保证，不靠 core skill 堆叠。

### Phase 6: Update Tests

修改目标：

- `tests/routing_eval_cases.json`
- `tests/routing_route_fixtures.json`
- router scoring unit tests

需要新增/修改的测试：

1. `推进到底` 不应选 `execution-controller-coding`。
2. `给验证证据` 不应选 `anti-laziness`，除非有质量风险信号。
3. 普通实现任务应选最窄 domain owner，并带 verification context。
4. `.supervisor_state.json` / shared continuity 才能触发 controller。
5. `autopilot` command 相关测试不在本次修改范围内。

## 4. Non-Goals

本次不做：

- 不修改 Codex user-skill 安装面的 `autopilot/SKILL.md`。
- 不把 `autopilot` 合并进默认执行协议。
- 不用 `autopilot` 修复默认 skill 体系。
- 不新增一个 `gsd` skill。
- 不新增另一个 controller。
- 不用更多 overlay 弥补体系复杂度。

## 5. Migration Order

推荐顺序：

1. 先改协议真源和 route context 设计。
2. 再降级 `execution-controller-coding`。
3. 再清理 router scoring。
4. 再收窄 `anti-laziness`。
5. 再调整 tiers/default surface。
6. 最后 regenerate routing artifacts。
7. 跑 focused routing tests。

不要先改 generated files。

## 6. Validation Plan

基础验证：

```bash
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
  --health-manifest skills/SKILL_HEALTH_MANIFEST.json \
  --apply
```

Routing 验证：

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml routing_eval_report_matches_expected_baseline
```

建议补充手工 route cases：

```text
根据既定 PRD 直接实现 React 页面，并给验证证据。
这个仓库的修复继续推进到底，别停。
这个任务需要维护 .supervisor_state.json，并集成多个 lane 的结果。
这个复杂任务现在报错了，先查根因，不要直接改。
```

预期：

- 前两个不应触发 `execution-controller-coding`。
- 第三个可以触发 `execution-controller-coding`。
- 第四个应先触发 evidence/debug gate。

## 7. Success Criteria

重构完成后应满足：

1. 用户不说 `gsd`，系统也默认按 `规范 -> 计划 -> 实施 -> 验证` 执行。
2. `execution-controller-coding` 不再是普通执行任务的默认中枢。
3. `autopilot` 仍是独立 command，不污染默认 skill owner 体系。
4. route 选择最窄 owner，执行闭环由 runtime protocol 保证。
5. `anti-laziness` 只处理质量风险，不承担默认执行姿态。
6. core/default surface 明显缩小。
7. generated routing artifacts 与手写 skill/frontmatter 没有 drift。

## 8. Finding Mapping

| Finding | Plan Section |
|---|---|
| 1, 3 | Phase 2 |
| 5, 11 | Phase 1 |
| 7, 12 | Phase 3 |
| 8 | Phase 1 |
| 9, 14 | Phase 5 |
| 2, 4, 6, 10, 13 | Out of scope for this pass because `autopilot` stays a separate manual command |

## 9. Final Shape

最终系统应像这样工作：

```text
User request
  -> Runtime applies four-step protocol by default
  -> Route detects gates and context
  -> Route selects narrowest domain owner
  -> Optional overlay only when risk exists
  -> Implementation proceeds
  -> Verification evidence closes the loop
```

不是：

```text
User says "推进到底"
  -> execution-controller-coding
  -> GSD posture
  -> controller becomes default owner
```

而是：

```text
Any actionable task
  -> 规范
  -> 计划
  -> 最窄 owner 实施
  -> 验证
```
