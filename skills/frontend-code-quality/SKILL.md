---
name: frontend-code-quality
description: |
  Enforce frontend code-quality rules such as ≤150-line files, early returns, and RORO parameters.
  Use as a cross-cutting overlay for React/Vue/Svelte when code quality, consistency, readability, or maintainability is the main concern, or when the user asks 代码质量、前端规范、编码风格、可读性、DRY, or early return.
metadata:
  version: "1.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - code-quality
    - frontend
    - patterns
    - readability
    - best-practices

routing_layer: L3
routing_owner: overlay
routing_gate: none
session_start: n/a
trigger_hints:
  - 代码质量
  - 前端规范
  - 编码风格
  - 可读性
  - DRY
  - early return
  - code quality
  - frontend
  - patterns
  - readability
---

# frontend-code-quality

Cross-cutting quality overlay for frontend code. Layer this on top of any framework owner skill (`$react`, `$vue`, `$svelte`, `$nextjs`) when code quality, consistency, or readability is the primary concern.

## When to use

- **As an overlay on a framework owner skill** when code quality, consistency, or readability is the primary concern alongside framework implementation
- Reviewing or refactoring frontend code for patterns and style when a framework skill (`$react`, `$vue`, `$svelte`, `$nextjs`) is the main owner
- The user asks about coding conventions, naming, file structure, or code organization in a frontend context
- Standalone only when the review scope is purely about code quality without framework-specific decisions
- Best for requests like:
  - "帮我审查这个前端代码质量" (overlay on framework owner)
  - "这个代码可读性太差了" (overlay on framework owner)
  - "统一一下前端编码风格" (standalone for cross-project standards)
  - "用 early return 重构这个函数" (standalone for single function)

## Do not use

- The task is framework-specific implementation without quality concern → use the framework skill directly
- The task is **backend or full-stack** code quality (Python, Node, Go, Rust, SQL, API conventions) → use `$coding-standards`
- The task is CSS-only patterns → use `$css-pro`
- The task is architecture review → use `$architect-review`
- The task is quantitative code scoring → use `$code-review`

## Task ownership and boundaries

This skill owns:
- Frontend-specific code patterns (RORO, early returns, handle-prefix naming, 150-line file limit)
- Frontend code readability, consistency, and structural quality
- Chain of Thought pseudocode-first workflow for frontend code
- The 6 quality mindsets applied to frontend contexts

This skill does not own:
- Backend/full-stack coding standards → `$coding-standards`
- Framework-specific rendering/data-flow behavior → framework skills
- CSS architecture and layout engineering → `$css-pro`
- Quantitative scoring → `$code-review`

## The 6 Quality Mindsets

Every frontend code change should be evaluated through these lenses:

1. **Simplicity** — Write simple, straightforward code. Less code = less debt.
2. **Readability** — Code is read far more often than written. Optimize for the reader.
3. **Performance** — Keep performance in mind but never at the cost of readability.
4. **Maintainability** — Easy to change, easy to delete, easy to understand 6 months later.
5. **Testability** — If it's hard to test, it's probably too coupled. Refactor.
6. **Reusability** — Extract, don't copy. But don't over-abstract prematurely.

## Core Patterns

### RORO (Receive an Object, Return an Object)
- Function parameters: use a single options object instead of positional args
- Return values: return a result object for multi-value returns
- Benefits: self-documenting, order-independent, easy to extend

### Error-First & Early Return
- Handle errors and edge cases at the beginning of functions
- Use guard clauses to handle preconditions early
- Place the happy path last for    improved readability
- Avoid unnecessary `else`; use if-return pattern

### Naming Conventions
- Event handlers: `handle` prefix (e.g., `handleClick`, `handleSubmit`, `handleKeyDown`)
- Booleans: auxiliary verbs (e.g., `isLoading`, `hasError`, `canSubmit`, `shouldRetry`)
- Constants: UPPER_SNAKE_CASE for true constants, camelCase for derived values
- Directories/files: lowercase with dashes (e.g., `components/auth-wizard`)
- Favor named exports over default exports

### File Size & Structure
- **≤150 lines per file** — refactor into smaller modules when exceeded
- File structure order: exports → subcomponents → helpers → static content → types
- One component per file; colocate tests, styles, and stories

### Chain of Thought Workflow
1. Outline a pseudocode plan step-by-step
2. Confirm the approach
3. Write the code
4. Validate against the 6 mindsets

## Hard constraints

- Do not leave TODO, placeholder, or incomplete implementations.
- Do not repeat code; extract shared logic into functions, hooks, or utilities.
- Do not use deeply nested conditionals; flatten with guard clauses.
- Do not use magic numbers or strings; define as named constants.
- Do not modify unrelated code sections; minimize change surface.
- Add a brief comment at the start of each function describing what it does.
- Prefer functional, immutable patterns unless they make code significantly more verbose.

## Trigger examples

- "Use $frontend-code-quality to review this component for readability."
- "帮我按 DRY 原则重构这些前端代码。"
- "统一这个项目的前端编码规范。"
- "这些组件太大了，帮我拆分。"
