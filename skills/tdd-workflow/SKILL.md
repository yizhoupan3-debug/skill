---
name: tdd-workflow
description: |
  Run a Test-Driven Development workflow centered on the RED-GREEN-REFACTOR
  loop with failing tests first, minimal implementation, and safe refactoring.
  Use when the user wants 先写测试、补回归保护、按测试驱动修 bug、增量实现功能、
  or explicit behavior-first development instead of coding first and testing
  later.
allowed_tools:
  - shell
  - git
  - python
  - node
approval_required_tools:
  - git push
metadata:
  version: "2.1.0"
  platforms: [codex]
  tags:
    - tdd
    - testing
    - regression
    - red-green-refactor
framework_roles:
  - executor
  - verifier
framework_phase: 2
framework_contracts:
  emits_findings: false
  consumes_findings: true
  emits_execution_items: false
  consumes_execution_items: true
  emits_verification_results: true
risk: low
source: local
routing_layer: L1
routing_owner: overlay
routing_gate: none
session_start: n/a
trigger_hints:
  - 先写测试
  - 补回归保护
  - 按测试驱动修 bug
  - 增量实现功能
  - testing later
  - tdd
  - testing
  - regression
  - red green refactor
---

# TDD Workflow

This skill owns execution when the user wants the work to proceed through
failing tests first, then minimal implementation, then cleanup.

## When to use

- The user asks for TDD, tests first, or regression-first implementation
- The task is a bug fix where a failing test should capture the defect first
- The task involves complex logic that benefits from behavior-first development
- The user wants safer refactors with explicit regression coverage
- Best for requests like:
  - "先写测试再实现"
  - "用 TDD 修这个 bug"
  - "给这个改动先补 failing test"

## Do not use

- The user wants to add or improve tests without an explicit TDD loop → use `$test-engineering`
- The user says "补测试" without requiring RED-GREEN-REFACTOR → use `$test-engineering`
- The task is exploratory spike work where the behavior is still unknown
- The task is mainly visual polish or layout tuning with weak automated-test value
- The user explicitly wants direct implementation without a TDD loop
- There is no meaningful automated test surface in the repo

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
