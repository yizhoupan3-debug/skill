---
name: architect-review
description: |
  Review software architecture, system design, and major structural code changes
  with focus on module boundaries, scalability, maintainability, reliability and
  long-term evolution. Covers Clean Architecture, microservices, event-driven
  systems, and DDD. Use proactively when the user asks for 架构评审、系统设计、
  技术选型、服务拆分、重构方案、边界划分、架构风险, or tradeoff analysis for major
  structural decisions rather than local code-style feedback.
metadata:
  version: "2.1.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - architecture
    - system-design
    - clean-architecture
    - ddd
    - microservices
framework_roles:
  - detector
  - planner
framework_phase: 1
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: false
  consumes_execution_items: false
  emits_verification_results: false
risk: low
source: local
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 架构评审
  - 系统设计
  - 技术选型
  - 服务拆分
  - 重构方案
  - 边界划分
  - 架构风险
  - 系统 review
  - 设计 review
  - architecture review
  - architecture
  - system design
  - clean architecture
  - ddd
---

# architect-review

This skill owns software architecture review: evaluating system design, module boundaries, scalability, and long-term evolution.

## Framework compatibility

This skill's architecture findings should be **mappable to the shared
finding-driven framework** while staying an architecture owner. Keep
tradeoffs, impact, and recommendation clarity, and add stable finding IDs when
the output may feed a downstream planning or refactoring step.

## When to use

- Reviewing system architecture or major design changes
- Evaluating scalability, resilience, or maintainability impact
- Assessing architecture conformance to patterns and principles
- Providing architecture guidance for complex systems
- Reviewing repo-level implementation quality when the main concern is structural risk
- Best for requests like:
  - "架构评审一下这个设计"
  - "帮我做技术选型分析"
  - "这个服务拆分方案合理吗"
  - "重构方案的 tradeoff 分析"
  - "做一次系统 review，重点看架构风险"

## Do not use

- Small code reviews without architectural implications → use `$code-review`
- Changes limited to a single module with no structural impact
- Missing system context that prevents meaningful design evaluation
- The task is local code style → use `$coding-standards`

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
