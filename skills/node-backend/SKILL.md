---
name: node-backend
description: |
  Produce well-layered Node.js backend services with thin handlers, boundary validation, and testable service modules.
  Use when the user asks for Node APIs, backend route design, middleware chains, JWT/session auth, controller/service separation, or phrases like “写后端接口”, “Node 服务”, “Express/Fastify API”, “中间件怎么写”, “后端目录结构”.
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - nodejs
    - backend
    - api
    - express
    - fastify
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 写后端接口
  - Node 服务
  - Express
  - Fastify API
  - 中间件怎么写
  - 后端目录结构
  - Node APIs
  - backend route design
  - middleware chains
  - JWT
---

# node-backend

This skill owns practical Node.js backend engineering: API design, request handling, middleware, validation, auth integration, and maintainable server-side structure.

## When to use

- The user wants to build or refactor a Node.js backend service or API
- The task involves Express, Fastify, Koa, Hono, NestJS, or a custom Node HTTP stack
- The task involves route handlers, middleware, controllers, services, request validation, or backend error handling
- The user wants login/session/JWT wiring, API structure, background tasks, or server-side refactors
- Best for requests like:
  - "写一个 Express/Fastify API"
  - "帮我设计后端接口和中间件"
  - "给这个 Node 服务加鉴权和参数校验"
  - "重构一下后端目录结构和错误处理"

## Do not use

- The main task is Next.js App Router or Server Actions tightly coupled to a Next app → use `$nextjs`
- The main task is TypeScript type-system design rather than backend engineering → use `$typescript-pro`
- The task is mainly architecture review without implementation ownership → use `$architect-review`
- The task is security review of backend code rather than backend implementation → use `$security-audit`
- The task is a **non-HTTP Node.js script, CLI tool, or file-processing utility** without server concerns → use `$javascript-pro`

> **Decision guide: `node-backend` vs `javascript-pro`**
> - Express/Fastify/Koa/Hono/NestJS API routes, middleware, auth → `node-backend`
> - Node scripts using `fs`, `child_process`, `stream`, CLI tools → `javascript-pro`

## Task ownership and boundaries

This skill owns:
- Node.js API/server implementation
- middleware and request lifecycle design
- validation, parsing, error handling, and response shaping
- auth/session/JWT integration at the application layer
- backend module boundaries and service structure

This skill does not own:
- framework-specific frontend routing concerns
- pure type-level design detached from backend behavior
- **Dual-Dimension Audit (Pre: Handler/Logic, Post: Response/Error Results)** → `$execution-audit-codex` [Overlay]
- high-level architecture review without coding ownership
- standalone security auditing

If the task shifts to adjacent skill territory, route to:
- `$nextjs`
- `$typescript-pro`
- `$architect-review`
- `$security-audit`

## Required workflow

1. Confirm the task shape:
   - object: service, API routes, middleware, auth flow, job worker, module layer
   - action: build, refactor, debug, review, harden, document
   - constraints: framework, runtime, database, auth model, deployment target
   - deliverable: code, endpoint design, refactor, testable module, or plan
2. Identify API contracts and failure modes before implementation.
3. Implement clear boundaries between transport, validation, business logic, and persistence.
4. Make errors explicit and observable.
5. Validate behavior with focused tests or runnable examples.

## Core workflow

### 1. Intake
- Determine framework/runtime, transport style, auth model, and validation needs.
- Inspect existing server structure before proposing a reorg.

### 2. Execution
- Define route contracts, inputs, outputs, and expected status codes.
- Add request validation close to the boundary.
- Keep handlers thin; move business logic into services/use-cases.
- Normalize error handling and logging rather than scattering `try/catch` blocks.

### 3. Validation / recheck
- Verify request/response shapes, error paths, and auth behavior.
- Check that route code is not directly coupled to persistence or transport details without reason.
- Run or outline focused tests for critical endpoints and middleware.

## Output defaults

Default output should contain:
- backend approach and boundaries
- implementation or refactor details
- validation and remaining risks

Recommended structure:

````markdown
## Backend Summary
- Stack: ...
- Scope: ...

## Design / Changes
- Routes: ...
- Middleware / Validation: ...
- Service structure: ...

## Validation / Risks
- Tested: ...
- Assumptions: ...
- Follow-up: ...
````

## Hard constraints

- Do not mix validation, transport, and business logic into one large handler without a good reason.
- Do not leave error handling implicit for expected failure cases.
- Do not add auth-sensitive behavior without specifying how identity and authorization are derived.
- Prefer narrow, testable modules over framework-wide magic.
- If the request affects public API behavior, call out the contract change clearly.
- **Superior Quality Audit**: For production APIs, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples

- "Use $node-backend to build a Fastify API with validation and auth."
- "帮我写一个 Node 后端接口，带中间件和错误处理。"
- "重构这个 Express 服务，把 controller/service 分开。"
- "强制进行 Node 后端深度审计 / 检查 Handler 逻辑与 API 返回结果数据一致性。"
- "Use $execution-audit-codex to audit this Node.js backend for result idealism."
