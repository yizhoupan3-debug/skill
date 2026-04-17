---
name: vue
description: |
  Deliver Vue 3 applications using Composition API with correct reactivity
  chains, composable extraction, and Pinia state architecture. Produces
  `<script setup>` components with typed props/emits, and Nuxt 3 SSR routes
  with server-first data loading. Use when the user asks for Vue development,
  component design, state management, or phrases like "Vue 项目", "组合式 API",
  "Pinia", "Nuxt", "composable 怎么写".
metadata:
  version: "2.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - vue
    - composition-api
    - pinia
    - nuxt
    - composables
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
---
# vue

This skill owns Vue-first engineering work: Composition API, reactivity system, Pinia state management, and Nuxt 3 SSR.

## When to use

- Building or reviewing Vue 3 applications
- Designing composables and component libraries
- Working with Pinia for state management
- Building SSR applications with Nuxt 3

## Do not use

- The project uses Vue 2 Options API exclusively without migration intent
- The task is about React, Svelte, or another framework
- The task is about Nuxt-specific server features (consider combining with other skills)

## Task ownership and boundaries

This skill owns:
- Vue 3 component design and Composition API
- reactivity system and composables
- Pinia state management
- Vue Router and navigation
- Nuxt 3 SSR integration

This skill does not own:
- React, Svelte, or other framework code
- pure TypeScript type design → `$typescript-pro`
- general backend API design → `$node-backend`
- **Dual-Dimension Audit (Pre: Reactivity/Spec, Post: Hydration/Component Results)** → `$execution-audit-codex` [Overlay]

If the task shifts to adjacent skill territory, route to:
- `$react` / `$svelte` for other UI frameworks
- `$typescript-pro` for deep TS type design
- `$node-backend` for backend services

## Required workflow

1. Confirm the task shape:
   - object: component, composable, store, route, SSR page
   - action: build, refactor, debug, optimize, review
   - constraints: Vue version, Composition vs Options API, Nuxt 3, TypeScript
   - deliverable: component code, composable, store, or architecture guidance
2. Clarify Vue version and API style (Composition vs Options).
3. Inspect existing patterns before introducing new ones.
4. Validate with build checks and component tests.

## Core workflow

### 1. Intake
- Identify Vue version and whether using Nuxt 3.
- Check existing state management approach (Pinia vs Vuex vs local).
- Inspect component structure and script setup conventions.

### 2. Execution
- Use `<script setup>` for all new components.
- Prefer `ref` over `reactive` for primitive values.
- Extract reusable logic into composable functions.
- Use Pinia for cross-component state; keep local state local.

### 3. Validation / recheck
- Run component tests with Vitest + Vue Test Utils.
- Verify reactivity chains are not broken.
- Check TypeScript types in templates.
- Validate SSR hydration if using Nuxt.

## Capabilities

### Composition API & Script Setup
- `<script setup>` as default component format
- `ref`, `reactive`, `computed`, `watch`, `watchEffect`
- `defineProps`, `defineEmits`, `defineExpose`, `defineModel`
- `provide` / `inject` for dependency injection
- `toRef`, `toRefs`, `unref` for reactivity utilities
- Lifecycle hooks: `onMounted`, `onUnmounted`, `onUpdated`

### Composables
- Extracting reusable logic into composable functions
- `VueUse` integration (200+ composables)
  - High-frequency: `useStorage`, `useFetch`, `useMediaQuery`, `useIntersectionObserver`, `useDark`, `useClipboard`, `useEventListener`, `useDebounce`, `useVModel`
- Async composables with `Suspense` support
- Composable naming conventions (`use*`)
- Testing composables in isolation

### State Management (Pinia)
- Store design with `defineStore`
- Setup stores vs option stores
- Getters, actions, and plugins
- Store composition and cross-store references
- Persistence with `pinia-plugin-persistedstate`
- DevTools integration

### Vue Router
- File-based routing with unplugin-vue-router
- Navigation guards and middleware
- Route transitions and keep-alive
- Dynamic routes and lazy loading
- Typed routes with TypeScript

### TypeScript Integration
- Type-safe props and emits with generics
- Component type inference with `<script setup lang="ts">`
- Generic components
- Typed `provide` / `inject`
- Global component type augmentation

### SSR & Nuxt 3
- Server components and hybrid rendering
- `useFetch`, `useAsyncData` for SSR data fetching
- Auto-imports and module system
- SEO with `useHead` and `useSeoMeta`
- Nitro server engine

### Performance
- `v-once`, `v-memo` for render optimization
- Component lazy loading with `defineAsyncComponent`
- Virtual scrolling for large lists
- Template ref for direct DOM access
- Keep-alive for component caching

### Tooling
- Build with Vite (default)
- Testing: Vitest + Vue Test Utils
- Linting: eslint-plugin-vue
- DevTools browser extension
- Component documentation with Histoire/Storybook

## Output defaults

Default output should contain:
- Vue context and API style assumptions
- component / composable design approach
- validation notes

Recommended structure:

````markdown
## Vue Summary
- Vue version: ...
- API style: Composition / Options
- State management: ...

## Changes / Guidance
- ...

## Validation / Risks
- Tested: ...
- SSR notes: ...
````

## Hard constraints

- Do not use Options API for new components unless migrating legacy code.
- Do not use Vuex in new code; prefer Pinia.
- Do not use mixins; extract shared logic into composables.
- Do not mutate props directly.
- Do not break reactivity chains by destructuring reactive objects without `toRefs`.
- Keep `<script setup>` as the default component format.
- Name event handler functions with `handle` prefix (e.g., `handleClick`, `handleKeyDown`).
- Add accessibility attributes (`aria-label`, `tabindex`, `role`) to interactive elements proactively.
- Keep component files ≤150 lines; extract composables or subcomponents when exceeded.
- Use constants with arrow functions over function declarations (e.g., `const toggle = () =>`).
- Prioritize early returns and guard clauses to reduce nesting.
- **Superior Quality Audit**: For complex reactivity or Nuxt routes, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples

- "Use $vue to refactor this component with Composition API."
- "帮我用 Vue 3 Composition API 重构这个组件。"
- "设计一个 Pinia store 架构。"
- "用 Nuxt 3 搭建 SSR 项目。"
- "写一个可复用的 composable。"
- "强制进行 Vue 深度审计 / 检查响应式链与组件渲染结果。"
- "Use $execution-audit-codex to audit this Vue/Nuxt implementation for result idealism."
