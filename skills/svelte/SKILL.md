---
name: svelte
description: |
  Deliver Svelte 5 applications using runes-based reactivity ($state, $derived,
  $effect) with compile-time optimization and minimal client JS. Produces
  SvelteKit routes with server-first data loading, form actions, and progressive
  enhancement. Use when the user asks for Svelte development, SvelteKit SSR,
  Svelte 4→5 migration, or phrases like "Svelte 项目", "SvelteKit", "runes
  怎么用", "Svelte 5 迁移".
metadata:
  version: "2.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - svelte
    - sveltekit
    - runes
    - ssr
    - compile-time
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - Svelte 项目
  - SvelteKit
  - runes 怎么用
  - Svelte 5 迁移
  - Svelte development
  - SvelteKit SSR
  - Svelte 4→5 migration
  - svelte
  - runes
  - ssr
---

# svelte

This skill owns Svelte-first engineering work: runes-based reactivity, SvelteKit architecture, and compile-time optimization.

## When to use

- Building or reviewing Svelte/SvelteKit applications
- Designing reactive component architectures with runes
- Working with SvelteKit server features (load, actions, hooks)
- Migrating from Svelte 4 to Svelte 5 runes

## Do not use

- The task is about React, Vue, or another framework
- The task is purely about server-side API without a Svelte frontend
- The task is about Svelte 3 legacy without migration intent

## Task ownership and boundaries

This skill owns:
- Svelte 5 runes-based component design
- SvelteKit architecture and routing
- load functions, form actions, and server hooks
- compile-time optimization strategy
- Svelte 4 → 5 migration

This skill does not own:
- React, Vue, or other framework code
- pure CSS/HTML without Svelte context
- backend API design without SvelteKit context

If the task shifts to adjacent skill territory, route to:
- `$react` / `$vue` for other UI frameworks
- `$node-backend` for standalone backend services
- `$typescript-pro` for deep TS type design
- **Dual-Dimension Audit (Pre: Runes/Compile-time, Post: SSR/JS-size Results)** → `$execution-audit` [Overlay]

## Required workflow

1. Confirm the task shape:
   - object: component, route, load function, form action, store, migration
   - action: build, refactor, debug, optimize, migrate, review
   - constraints: Svelte 4 vs 5, SvelteKit, deployment adapter, TypeScript
   - deliverable: component code, route, migration plan, or architecture guidance
2. Clarify Svelte version (4 vs 5 runes).
3. Design with runes (`$state`, `$derived`, `$effect`) for Svelte 5.
4. Implement proper server/client separation in SvelteKit.
5. Verify with build checks and component tests.

## Core workflow

### 1. Intake
- Identify Svelte version and SvelteKit usage.
- Check deployment adapter (Node, Vercel, Cloudflare, static).
- Inspect existing component and store patterns.

### 2. Execution
- Use Svelte 5 runes syntax for all new code.
- Follow server-first data loading in SvelteKit.
- Apply progressive enhancement with form actions.
- Leverage compile-time optimizations and minimal client JS.

### 3. Validation / recheck
- Run `svelte-check` for type errors.
- Verify SSR hydration and adapter compatibility.
- Run component tests with Vitest + Testing Library.
- Check build output size for unexpected growth.

## Capabilities

### Svelte 5 Runes
- `$state` for reactive state declarations
- `$derived` for computed values (replaces `$:` reactive statements)
- `$effect` for side effects (replaces `$:` side-effect blocks)
- `$props` for component props declaration
- `$bindable` for two-way binding props
- `$inspect` for debugging reactivity
- Snippets for reusable template fragments

### SvelteKit
- File-based routing with `+page.svelte`, `+layout.svelte`
- `+page.server.ts` for server-side load functions
- `+page.ts` for universal load functions
- Form actions with `+page.server.ts` `actions`
- `+server.ts` for API routes
- `hooks.server.ts` for request interception
- Error handling with `+error.svelte`
- Streaming with promises in load functions

### Reactivity & State
- Fine-grained reactivity without Virtual DOM
- Stores for cross-component state (`writable`, `readable`, `derived`)
- Context API for dependency injection
- Deep reactivity with `$state` proxies

### Rendering Strategies
- SSR by default with hydration
- `export const ssr = false` for client-only pages
- `export const prerender = true` for static generation
- Adapter-based deployment (Node, Vercel, Cloudflare, static)

### Component Patterns
- Slot-based composition with snippets (Svelte 5)
- Component events and event forwarding
- Actions for reusable DOM behavior
- Transitions and animations (`transition:`, `animate:`)
- Dynamic components with `<svelte:component>`

### Performance
- Compile-time optimizations (no runtime overhead)
- Minimal JavaScript output
- Built-in transitions and animations
- Lazy loading with dynamic imports
- CSS scoping by default

### Tooling
- Vite-based development with `@sveltejs/vite-plugin-svelte`
- Testing: Vitest + @testing-library/svelte
- Type checking: svelte-check
- Linting: eslint-plugin-svelte
- Svelte Inspector for debugging

## Output defaults

Default output should contain:
- Svelte context and version assumptions
- component / route design approach
- validation notes

Recommended structure:

````markdown
## Svelte Summary
- Svelte version: ...
- SvelteKit: yes / no
- Adapter: ...

## Changes / Guidance
- ...

## Validation / Risks
- Checked: ...
- Build output: ...
````

## Hard constraints

- Do not use Svelte 4 reactive statements (`$:`) in Svelte 5 projects.
- Do not bypass SvelteKit's load/action patterns for ad-hoc client fetching.
- Do not add unnecessary runtime overhead; leverage compile-time optimizations.
- Do not skip `svelte-check` before concluding changes.
- Preserve progressive enhancement in form actions.
- Keep client-side JavaScript minimal by default.
- **Superior Quality Audit**: For high-performance Svelte apps, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## Trigger examples

- "Use $svelte to build a SvelteKit SSR project."
- "用 SvelteKit 搭建一个 SSR 项目。"
- "帮我把 Svelte 4 迁移到 Svelte 5 runes。"
- "设计一个 Svelte 组件库。"
- "SvelteKit 的 form actions 怎么用？"
- "强制进行 Svelte 深度审计 / 检查 Runes 响应式与 SSR 结果。"
- "Use $execution-audit to audit this Svelte 5 implementation for perfect execution."
