---
name: vercel-react-best-practices
description: |
  Apply Vercel-style React/Next.js best practices for App Router, Server Components, streaming, caching, and rendering.
  Use when writing, reviewing, or refactoring React/Next.js code and the concern is rendering strategy, data flow, or frontend performance rather than generic visual styling. 适用于“React 最佳实践”“Next.js 最佳实践”“bundle 优化”“hydration”“re-render 优化”这类请求.
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - react
    - nextjs
    - app-router
    - performance
    - rendering-strategy
risk: low
source: local
routing_layer: L4
routing_owner: overlay
routing_gate: none
session_start: n/a
trigger_hints:
  - React 最佳实践
  - Next.js 最佳实践
  - bundle 优化
  - hydration
  - re-render 优化
  - react
  - nextjs
  - app router
  - performance
  - rendering strategy
---

# vercel-react-best-practices

This skill owns React/Next.js code-level rendering and data-flow best practices, especially where Vercel-style architecture guidance matters.

## When to use

- The user wants a React or Next.js code review focused on rendering/data flow quality
- The user wants to reduce waterfalls, hydration issues, bundle size, or re-renders
- The user wants better Server Component / Client Component boundaries
- The user is refactoring App Router code for performance or correctness
- Best for requests like:
  - "看下这个 Next.js 数据获取有没有 waterfall"
  - "优化 React re-render"
  - "这个页面该放 server 还是 client"

## Do not use

- The task is general web performance outside React/Next.js → use `$performance-expert`
- The task is general React performance optimization without App Router / Server Components context → use `$react` (this skill is narrowly scoped to Vercel-style App Router patterns)
- The task is primarily CSS, design, or visual polish
- The stack is not React or Next.js
- The issue is mainly backend/database performance

## Task ownership and boundaries

This skill owns:
- server/client rendering boundaries
- App Router data-fetching patterns
- waterfall reduction
- hydration discipline
- bundle-shape and component-surface review
- unnecessary re-render reduction

This skill does not own:
- generic frontend aesthetics
- backend query optimization
- non-React/non-Next.js codebases

If the task shifts to adjacent skill territory, route to:
- `$performance-expert` for broader web perf and Core Web Vitals strategy
- `$nextjs` when the task is broad Next.js feature implementation beyond best-practice review
- `$react` when the task is modern React implementation not primarily about Vercel-style rendering guidance

## Required workflow

1. Identify whether the task is review mode or implementation/refactor mode.
2. Inspect data flow before micro-optimizations.
3. Eliminate structural problems first.
4. Only then suggest memoization or smaller tactical changes.
5. Deliver concrete fixes tied to code patterns.

## Core workflow

### 1. Intake

- Determine:
  - React vs Next.js
  - App Router vs other routing model
  - review vs implementation task
  - primary pain point:
    - waterfall
    - hydration
    - bundle size
    - re-rendering
    - server/client boundary

### 2. Review structural issues first

Check in this order:

#### Waterfalls
- independent fetches done serially
- requests started too late
- missing `Promise.all`
- poor Suspense/streaming boundaries

#### Server/client boundary
- data fetched in client when it can live on server
- over-large client component surface
- unnecessary serialization
- mixing Client handlers and Server Actions incorrectly, causing closure/serialization bugs

#### Bundle shape & Component Surface
- barrel imports
- eager loading of heavy components/libs
- missing dynamic import opportunities
- monolithic components with 10+ boolean props (Remedy: Variant Pattern, Config Objects, Compound Components `Parent.Child`, or Slots Pattern)
- conflicting props allowing invalid states (Remedy: TypeScript Discriminated Unions)

#### Re-render patterns & State
- excessive hook prop drilling across component layers
- passing complex objects as props triggering re-renders
- missing Context or component composition (`children` prop) to avoid drilling
- derived state in effects
- unstable callbacks without need
- giant subscriptions for tiny reads
- component definitions inside render paths

#### Hydration discipline
- mismatch-prone client-only values
- unstable config objects causing hydration mismatches
- unclear conditional rendering
- wrong assumptions about first render state

### 3. Implementation/refactor mode

When changing code, prioritize:
1. fix fetch sequencing
2. move data server-side where possible
3. reduce client surface
4. split heavy imports/components
5. stabilize render logic

Avoid reflexive `useMemo` / `useCallback` unless they solve a demonstrated issue.

## Output defaults

Default output should contain:
- biggest structural issues
- why they hurt
- concrete fixes

Recommended structure:

````markdown
## React/Next Review Summary
- Primary issue: ...

## Findings
1. Issue: ...
   - Why it hurts: ...
   - Fix: ...

## Recommended Changes
- ...

## Expected Gains / Risks
- ...

*(Note: Always link directly to the affected component files or line numbers when suggesting structural refactors.)*
````

## Hard constraints

- Do not lead with micro-optimizations before checking architecture/data flow.
- Do not recommend memoization mechanically.
- Do not move logic client-side when server-side placement is viable.
- Clearly separate confirmed issues from likely-but-unverified concerns.
- Keep fixes grounded in the actual code, not generic advice.

## Trigger examples

- "Use $vercel-react-best-practices to review this Next.js page for waterfalls and hydration issues."
- "Refactor this React code to reduce unnecessary re-renders and client bundle size."
- "这个 App Router 页面应该怎么调整 server/client 边界？"
- "帮我过一遍这段 React 代码的渲染性能。有过度渲染吗？"
- "怎么解决 Next.js 里的 hydration mismatch 报错？"
