---
name: execution-controller-app
description: |
  Explicit APP-wide orchestration lane for production-grade full-stack app audits,
  contract/UI/test synchronization, and `.app_supervisor_state.json` continuity.
  Use only when the user asks for whole-app orchestration across frontend, backend,
  and verification surfaces. Single-stack app work should route to the narrowest
  domain owner instead.
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
short_description: Master orchestrator for production-grade app optimization, refactor, and full-stack verification
trigger_hints:
  - APP一键优化
  - 全栈重构
  - 跨栈协同
  - 前后端联调优化
  - 自动化测试闭环
  - 性能全链路调优
  - 全栈安全审计
  - APP深度体检
  - 深度核查前端/后端/测试
  - 系统级 APP 优化
framework_roles:
  - orchestrator
  - supervisor
  - quality-gatekeeper
framework_phase: runtime-orchestration
framework_contracts:
  emits_findings: true
  consumes_findings: true
  emits_execution_items: true
  consumes_execution_items: true
  emits_verification_results: true
metadata:
  version: "3.1.1"
  platforms: [codex, web-app, hybrid-app]
  tags:
    - app-orchestration
    - full-stack
    - aesthetic-engineering
    - api-contract
    - tdd
risk: high
source: local
allowed_tools:
  - shell
  - git
  - python
  - node
  - browser
approval_required_tools:
  - git push
  - gui automation
  - destructive shell
filesystem_scope:
  - repo
  - .app_supervisor_state.json
  - artifacts
network_access: conditional
artifact_outputs:
  - SESSION_SUMMARY.md
  - NEXT_ACTIONS.json
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once
---

# execution-controller-app

`execution-controller-app` is an explicit APP-wide orchestration lane for production-grade optimization work. It owns tasks where frontend quality, backend contracts, and test closure must move together under one orchestration layer.

This skill is **not** part of the default route surface. It is the APP ecosystem orchestration lane when the request explicitly needs whole-app coordination.

## When to use

- 用户明确要求 **APP 全局治理 / 全栈体检 / 跨栈一致性修复 / 一键优化**
- 如果 APP 全局优化信号与通用复杂执行信号同时出现，优先使用本 skill，而不是 [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)
- 需要同时协调 Frontend、Backend、Testing，而不是只改单层
- 需要把 UI 升级到 Premium / WOW，同时同步后端 contract 与测试闭环
- 需要显式维护 `.app_supervisor_state.json` 做 SCR / checkpoint / 恢复
- 需要把 API、schema、frontend rendering、integration/E2E evidence 放进同一执行链
- 需要主线程极简，细节下沉到 sidecars、本地 supervisor 队列、artifact 或日志

## Do not use

- 只是单一前端视觉优化 → use [`$frontend-design`](/Users/joe/Documents/skill/skills/frontend-design/SKILL.md)
- 只是普通 APP 单层任务，或只说“做完/推进/验证” → route to the narrowest domain owner plus runtime verification context
- 只是前端代码规范/组件重构 → use [`$coding-standards`](/Users/joe/Documents/skill/skills/coding-standards/SKILL.md) frontend-quality reference
- 只是后端分层或接口实现 → use [`$node-backend`](/Users/joe/Documents/skill/skills/node-backend/SKILL.md)
- 只是 API / OpenAPI 设计 → use [`$api-design`](/Users/joe/Documents/skill/skills/api-design/SKILL.md)
- 只是补测试或单独走 TDD → use [`$test-engineering`](/Users/joe/Documents/skill/skills/test-engineering/SKILL.md) or [`$tdd-workflow`](/Users/joe/Documents/skill/skills/tdd-workflow/SKILL.md)
- 任务不是 APP 生态，而是通用复杂执行编排 → use [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md)
- 根因未知、当前首要任务是定位故障 → use [`$systematic-debugging`](/Users/joe/Documents/skill/skills/systematic-debugging/SKILL.md)

## Primary operating principle

1. route each stack slice to the narrowest valid owner
2. keep one APP-level objective across UX, contract, logic, and verification
3. enforce contract-first backend changes and verification-first risky rewrites
4. keep the main thread short and integration-focused
5. derive bounded APP sidecars before runtime branching and preserve orchestration discipline even when runtime delegation is degraded

## Core execution model

### Frontend lane

