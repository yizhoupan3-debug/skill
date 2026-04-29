---
name: test-engineering
description: |
  Choose the right test layer, write maintainable tests, and stabilize flaky behavior.
  Use when the user asks to add tests, fix flaky tests, design test strategy, improve mocks and fixtures, or phrases like “补测试”, “写 pytest”, “Vitest/Jest 怎么写”, “测试总是 flaky”, “fixture 和 mock 怎么设计”.
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - testing
    - pytest
    - jest
    - vitest
    - flaky
risk: medium
source: local
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 补测试
  - 写 pytest
  - Vitest
  - Jest 怎么写
  - 测试总是 flaky
  - fixture 和 mock 怎么设计
  - fix flaky tests
  - design test strategy
  - improve mocks
  - fixtures
allowed_tools:
  - shell
  - git
  - python
  - node
approval_required_tools:
  - git push

---

- **Dual-Dimension Audit (Pre: Test-Case/Logic, Post: Coverage-Metric/Mutation-Score Results)** → runtime verification gate
# test-engineering

This skill owns practical testing work: choosing the right test layer, writing maintainable tests, stabilizing flaky behavior, and improving regression protection.

## When to use

- The user wants to add, revise, or stabilize automated tests
- The task involves pytest, unittest, Jest, Vitest, Testing Library, Playwright tests, mocks, fixtures, or factories
- The user wants a testing strategy for unit/integration/e2e coverage
- The task involves flaky tests, weak assertions, poor test isolation, or regression protection
- **Default priority**: Any testing request ("补测试", "写测试", "add tests") defaults to this skill unless the user **explicitly asks for TDD / RED-GREEN-REFACTOR** → then use `$tdd-workflow`
- Best for requests like:
  - "给这个模块补测试"
  - "帮我写 pytest / vitest / jest"
  - "这个测试为什么总是 flaky"
  - "fixture 和 mock 应该怎么设计"

## Do not use

- The main request is explicitly about RED-GREEN-REFACTOR process → use `$tdd-workflow`
- The main task is root-cause debugging of an application bug before test design → use `$systematic-debugging`
- The task is browser automation itself rather than test architecture or test code → use the built-in browser/browser-use capability
- The task is a framework/domain implementation task where tests are only incidental

## Task ownership and boundaries

This skill owns:
- test-layer selection and strategy
- test implementation and refactoring
- fixtures, mocks, fakes, factories, and test data patterns
- flaky-test diagnosis and stabilization
- regression test coverage design

This skill does not own:
- TDD methodology by itself
- generic app debugging with no testing focus
- browser automation outside a testing context
- the underlying domain implementation unless explicitly requested

If the task shifts to adjacent skill territory, route to:
- `$tdd-workflow`
- `$systematic-debugging`
- built-in browser/browser-use capability

## Required workflow

1. Confirm the task shape:
   - object: module, endpoint, component, workflow, bug, or regression risk
   - action: add tests, fix flaky behavior, redesign test strategy, refactor tests
   - constraints: framework, runner, CI environment, mockability, speed targets
   - deliverable: test code, strategy, failure analysis, or stabilization plan
2. Choose the narrowest test layer that still gives confidence.
3. Prefer behavior-focused assertions.
4. Eliminate avoidable nondeterminism before retries or sleeps.
5. Validate with focused runs first.

## Core workflow

### 1. Intake
- Identify the target framework and runner.
- Clarify whether the goal is missing coverage, regression protection, flaky repair, or maintainability improvement.
- Inspect existing helpers, fixtures, and test conventions before adding a new pattern.

### 2. Execution
- Choose unit vs integration vs end-to-end deliberately.
- Build stable fixtures/factories with explicit setup and teardown.
- Mock only the boundaries that need isolation; avoid mocking the whole world.
- Make assertions reflect user-visible or contract-visible behavior when possible.

### 3. Validation / recheck
- Run the narrowest affected tests first.
- Re-check determinism, isolation, and failure readability.
- Call out remaining gaps if full confidence would require a higher test layer.

## Output defaults

Default output should contain:
- test scope and chosen layer
- what was added/fixed and why
- validation result and remaining gaps

Recommended structure:

````markdown
## Test Summary
- Target: ...
- Layer: unit / integration / e2e

## Test Changes
- Added / fixed: ...
- Fixture / mock strategy: ...

## Validation / Remaining Risk
- Ran: ...
- Still missing: ...
````

## Hard constraints

- Do not default to e2e when a lower layer can cover the behavior well.
- Do not over-mock behavior that should be covered by a real integration boundary.
- Do not paper over flaky tests with blind retries or long sleeps by default.
- Do not couple assertions tightly to internal implementation details unless that is the contract.
- In this repository, follow [`RTK.md`](/Users/joe/Documents/skill/RTK.md) for broad `pytest` / `cargo test` / similar validation runs when the output is high-volume and raw fidelity is not the immediate need.
- If a test gap remains, say exactly what confidence is still missing.
- **Superior Quality Audit**: For high-fidelity testing frameworks, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples

- "Use $test-engineering to add regression tests for this bug."
- "帮我给这个 API 写 pytest，并设计 fixtures。"
- "这个 Vitest/Jest 测试很 flaky，帮我稳定下来。"
- "强制进行测试工程深度审计 / 检查用例覆盖率与变异测试结果。"
- "Use the runtime verification gate to audit this test suite for coverage-integrity idealism."
