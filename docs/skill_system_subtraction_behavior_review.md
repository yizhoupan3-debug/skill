# Skill System Subtraction / Behavior-Driven Review

日期：2026-04-25
范围：在 Codex-only 前提下，复核 skill 系统是否已经从“入口/控制器驱动”收敛为“行为协议驱动”，并识别剩余多余入口与不必要抽象。

## 1. 一句话结论

系统已经基本完成从 `gsd` / controller / autopilot 触发驱动，转向默认行为协议驱动：

```text
规范 -> 计划 -> 实施 -> 验证
```

但还没有完全完成减法。当前最大剩余问题不是能力不够，而是“默认协议已经很清楚，但控制面仍保留了偏多的入口、兼容字段和策略文件”，导致行为模型清晰，系统表面积仍偏大。

## 2. 第一性原理判断

一个最小 skill 系统只需要回答四个问题：

1. 用户要做什么：对象、动作、约束、交付物、成功标准。
2. 谁最适合做：最窄 owner，必要 gate，最多一个 overlay。
3. 怎么做最小：最小 delta，不扩大抽象。
4. 怎么证明完成：测试、命令、截图、产物或明确 blocker。

按这个标准，当前系统的正确方向是：

- Runtime protocol 负责默认执行闭环。
- Route context 负责承载执行压力、验证压力、delegation 候选等上下文。
- Skill owner 只负责领域能力选择。
- Explicit command 只在用户显式进入时生效。

这个边界已经在文档和 runtime 中成形，但还需要继续压缩控制面。

## 3. 已经清晰的部分

### 3.1 默认行为协议已进入 runtime

`skills/SKILL_ROUTING_RUNTIME.json` 的 checklist 已经把默认动作写成：

- `规范`: 先抽取 object / action / constraints / deliverable / success criteria。
- `计划`: 先检查 source / artifact / evidence / delegation gate。
- `计划`: 选择最窄 domain owner，最多加一个 overlay。
- `实施`: 走最小 route delta，不扩大抽象。
- `验证`: 用测试、命令、截图、产物或 blocker 关闭。
- Completion pressure 只改变 route context，不改变 selected owner。

这说明默认协议已经不依赖 `gsd`、`推进到底`、`别停` 这类姿态词触发。

### 3.2 Controller 已降级为真实 orchestration owner

`execution-controller-coding` 已经从默认执行人格，降级为显式 supervisor / shared continuity / 多 lane 集成 owner：

- `routing_priority: P2`
- `session_start: n/a`
- trigger 只剩 `.supervisor_state.json`、共享 continuity、多 lane 集成、主线程集成等真实 orchestration 信号。
- skill 文档明确写出 completion-pressure wording 是 route context，不是 controller owner trigger。

这是对的。它不再应该接普通“直接干完 / 给验证证据”的任务。

### 3.3 Anti-laziness 已变成验证质量 overlay

`anti-laziness` 现在是 overlay，不是 owner，也不是默认执行模式：

- `routing_owner: overlay`
- `session_start: n/a`
- trigger 聚焦“别糊弄 / 严格落实 / 没有验证 / 无证据完成”。
- 文档明确 verification pressure 不允许改变 owner。

这符合减法原则：质量约束只能附加，不能抢领域主线。

### 3.4 路由实测符合新语义

实测命令：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '按方案实现这个仓库修复，直接做代码，推进到底，别停，并给我验证证据'
```

结果摘要：

- `selected_skill`: `plan-to-code`
- `overlay_skill`: `null`
- `route_context.execution_protocol`: `four_step`
- `route_context.verification_required`: `true`
- `route_context.evidence_required`: `true`
- `route_context.supervisor_required`: `false`
- `route_context.continue_safe_local_steps`: `true`
- `route_context.route_reason`: `completion_signal_context`

这证明“推进到底 / 别停 / 给验证证据”已经不再把 owner 改成 controller。

另一个实测：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '维护 .supervisor_state.json，多 lane 集成并保持主线程收口'
```

结果摘要：

- `selected_skill`: `execution-controller-coding`
- `route_context.supervisor_required`: `true`
- `route_context.route_reason`: `explicit_supervisor_continuity`

