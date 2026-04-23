---
name: javascript-pro
description: |
  Deliver correct JavaScript code for ESM/CJS boundaries, browser vs Node runtime
  differences, and explicit non-TypeScript constraints. Covers `.js`/`.mjs`/`.cjs`,
  `package.json` module settings, async patterns, and Node scripts/CLIs. Use when the user
  asks for pure JS, vanilla JS, ESM/CJS migration, or phrases like '这是 JS 项目', '不要上 TS',
  'require/import 怎么处理'.
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - javascript
    - nodejs
    - browser
    - esm
    - commonjs
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 这是 JS 项目
  - 不要上 TS
  - require
  - import 怎么处理
  - pure JS
  - vanilla JS
  - ESM
  - CJS migration
  - javascript
  - nodejs
---

# javascript-pro

This skill owns JavaScript-first engineering work that should not be forced into TypeScript-specific guidance or a framework-specific skill.

## When to use

- The user wants to build, refactor, or debug a JavaScript codebase without introducing TypeScript
- The task involves `.js`, `.mjs`, `.cjs`, `package.json`, ESM/CJS interop, or JavaScript runtime behavior
- The task involves browser JavaScript, Node.js **scripts/tools/CLIs** (non-HTTP), module boundaries, async flows, or JS project cleanup
- The user explicitly says the project is JavaScript, not TypeScript
- Node.js scope: **scripts, CLI tools, file processing, module utilities** — NOT HTTP servers/APIs (those go to `$node-backend`)
- Best for requests like:
  - "这是 JS 项目，不要上 TS"
  - "帮我改这个 .js 文件"
  - "CommonJS 怎么迁到 ESM"
  - "require/import 怎么处理"

## Do not use

- The main task is TypeScript type-system design, strict typing, or tsconfig strategy → use `$typescript-pro`
- The main task is React, Next.js, Vue, or Svelte framework behavior rather than JavaScript itself → use the relevant framework skill
- The task is **Node.js HTTP API / service / middleware** design (Express, Fastify, Koa, Hono, NestJS) → use `$node-backend`
- The task is npm packaging/publishing workflow rather than application code → use `$npm-package-authoring`

> **Decision guide: `javascript-pro` vs `node-backend`**
> - "Help me write a Node script to process CSV files" → `javascript-pro` (Node script/tool)
> - "Build an Express REST API" → `node-backend` (HTTP service)
> - "Fix this ESM/CJS import issue in a CLI tool" → `javascript-pro`
> - "Add middleware and auth to this Fastify server" → `node-backend`

## Task ownership and boundaries

This skill owns:
- modern JavaScript code structure and patterns
- ESM/CJS module strategy and migration
- async control flow and runtime-safe JS refactors
- JS-specific package/runtime configuration
- JSDoc-based type hints and gradual hardening when TypeScript is out of scope

This skill does not own:
- deep TypeScript type design
- framework-specific rendering/data-flow strategy
- Node.js HTTP service architecture (Express, Fastify, etc.) → `$node-backend`
- package publishing workflows

### Overlay interaction rules

- Language-idiomatic error handling (try/catch patterns, Promise rejection) is owned by this skill
- Cross-language error **architecture** (error taxonomies, retry/circuit-breaker, error code systems) → `$error-handling-patterns`
- Web frontend performance audits → `$performance-expert`
- **Critical implementation auditing (Memory, Speed, Platform-native)** → `$execution-audit` [Overlay]

If the task shifts to adjacent skill territory, route to:
- `$typescript-pro`
- `$react`
- `$nextjs`
- `$vue`
- `$svelte`
- `$node-backend`
- `$npm-package-authoring`

## Required workflow

1. Confirm the task shape:
   - object: JS file, script, module graph, package config, browser or Node runtime
   - action: build, refactor, debug, migrate, review, optimize
   - constraints: runtime, module system, framework context, dependency policy
   - deliverable: code change, migration plan, fix, or review guidance
2. Check runtime and module-system assumptions before changing imports or exports.
3. Preserve JavaScript runtime behavior while improving clarity, boundaries, and maintainability.
4. Call out ESM/CJS and browser/Node differences explicitly when relevant.
5. Validate the resulting code path in the intended runtime.

## Core workflow

### 1. Intake
- Identify runtime context: browser, Node.js, bundler, test runner, or mixed environment.
- Check module system assumptions: CommonJS, ESM, dual-package, or transpiled output.
- Inspect current project conventions before imposing a new pattern.

### 2. Execution
- Prefer clear JavaScript patterns over pseudo-TypeScript habits in `.js` files.
- Untangle module boundaries, side effects, and import/export shape first.
- Use JSDoc only when it materially improves maintainability without violating the JS-only constraint.
- Keep async/error behavior explicit and avoid silent promise handling.
- Apply Chain of Thought: outline a pseudocode plan step-by-step, confirm, then write code.
- Follow the 6 quality mindsets: Simplicity, Readability, Performance, Maintainability, Testability, Reusability.

### 3. Validation / recheck
- Re-check runtime compatibility, module resolution, and import paths.
- Verify that refactors preserve external API and side-effect timing when relevant.
- If migration tradeoffs remain, state them explicitly.

## Output defaults

Default output should contain:
- JavaScript context and runtime assumptions
- code/refactor approach
- validation notes and compatibility risks

Recommended structure:

````markdown
## JavaScript Summary
- Runtime: ...
- Module system: ...

## Changes / Guidance
- ...

## Validation / Risks
- Checked: ...
- Compatibility notes: ...
````

## Hard constraints

- Do not force TypeScript migration when the request is clearly JS-only.
- Do not assume ESM and CommonJS are interchangeable without checking runtime/package settings.
- Do not add build-tool assumptions unless the repo already uses them or the user asks for them.
- If browser and Node semantics differ for the solution, say so explicitly.
- Preserve public-facing module contracts unless the user asks to break them.
- Use early returns and guard clauses to avoid deep nesting.
- Prefer functional, immutable style unless it makes code significantly more verbose.
- Name event handlers with `handle` prefix (e.g., `handleClick`, `handleKeyDown`).
- Focus on correct, DRY, minimal code changes — lines of code = debt.

## Trigger examples

- "Use $javascript-pro to refactor this JS module without converting to TypeScript."
- "这是纯 JavaScript 项目，别上 TS，帮我改。"
- "帮我处理 CommonJS 和 ESM 的兼容问题。"
