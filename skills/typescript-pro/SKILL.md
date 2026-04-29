---
name: typescript-pro
description: |
  Deliver type-safe TypeScript 5.x+ code. Enforces strict mode, encodes domain constraints, and handles complex generics.
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - typescript
    - type-system
    - generics
    - tsconfig
    - migration
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - typescript
  - type system
  - generics
  - tsconfig
  - migration

---

# typescript-pro

This skill owns TypeScript-first engineering work: type system design, strict-mode enforcement, tsconfig strategy, and TS-specific refactoring.

## When to use

- The user wants to build, refactor, or debug a TypeScript codebase
- The task involves type system design, generics, utility types, or type errors
- The task involves tsconfig optimization, module resolution, or TS migration
- The user wants strict mode enforcement or `any` elimination
- Best for requests like:
  - "帮我设计一个类型安全的 API client"
  - "这个 TypeScript 类型错误怎么解"
  - "把这个 JS 项目迁移到 TypeScript"
  - "写一个泛型工具类型"

## Do not use

- The project is pure JavaScript without TypeScript intent → use `$javascript-pro`
- The main task is a framework's rendering/data-flow behavior → use the relevant framework skill
- The task is about runtime behavior without type concerns → use `$javascript-pro` or `$node-backend`
- The task is npm packaging/publishing → use `$npm-package-authoring`

## Task ownership and boundaries

This skill owns:
- TypeScript type system design and patterns
- tsconfig strategy and module resolution
- strict mode enforcement and `any` elimination
- JS→TS migration planning and execution
- type-level testing and validation

This skill does not own:
- pure JavaScript runtime behavior
- framework-specific rendering strategy
- backend service architecture by default
- package publishing workflows

### Overlay interaction rules

- Language-idiomatic TS error handling (union types, discriminated errors, `never` exhaustiveness) is owned by this skill
- Cross-language error **architecture** (error taxonomies, retry/circuit-breaker) → `$error-handling-patterns`
- Web frontend performance audits → `$performance-expert`
- **Critical implementation auditing (Memory, Speed, Platform-native)** → runtime verification gate

If the task shifts to adjacent skill territory, route to:
- `$javascript-pro`
- `$react`
- `$nextjs`
- `$vue`
- `$node-backend`
- `$npm-package-authoring`

## Required workflow

1. Confirm the task shape:
   - object: TS file, module, type system, tsconfig, migration scope
   - action: build, refactor, debug type errors, migrate, review, optimize types
   - constraints: TS version, strict mode, module resolution, framework context
   - deliverable: code change, type design, migration plan, or review guidance
2. Verify TypeScript version and tsconfig settings before changing type patterns.
3. Check existing type conventions before introducing new patterns.
4. Preserve type safety invariants throughout changes.
5. Validate with `tsc --noEmit` after modifications.

## Core workflow

### 1. Intake
- Identify TypeScript version, tsconfig strict settings, and module resolution mode.
- Check existing type patterns and conventions in the project.
- Inspect tsconfig hierarchy (extends, references, composite) before making changes.

### 2. Execution
- Design types that encode domain constraints precisely.
- Prefer `unknown` over `any` and `satisfies` over type assertions.
- Keep types close to their usage; avoid god-type files.
- Use `const` assertions and literal types for compile-time safety.
- Leverage inference over explicit annotations where the intent is clear.

### 3. Validation / recheck
- Run `tsc --noEmit` on affected files.
- Verify no `any` leaks were introduced.
- Check that type narrowing works correctly in all branches.
- If complex utility types were added, suggest type-level tests.

## Capabilities

### Advanced Type System
- Generics with constraints, defaults, and variance annotations
- Conditional types, infer keyword, distributive conditionals
- Mapped types, template literal types, recursive types
- Type narrowing: type guards, discriminated unions, `satisfies`
- `const` assertions and literal types
- Declaration merging and module augmentation
- Branded/opaque types for domain safety

### Modern TypeScript Features (5.x+)
- `using` keyword and explicit resource management
- Decorator metadata and stage 3 decorators
- `const` type parameters
- `NoInfer<T>` utility type
- Module resolution: `bundler`, `node16`, `nodenext`
- `verbatimModuleSyntax` and ESM best practices
- Isolated declarations and project references

### Configuration & Tooling
- tsconfig.json optimization for different targets
- Strict mode settings and their implications
- Project references and composite builds
- Path aliases and module resolution strategies
- Build tools: tsc, esbuild, swc, tsup, unbuild
- Type checking: tsc, @typescript-eslint, oxlint

### Type Design Patterns
- Builder pattern with fluent generic chains
- Type-safe event emitters and pub/sub
- Exhaustive switch with `never` checks
- Type-safe API client generation (OpenAPI, tRPC)
- Zod/Valibot schema inference patterns
- Type-safe state machines
- Phantom types for compile-time validation

### Migration & Integration
- Gradual migration from JavaScript (`allowJs`, `checkJs`)
- Writing and publishing `.d.ts` declaration files
- DefinitelyTyped contribution patterns
- Monorepo TypeScript configuration

## Output defaults

Default output should contain:
- TypeScript context and configuration assumptions
- type design / code approach
- validation notes and compatibility risks

Recommended structure:

````markdown
## TypeScript Summary
- TS version: ...
- Strict mode: ...
- Module resolution: ...

## Changes / Guidance
- ...

## Validation / Risks
- Checked: ...
- Compatibility notes: ...
````

## Hard constraints

- Never use `any` unless explicitly justified with a comment.
- Do not suppress type errors with `@ts-ignore` or `@ts-expect-error` without a justifying reason.
- Do not introduce TS 5.x+ features when the project targets earlier versions without flagging.
- Do not change module resolution mode without understanding downstream impact.
- Prefer `satisfies` over type assertions for validated data.
- Do not create god-type files that become a dependency bottleneck.
- If type narrowing fails at a boundary, fix the narrowing rather than casting.

## Trigger examples

- "Use $typescript-pro to design a type-safe API client with generics."
- "帮我设计一个类型安全的 API client。"
- "这个 TypeScript 类型错误怎么解？"
- "把这个 JS 项目迁移到 TypeScript。"