这说明 controller 只在真实 supervisor / continuity / lane 集成信号下接管。

## 4. 仍然不够减法的地方

### 4.1 “讨论-规划-执行-验证”命名与 runtime canonical 不完全一致

用户口径是：

```text
讨论 -> 规划 -> 执行 -> 验证
```

当前 runtime canonical 是：

```text
规范 -> 计划 -> 实施 -> 验证
```

语义基本等价，但名称不完全一致。建议只保留一套公开口径，避免后续文档出现两个四步协议。

推荐收敛为：

```text
讨论/规范 -> 规划/计划 -> 执行/实施 -> 验证
```

或者更彻底地选一个 canonical。若目标是行为驱动和用户可理解性，建议公开层用“讨论-规划-执行-验证”，内部字段仍保留 `four_step`。

### 4.2 Framework profile 字段仍偏多

`docs/framework_profile_contract.md` 的 canonical fields 仍包括大量控制面字段：

- `core_capabilities`
- `optional_capabilities`
- `rules_bundle`
- `skill_bundle`
- `session_policy`
- `tool_policy`
- `approval_policy`
- `loadout_policy`
- `framework_surface_policy`
- `artifact_contract`
- `model_policy`
- `memory_mounts`
- `mcp_servers`
- `workspace_bootstrap`
- `host_capability_requirements`
- `execution_protocol_contract`
- `execution_controller_contract`
- `delegation_contract`
- `supervisor_state_contract`

其中 `execution_controller_contract` 已标成 compatibility projection only，这是好事，但第一性原理看，默认理解系统时不应该还看到这么大的字段面。

建议把 profile 分成两层：

- `kernel_profile`: routing、runtime_protocol、memory、continuity、codex_host_payload。
- `capability_profile`: tools、mcp、loadouts、approval、artifact、delegation、supervisor 等显式 opt-in 能力。

默认文档只讲 kernel，capability 作为附录或生成物。

### 4.3 Loadouts / tiers / surface policy 仍像三套激活系统

当前同时存在：

- `configs/framework/FRAMEWORK_SURFACE_POLICY.json`
- `skills/SKILL_LOADOUTS.json`
- `skills/SKILL_TIERS.json`

三者都在描述 default、explicit opt-in、core、optional、activation 等概念。虽然内容已经比之前干净，但仍然构成认知重复。

第一性原理上，runtime 只需要一个问题：

> 默认路由可见面是什么，哪些需要显式 opt-in？

建议保留一个机器真源，另外两个降级为生成物或调试报告。推荐：

- 真源：`configs/framework/FRAMEWORK_SURFACE_POLICY.json`
- 生成物：`skills/SKILL_TIERS.json`
- 可删除或生成：`skills/SKILL_LOADOUTS.json`

如果短期不删，至少要在文档里明确“谁是 authoring source，谁是 compiled output”。

### 4.4 Core gate 数量仍偏大

`skills/SKILL_TIERS.json` 当前 core 为 16 个：

- `design-agent`
- `doc`
- `execution-controller-app`
- `gh-address-comments`
- `gh-fix-ci`
- `idea-to-plan`
- `openai-docs`
- `pdf`
- `playwright`
- `sentry`
- `skill-framework-developer`
- `slides`
- `spreadsheets`
- `subagent-delegation`
- `systematic-debugging`
- `visual-review`

这些大多合理，但按减法标准，core 应该只保留“默认路由前置门”，不是所有高价值能力。

可疑 core：

- `execution-controller-app`: 更像显式 APP 全局优化 controller，不应默认常驻。
- `design-agent`: 如果只服务“命名产品参考源/品牌 token”，更像 evidence/source gate，但未必需要 core。
- `skill-framework-developer`: 对框架维护很重要，但普通用户任务未必需要默认 core；可以保持高优先 route，而不一定是 default surface。
- `idea-to-plan`: 对模糊任务有价值，但它是战略 owner，不是纯 gate；是否 core 取决于是否允许模糊任务默认先规划。

建议把 core 从 16 压到更硬的门：

