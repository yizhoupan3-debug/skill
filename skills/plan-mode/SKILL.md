---
name: plan-mode
description: |
  Cursor Plan / 策划文档闸门 owner：先调研与 review，再产出可验收 todo 与修订闭环，最后用 `/gitx plan` 对照计划收口。
  Use at 每轮对话开始 / first-turn / conversation start when the user wants Cursor Plan mode、Plan 模式、策划文档闸门、可验收 todo、
  subagent 审 plan、或明确要走「计划→实现→验证→对照 git 收口」而不是直接堆代码。
  Aligns execution-item / verification shapes with `skills/SKILL_FRAMEWORK_PROTOCOLS.md`；continuity 分层见 `docs/harness_architecture.md`。
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: preferred
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - Cursor Plan
  - Plan 模式
  - 策划文档闸门
  - 可验收 todo
  - subagent 审 plan
  - gitx plan 收口
  - 计划对照实际
  - plan revision round
  - 独立上下文 review 计划
metadata:
  version: "1.0.0"
  platforms: [codex, cursor]
  tags: [plan, cursor-plan, workflow, gate, closeout]
---

# plan-mode

把「写计划」当成**可闸门、可验收、可对照 Git 收口**的产物，而不是一次性 prose。计划文件建议落在 **`.cursor/plans/`**（Cursor 默认）并在仓库内用 **`docs/plans/`** 做可读镜像或摘要指针，避免只在 IDE 私有目录里丢稿。

## When to use

- 用户要在 **Cursor Plan** / **Plan 模式** 下先把范围、风险、验证路径钉死，再允许大规模改动。
- 用户提到 **策划文档闸门**、**可验收 todo**、要用 **独立上下文 subagent** 审计划初稿。
- 用户明确要走：**计划获批 → 实现 + 测试通过 → 固定用 `/gitx plan` 做计划 vs 实际** 的收口（`/gitx plan` 与 `/gitx` 等价，见 `skills/gitx/SKILL.md`）。
- **每轮对话开始 / first-turn / conversation start**：任务看起来像「先出高质量计划/蓝图」且后续实现依赖该计划的验收标准。

## Do not use

- 用户只要直接实现、明确禁止前置规划或策划文档。
- 纯 **skill 路由系统 / routing registry / manifest** 治理与 miss repair → `skill-framework-developer`（本 skill 不把框架路由当主触发域）。
- 单一极小改动（单文件几行）且计划成本明显高于收益 → 直接最小 delta + 验证即可。

## Workflow（六步）

1. **调研 + review 先于计划**：在写结构化计划前，完成域内必要的深读、检索或代码定位；需要对抗性或跨模块 review 时按宿主规则拆 reviewer，再把结论**收敛进计划**，而不是反过来用计划代替证据。
2. **Todo 必须可验收**：每条 todo 绑定明确的 **完成定义** 与 **验证手段**（命令、测试、diff 范围或人工检查点）；表格字段可与 `skills/SKILL_FRAMEWORK_PROTOCOLS.md` 中的 execution item / verification result 思想对齐（不必冗长复制 schema）。
3. **初稿后：独立上下文 subagent 审 plan**：第一轮计划草案完成后，用 **与主线程隔离的 reviewer subagent**（独立上下文；或宿主等价机制）只读计划与已有证据，输出 findings；主线程合并后再改计划正文。
4. **一轮修订**：基于 reviewer findings **最多一轮**集中修订计划（合并冲突意见、删掉不可验证条目、补齐验证命令）；避免无尽「计划迭代」阻塞执行。
5. **人工交接带 delta**：提交给用户审批时，附带 **相对上一版的 delta**（改了哪些验收标准、哪些 todo、哪些风险假设），而不是全文重贴。
6. **获批且实现与测试通过后：`/gitx plan` 固定收口**：按计划 vs 实际逐项对照（scope、验证、未做项的原因），再按 `skills/gitx/SKILL.md` 执行 Git 一条龙收口（别名 **`/gitx plan`** 与 **`/gitx`** 同契约）。

## Continuity 与工件

- **分层与 hook**：控制面、证据与续跑注入边界以 `docs/harness_architecture.md` 为准；不要在 skill 正文发明第二套账本格式。
- **计划落盘**：权威草稿/链接建议 `.cursor/plans/`；仓库协作或审计需要的摘要可复制或同步到 `docs/plans/`，与仓库内其它计划文档同一叙事。

## Related

- `skills/SKILL_FRAMEWORK_PROTOCOLS.md` — 讨论 → 规划 → 执行 → 验证 与最小 findings / execution / verification 形状。
- `skills/gitx/SKILL.md` — `/gitx` / `/gitx plan` 收口与深度 review checklist。
