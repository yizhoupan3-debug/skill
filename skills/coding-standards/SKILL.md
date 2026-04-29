---
name: coding-standards
description: |
  Enforce cross-stack coding standards: naming, readability, error handling,
  immutability, and type safety for backend and full-stack code (Python, Go,
  Node.js, Rust, SQL). Use when reviewing code quality drift, preventing
  recurring defects, and cutting needless abstraction or scope drift.
  Frontend component quality can be handled as a reference mode when no framework-specific owner is needed.
metadata:
  version: "4.2.0"
  platforms: [codex]
  tags:
    - coding-standards
    - readability
    - naming
    - error-handling
    - backend
    - full-stack
    - kaizen
    - continuous-improvement
    - poka-yoke
    - standardization
source: local
risk: low
routing_layer: L1
routing_owner: overlay
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - coding standards
  - 代码规范
  - readability
  - naming
  - error handling
  - backend
  - full stack
allowed_tools:
  - shell
  - python
approval_required_tools: []

---

# coding-standards

This skill owns **cross-stack and backend coding standards** as an overlay:
naming, readability, error handling, immutability, type safety, anti-pattern
detection, and practical simplification.

## When to use

- The user asks for coding standards, naming conventions, or style enforcement on backend or full-stack code
- The task involves code quality drift cleanup across Python, Go, Node.js backend, Rust, or SQL
- The user says "编码规范", "代码风格统一", "anti-pattern 检测", "后端代码规范"
- The user wants simplification, mistake-proofing, or standardization across workflows
- The user says "持续改进", "防错设计", "标准化", "流程优化", "质量改进"
- The user wants to reduce rework, prevent recurring defects, or unify scattered patterns
- Use as an overlay when another domain skill owns the task but coding standard compliance is also needed

## Do not use

- The task is framework-specific frontend semantics → use `$react`, `$nextjs`, `$vue`, or `$svelte`
- The user wants quantitative code scoring with a 0-100 score → use `$code-review`
- The user wants architecture-level review → use `$architect-review`
- The user wants security-focused audit → use `$security-audit`
- The user wants a full code review checklist or PR review → use `$code-review`
- The user wants hands-on code refactoring (extract, inline, move) → use `$refactoring`

## Task ownership and boundaries

This skill owns:
- Cross-language naming, readability, immutability, type safety, and error handling
- Async patterns and common code-smell detection
- Simplicity rules: no single-use abstraction, no speculative configurability, no framework for a one-off case
- Surgical-change discipline: keep diffs traceable to the request and avoid drive-by cleanup
- API and comment conventions
- Process-level quality patterns such as fail-fast config, guard clauses, and Rule of Three

This skill does not own:
- Framework-specific frontend semantics → `$react`, `$nextjs`, `$vue`, or `$svelte`
- Error handling architecture design → `$error-handling-patterns`
- Quantitative scoring → `$code-review`
- Security auditing → `$security-audit`

## Core workflow

### 1. Intake

- Identify the language/stack
- Identify the scope (file, module, project)
- Identify the primary concern (naming, readability, anti-patterns, continuous improvement, etc.)

### 2. Review in priority order

| Area | Core Rules |
|---|---|
| Naming | Descriptive names, verb-noun pattern, language-idiomatic casing |
| Immutability | Spread operators, no direct mutation |
| Error handling | Complete try/catch, specific exception types |
| Async | Use `Promise.all` for parallel work |
| Type safety | No `any`, define concrete interfaces |
| Simplicity | No speculative flags/options, no single-use abstraction, no future-proofing by default |
| Change scope | Every changed line should trace back to the request or required fallout |
| Comments | Explain "why" not "what", JSDoc for public APIs |
| Code smells | Functions < 50 lines, nesting < 5 levels, no magic numbers |

### 3. Process checks

When doing a process-level review, also check:

- **Invalid-state control**: Can simple guard rails or types prevent recurring bugs?
- **Guard clauses**: Are there opportunities to replace deep nesting with early returns?
- **Fail-fast**: Does configuration validate at startup, not at request time?
- **Standardized work**: Do similar code paths follow consistent patterns?
- **Just-in-time complexity**: Is abstraction added only when a pattern appears 3+ times?
- **Speculation control**: Did the change add options, layers, or extensibility that no current requirement needs?
- **Diff discipline**: Did the task widen into unrelated cleanup instead of staying surgical?

### 4. Validation

- Verify fixes do not break existing tests
- Run language-specific linter if available

## Hard constraints

- Do not enforce frontend-specific patterns on backend code
- Do not mix style preferences with real anti-patterns; distinguish clearly
- Do not ignore language idioms (e.g., don't enforce JS naming on Python)
- Prefer self-documenting code over excessive comments
- Keep findings actionable with specific file/line locations
- Only abstract after a pattern appears 3+ times (Rule of Three)
- Do not add configurability, indirection, or "future use" hooks without a present requirement
- Do not turn a local fix into adjacent refactoring unless the dependency is real and explained
- Prefer small, compounding improvements over big rewrites

## References

- [references/DETAIL.md](references/DETAIL.md)
- [references/frontend-quality.md](references/frontend-quality.md)
