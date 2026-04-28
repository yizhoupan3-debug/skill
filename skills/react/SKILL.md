---
name: react
description: |
  Deliver React 19+ components with correct hook dependencies, optimal Server
  Component boundaries, and controlled re-render behavior. Produces composable
  component hierarchies, well-scoped custom hooks, and rendering-safe state
  architecture using Zustand/Jotai/TanStack Query. Use when the user asks for
  React development, component design, state management, hook patterns, or
  phrases like "React 组件", "状态管理", "自定义 hook", "React 性能优化".
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - react
    - hooks
    - server-components
    - state-management
    - testing
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - React 组件
  - 状态管理
  - 自定义 hook
  - React 性能优化
  - React development
  - component design
  - state management
  - hook patterns
  - react
  - hooks
---

# react

This skill owns React-first engineering work: component design, hooks, state management, and rendering optimization.

## When to use

- Building or reviewing React components and applications
- Designing state management and data flow architecture
- Optimizing React rendering performance
- Writing custom hooks or component libraries

## Do not use

- The project uses Next.js (use `$nextjs` for framework-specific patterns)
- The task is about Vue, Svelte, or another framework
- The task is about vanilla JavaScript without React

## Task ownership and boundaries

This skill owns:
- React component design and patterns
- hooks and custom hooks
- state management architecture
- rendering optimization and memoization
- component testing strategy

This skill does not own:
- Next.js framework-specific features
- non-React UI frameworks
- backend API design
- vanilla JS/DOM behavior

If the task shifts to adjacent skill territory, route to:
- `$nextjs` for Next.js App Router / Server Actions
- `$vue` / `$svelte` for other frameworks
- `$typescript-pro` for deep type system design
- `$nextjs` with the Vercel best-practices reference for rendering/data-flow audits
- **Dual-Dimension Audit (Pre: Spec/Logic, Post: Result Idealism)** → runtime verification gate

## Required workflow

1. Confirm the task shape:
   - object: component, hook, state architecture, rendering pipeline
   - action: build, refactor, debug, optimize, review
   - constraints: React version, rendering mode, state management, testing framework
   - deliverable: component code, hook, architecture design, or review guidance
2. Clarify React version and rendering mode (CSR, SSR, RSC).
3. Inspect existing component and state patterns before introducing new ones.
4. Validate with component tests and performance profiling.

## Core workflow

### 1. Intake
- Identify React version and rendering strategy.
- Check existing state management approach.
- Inspect component hierarchy and data flow.

### 2. Execution
- Design components with composition and single responsibility.
- Implement hooks with correct dependency arrays.
- Use memoization only when it solves a demonstrated problem.
- Keep Server Components as default; add `'use client'` only for interactivity.

### 3. Validation / recheck
- Run component tests with Testing Library.
- Check for unnecessary re-renders with React DevTools.
- Verify hook dependency arrays are complete and correct.
- If performance-sensitive, profile before and after changes.

## Capabilities

### Modern React Features (19+)
- `use()` hook for promises and context
- `useActionState` and `useFormStatus` for form handling
- `useOptimistic` for optimistic UI updates
- Server Components and `'use client'` boundaries
- `<Suspense>` for async rendering and streaming
- `useTransition` and `startTransition` for non-blocking updates
- React Compiler (automatic memoization — when enabled, manual `useMemo`/`useCallback` often unnecessary)
- `ref` as prop (no more `forwardRef`)
- `Activity` component for offscreen rendering

### Component Patterns
- Composition over inheritance
- Compound components with context
- Render props and headless components
- Controlled vs uncontrolled components
- Error boundaries and error recovery
- Portals for modal/tooltip rendering
- Higher-order components (HOC) when appropriate

### Hook Patterns
- Custom hooks for logic extraction and reuse
- `useReducer` for complex state logic
- `useCallback` / `useMemo` with correct dependencies
- `useRef` for mutable values and DOM access
- `useSyncExternalStore` for external store integration
- `useId` for accessible form labels
- `useImperativeHandle` for imperative APIs

### State Management
- Local state with `useState` / `useReducer`
- Context for cross-cutting concerns (theme, auth, locale)
- Zustand for lightweight global state
- Jotai for atomic state management
- TanStack Query for server state (caching, refetching, optimistic updates)
- URL state with search params

### Performance Optimization
- React DevTools Profiler and `React.memo`
- Virtualization for large lists (TanStack Virtual, react-window)
- Code splitting with `React.lazy` and dynamic imports
- Bundle analysis and tree-shaking
- Avoiding unnecessary re-renders

### Testing
- Component testing with Vitest + Testing Library
- User-event simulation and accessibility queries
- Integration testing with MSW for API mocking
- Snapshot testing (sparingly)
- Hook testing with `renderHook`

### Ecosystem Integration
- React Router v6/v7 routing patterns
- Form libraries: React Hook Form, Formik
- Animation: Framer Motion, React Spring
- UI libraries: Radix, shadcn/ui, Headless UI
- Styling: CSS Modules, styled-components, vanilla-extract

## Output defaults

Default output should contain:
- React context and rendering assumptions
- component / hook design approach
- validation notes and performance risks

Recommended structure:

````markdown
## React Summary
- React version: ...
- Rendering mode: ...
- State management: ...

## Changes / Guidance
- ...

## Validation / Risks
- Tested: ...
- Performance notes: ...
````

## Hard constraints

- Do not add `'use client'` to components that can remain Server Components.
- Do not apply `useMemo` / `useCallback` reflexively without a demonstrated problem; React Compiler auto-memoizes when enabled.
- Do not disable `react-hooks/exhaustive-deps` lint rule.
- Do not use inline component definitions inside render paths.
- Do not mix multiple state management libraries without explicit justification.
- Prefer composition over HOCs or render props when both work.
- All list items must have stable, meaningful `key` props.
- Use RORO pattern (Receive an Object, Return an Object) for component props and function parameters.
- Prioritize early returns and guard clauses to reduce nesting and improve readability.
- Keep component files ≤150 lines; extract subcomponents, hooks, or helpers when exceeded.
- Name event handler functions with `handle` prefix (e.g., `handleClick`, `handleSubmit`).
- Handle errors at the beginning of functions; place the happy path last.
- **Superior Quality Audit**: For high-stake UI, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples

- "Use $react to design a reusable component library."
- "帮我设计一个可复用的 React 组件库。"
- "这个 React 组件 re-render 太频繁了。"
- "用 Zustand 做全局状态管理。"
- "强制进行 React 深度审计 / 检查 Hook 闭包与渲染性能。"
- "Use runtime verification gate to review this React implementation for superior quality."
