---
name: refactoring
description: |
  Plan and execute systematic code refactoring without changing behavior.
  Use when the user asks to 重构代码, 解耦模块, 抽接口, 消除重复, 渐进迁移,
  清理死代码, 降低复杂度, 拆大函数, 拆大类, or modernize legacy code. Best for
  structural improvement with safety checks, not style-only cleanup.
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 解耦模块
  - 抽接口
  - 消除重复
  - 渐进迁移
  - 清理死代码
  - 降低复杂度
  - 拆大函数
  - 拆大类
  - modernize legacy code
  - refactoring
allowed_tools:
  - shell
  - git
  - python
  - node
approval_required_tools:
  - git push
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - refactoring
    - legacy-migration
    - coupling
    - code-structure
    - extract-method
    - dependency-inversion
    - dead-code
risk: medium
source: local
---

# refactoring

This skill owns systematic code refactoring when the primary goal is to improve
code structure, reduce coupling, or modernize legacy code without changing
observable behavior.

## When to use

- The user wants to restructure code: extract methods/classes, move modules, introduce abstractions
- The task involves reducing coupling, breaking circular dependencies, or inverting dependencies
- The user is doing a large-scale legacy modernization or incremental migration (strangler fig, branch by abstraction)
- The task involves dead code removal, feature flag cleanup, or deprecated API replacement
- The user wants to reduce cyclomatic complexity, flatten deep nesting, or simplify control flow
- Best for requests like:
  - "这个文件太大了，帮我拆开"
  - "帮我重构这个模块，降低耦合度"
  - "这段 legacy 代码怎么渐进式迁移"
  - "帮我清理死代码和未使用的导入"
  - "把这些重复逻辑抽成公共函数"
  - "这个类职责太多，帮我拆分"

## Do not use

- The task is only style/naming/formatting fixes → use `$coding-standards`
- The task is continuous improvement philosophy or kaizen overlay → use `$coding-standards`
- The task is architecture-level review without hands-on refactoring → use `$architect-review`
- The task is implementing a plan from scratch (not restructuring existing code) → use `$plan-to-code`
- The task is fixing a bug (behavior change, not structural improvement) → use `$systematic-debugging`
- The task is test design or test refactoring only → use `$test-engineering`
- The task is **single-file language-idiomatic refactoring** (e.g. Python list comprehension, JS destructuring, Go error wrapping simplification) without cross-module structural goals → use the relevant language pro skill

## Task ownership and boundaries

This skill owns:
- Refactoring pattern selection and execution (extract, inline, move, rename, introduce, replace)
- Coupling analysis and dependency graph simplification
- Legacy code modernization strategies (strangler fig, branch by abstraction, parallel run)
- Dead code detection and safe removal
- Refactoring safety verification (test coverage check, behavioral equivalence)
- Complexity reduction (cyclomatic, cognitive, nesting depth)

This skill does not own:
- Code style enforcement (formatting, naming conventions)
- High-level architecture evaluation
- New feature implementation
- Bug fixing
- Test suite design from scratch

## Core workflow

### 1. Assess

- Map the current structure: dependencies, coupling points, complexity hotspots
- Identify the refactoring goal: what structural property should improve
- Check test coverage on the code to be refactored; if insufficient, flag it before proceeding
- Determine whether the refactoring can be done incrementally or requires a big-bang approach

### 2. Plan

- Select the smallest set of refactoring patterns that achieve the goal
- Sequence operations to maintain a passing build after each step
- Identify behavioral invariants that must hold throughout
- Plan verification points: which tests to run after each step

### 3. Execute

- Apply one refactoring pattern at a time
- Run tests after each atomic change to catch regressions immediately
- Preserve git history: prefer small, descriptive commits over one giant diff
- Document any intentional behavior changes separately from structural changes

### 4. Verify

- Run the full test suite after completion
- Compare observable behavior before and after (API contracts, outputs, side effects)
- Measure the structural improvement (coupling, complexity, file size, dependency graph)
- Document what changed and what was preserved

## Output defaults

```markdown
## Refactoring Summary
- Goal: [what structural property improved]
- Scope: [files/modules affected]

## Patterns Applied
1. [pattern] — [what it achieved]

## Safety Verification
- Tests: PASS / FAIL
- Behavioral equivalence: confirmed / caveats

## Metrics (before → after)
- Complexity: ...
- Coupling: ...
- File count/size: ...
```

## Hard constraints

- Never change observable behavior without explicit acknowledgment
- Always verify test coverage before starting; warn if coverage is insufficient
- Prefer incremental, reversible steps over big-bang rewrites
- Do not mix refactoring commits with feature or bugfix commits
- If tests are missing for the refactoring target, suggest adding them first
- Keep each refactoring step small enough to be reviewable independently
