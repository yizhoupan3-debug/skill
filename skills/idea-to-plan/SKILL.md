---
name: idea-to-plan
description: |
  内核级战略编排器 (L-1)：把模糊意图转成高成熟度计划、路线比较、决策日志与 Duo-Doc 交付。
  适用于“意图到计划 / 蓝图设计 / 方案探索 / 先调研再给计划 / 先别写代码 / 先探索现状再提方案 / critical files / Pilot 验证 / 科研自动化 / outline.md / code_list.md / 主线程极简”。
  每轮对话开始 / first-turn / conversation start，如任务仍处于模糊规划阶段，必须优先检查此控制层。
routing_layer: L-1
routing_owner: "@strategic-orchestrator"
routing_gate: delegation
routing_priority: P0
session_start: required
short_description: Turn ambiguous ideas into evidence-backed plans with branch routing and compressed context
trigger_hints:
  - idea-to-plan
  - 战略编排
  - 意图到计划
  - 先调研再给计划
  - 先别写代码
  - outline.md
  - code_list.md
  - 先探索现状再提方案
  - 先探索代码库再出方案
  - 先做方案
  - 技术方案
  - 路线比较
  - 风险评估
  - critical files
  - explore-plan
  - 科研自动化
  - 试点验证
  - Pilot
  - Duo-Doc
  - 蓝图设计
framework_roles:
  - strategic-orchestrator
  - planner
  - pilot-router
framework_phase: pre-execution
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: true
metadata:
  version: "2.4.0"
  platforms: [codex]
  tags:
    - strategy
    - planning
    - pilot
    - automation
    - subagent
    - duo-doc
    - first-turn-routing
    - context-compression
    - critical-files
    - explore-plan
risk: medium
source: local
allowed_tools:
  - shell
  - git
  - python
  - web
approval_required_tools:
  - git push
  - network install
filesystem_scope:
  - repo
  - .supervisor_strategy.json
  - artifacts
network_access: conditional
artifact_outputs:
  - outline.md
  - code_list.md
  - assumptions.md
  - open_questions.md
  - decision_log.md
  - plan_rubric.md
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once
---

# idea-to-plan

`idea-to-plan` is the strategic owner for work that is still ambiguous. Its job is to turn fuzzy intent into a decision-ready plan with explicit tradeoffs, assumptions, open questions, and handoff artifacts, not to start coding early.

This skill is a **repo-local planning delta**, not the owner of the host's generic planning protocol. It should add this repository's planning artifacts, reroute rules, and owner boundaries on top of the host's native plan-mode behavior.

## When to use

- 用户只有方向，没有成熟 plan
- 需要比较多个路线并收敛
- 需要先调研、澄清、比较路线，再决定怎么做
- 需要先探索代码库现状、现有模式、关键文件，再给方案
- 需要把模糊意图变成 `outline.md`、`decision_log.md`、`assumptions.md`、`open_questions.md`、`plan_rubric.md` 和 `code_list.md`
- 每轮对话开始 / first-turn / conversation start，主问题仍然是“到底该怎么做”

## Do not use

- 已有成熟 PRD / plan，只差实现 → [`$plan-to-code`](/Users/joe/Documents/skill/skills/plan-to-code/SKILL.md)
- 战略路线已经确定，只差拆执行清单 → [`$checklist-planner`](/Users/joe/Documents/skill/skills/checklist-planner/SKILL.md)
- 已进入复杂执行编排 → [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)
- 根因定位或局部修 bug

## Output Contract

最低交付物：

- `outline.md`
- `assumptions.md`
- `open_questions.md`
- `decision_log.md`
- `plan_rubric.md`
- `code_list.md`

如复杂度需要，可补：

- `pilot_matrix.md`
- `.supervisor_strategy.json`

每份计划至少要回答：

- 目标与非目标分别是什么
- 至少 2 条候选路线如何比较，为什么选当前路线
- 当前 plan 依赖哪些显式 assumptions
- 还有哪些 open questions 需要澄清后才能执行
- 哪些风险 / stop conditions 会阻止直接进入实现
- 哪些内容属于后续执行清单而不是战略计划正文

主线程只保留：

- normalized target
- compared options
- chosen route and why
- handoff owner

计划验收最低标准：

- `outline.md` 给出问题重述、范围、路线比较、推荐路径与高层阶段
- `decision_log.md` 记录为什么淘汰其它路线，而不是只写最终结论
- `assumptions.md` 只保留当前决策真正依赖的前提，不混入执行细节
- `open_questions.md` 列出需要用户或外部证据澄清的事项
- `plan_rubric.md` 明确计划通过标准，便于后续审计
- `code_list.md` 只在战略路径确定后承接实现拆解，不得反客为主替代战略 plan

## Primary Operating Principle

这个 owner 只补三类 repo-local 信息：

1. 本仓库需要的 planning artifacts 与验收面
2. `idea-to-plan` 与 `checklist-planner`、`plan-to-code` 的边界
3. strategic planning 在本仓库里的 reroute / handoff 规则

## Reroute Rules

- 已有明确实现面：reroute to `$plan-to-code`
- 战略路径已定、主需求是执行拆解：reroute to `$checklist-planner`
- 已进入高负载执行：reroute to `$execution-controller-coding`
- 需要并行探索：先过 `$subagent-delegation`

## References

- [references/DESIGN.md](references/DESIGN.md)
- [references/AUDIT.md](references/AUDIT.md)