Coordinate [`$frontend-design`](/Users/joe/Documents/skill/skills/frontend-design/SKILL.md), [`$coding-standards`](/Users/joe/Documents/skill/skills/coding-standards/SKILL.md) frontend-quality reference, and the active framework skill.

Default quality bar:

- premium direction such as Bento UI / Glassmorphism / Mesh Gradients when requested
- oklch-first color thinking when supported
- concise files, RORO params, Early Return
- motion only when it improves perceived quality without hurting performance or accessibility

### Backend lane

Coordinate [`$node-backend`](/Users/joe/Documents/skill/skills/node-backend/SKILL.md), [`$api-design`](/Users/joe/Documents/skill/skills/api-design/SKILL.md), and deeper backend specialists as needed.

Default quality bar:

- contract-first changes
- thin handlers, clear controller/service separation
- business logic below transport layer
- stable error/status contracts
- synchronized API, schema, and frontend-consumption updates

### Testing lane

Coordinate [`$test-engineering`](/Users/joe/Documents/skill/skills/test-engineering/SKILL.md), [`$tdd-workflow`](/Users/joe/Documents/skill/skills/tdd-workflow/SKILL.md), and [`$playwright`](/Users/joe/Documents/skill/skills/playwright/SKILL.md) when browser-grounded evidence is required.

Default quality bar:

- prefer Unit / Integration first
- require failing tests before high-risk logic rewrites
- collect compact evidence such as `diff_summary.md` and `test_report.log` for deep optimization passes

## Input contract

Normalize before dispatch:

- app objective
- current phase
- stack surfaces in scope
- forbidden scope
- constraints and quality bars
- acceptance criteria
- evidence required for sign-off
- blockers and assumptions

## Automation lane

Required moves:

1. restore or initialize `.app_supervisor_state.json`
2. audit UI shell, API edges, contract surfaces, and test posture
3. check [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md) before splitting work
4. build a bounded frontend / backend / verification sidecar plan before runtime branching
5. attempt real spawning when the runtime allows it; otherwise queue the same plan locally
6. validate integration before phase exit and final sign-off
7. reroute immediately if the task becomes debugging, performance, accessibility, security, or architecture dominated

Maintain `.app_supervisor_state.json` with at least:

- `app_health` → `lighthouse`, `security_audit`, `coverage_metric`
- `stack_sync` → `api_hash`, `db_schema_hash`, `frontend_checksum`
- `delegation_plan_created`, `spawn_attempted`, `spawn_block_reason`, `fallback_mode`
- `progress_log`, `active_phase`, `delegated_sidecars`, `local_supervisor_queue`
- `artifacts`, `blockers`, `next_actions`, `rollback_point`

Default phase chain:

`Planning -> Profiling -> Executing -> Verifying -> Integrating`

Final sync metrics:

- `performance_v8`
- `security_score`
- `test_coverage`
- notable UX / contract / regression risks still open

## Runtime-policy adaptation

Once the APP split is clear, derive the sidecar plan before checking the runtime branch.

If runtime delegation is allowed:

- attempt real spawning from the pre-built APP sidecar plan
- use bounded sidecars for inspection, implementation, and verification
- keep orchestration, state, and final synthesis local

If runtime delegation is blocked:

- switch to local-supervisor APP mode
- preserve the same pre-built split and output contracts
- keep details in `.app_supervisor_state.json`, artifacts, and compact summaries

## Boundary logic

- `execution-controller-app` owns **APP ecosystem coherence** across UX, contract, backend layering, and test closure.
- [`$execution-controller-coding`](/Users/joe/Documents/skill/skills/execution-controller-coding/SKILL.md) owns **generic complex execution orchestration** beyond APP-wide optimization.
- Domain skills still own implementation; this skill owns orchestration, stack sync, quality gates, and final integration judgment.

## Hard constraints

- do not trade accessibility or performance for polish
- do not mutate backend interfaces without contract updates
- do not rewrite risky logic without verification coverage
- do not claim one-click optimization without cross-stack evidence
- do not let APP-wide orchestration collapse into isolated layer edits with no final sync

## Trigger examples

- “启动全栈总控，对我现在的 App 进行深度一键优化。”
- “深度核查现在的全链路，把 UI 变高端，后端变解耦，加上完整测试。”
- “这是一个高负载的 APP 深度体检任务，请维持 SCR 状态并指挥全栈协同。”
- “帮我做一次生产级 APP 重构，前后端契约和测试一起拉齐。”