- source gates: `openai-docs`, `gh-address-comments`, `gh-fix-ci`, `sentry`
- artifact gates: `doc`, `pdf`, `slides`, `spreadsheets`
- evidence gates: `playwright`, `visual-review`, `systematic-debugging`
- delegation gate: `subagent-delegation`

其余用 route 精准命中即可。

### 4.5 Meta routing 识别太窄，导致本次真实 query 会误选 architect-review

实测：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml --quiet -- route '减法视角和第一性原理review我的skill系统，是否已经清晰的变成行为驱动（讨论-规划-执行-验证），是否还有多余入口和不必要抽象'
```

结果摘要：

- `selected_skill`: `architect-review`
- 原因：命中了 `review`、`系统`，但没有命中 `skill-framework-developer` 的 trigger。

这是当前最明确的 routing gap。`is_meta_routing_task()` 只识别 `skill + 路由/触发/routing/router/route`，而“skill系统 / 多余入口 / 不必要抽象 / 行为驱动 / 第一性原理”没有进入 meta-framework 信号。

建议补一条 regression case：

```json
{
  "id": "skill-framework-subtraction-behavior-review",
  "category": "should-trigger",
  "task": "减法视角和第一性原理review我的skill系统，是否已经清晰的变成行为驱动（讨论-规划-执行-验证），是否还有多余入口和不必要抽象",
  "focus_skill": "skill-framework-developer",
  "expected_owner": "skill-framework-developer",
  "expected_overlay": null,
  "forbidden_owners": ["architect-review"],
  "first_turn": true
}
```

并把 `skill-framework-developer` trigger 扩展为：

- `skill系统`
- `skill system`
- `行为驱动`
- `讨论-规划-执行-验证`
- `多余入口`
- `不必要抽象`
- `减法视角`
- `第一性原理`

或者在 router 中把 meta-framework detector 扩展到：

- `skill` + `系统`
- `skill` + `入口`
- `skill` + `抽象`
- `skill` + `行为驱动`
- `skill` + `第一性原理`

## 5. 减法后的推荐目标形态

建议把系统压成四层，且每层只回答一个问题：

| Layer | 唯一职责 | 不应该做 |
|---|---|---|
| Runtime Protocol | 默认行为闭环 | 不选 owner，不表达人格 |
| Route Context | 承载验证、证据、continuity、delegation 信号 | 不改变 selected owner |
| Skill Registry | 选择最窄 owner / gate / overlay | 不重复全局执行哲学 |
| Explicit Commands | 用户显式进入特殊模式 | 不作为默认路由捷径 |

最终默认链路应该是：

```text
AGENTS.md
-> skills/SKILL_ROUTING_RUNTIME.json
-> route_context(four_step)
-> narrowest owner/gate/overlay
-> skills/<name>/SKILL.md
-> evidence-backed completion
```

## 6. 建议下一步

优先级从高到低：

1. 修复 `skill系统 + 行为驱动/减法/第一性原理` 误路由到 `architect-review` 的问题。
2. 统一公开四步协议命名：要么全用“规范-计划-实施-验证”，要么公开层改成“讨论-规划-执行-验证”。
3. 压缩 core surface，把 `execution-controller-app`、`design-agent`、`skill-framework-developer`、`idea-to-plan` 重新评估为 route 命中能力，而不是默认 core。
4. 明确 `FRAMEWORK_SURFACE_POLICY.json` / `SKILL_LOADOUTS.json` / `SKILL_TIERS.json` 的真源关系，减少三套激活系统的认知重复。
5. 把 `execution_controller_contract` 迁移为显式 compatibility artifact，默认 profile 文档不再展示。

## 7. Final Verdict

行为驱动已经成立，尤其是 completion pressure 不再改变 owner，这个核心方向是对的。

剩余的减法重点不是再加新 skill，也不是再写更多协议，而是：

- 修 routing gap。
- 减 default core。
- 合并 surface/loadout/tier 真源。
- 把 compatibility/controller/profile 叙事继续从默认阅读路径中移出去。

如果继续沿这个方向压缩，系统会更像“轻 runtime + 精准能力库”，而不是“多个中枢互相解释的控制面”。
