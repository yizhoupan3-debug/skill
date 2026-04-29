---
name: nextjs
description: |
  Deliver Next.js 14/15 applications with correct App Router, Server Component,
  and caching boundaries.
  Use for Next.js development, SSR/SSG strategy, streaming, Server Actions, hydration, waterfalls, and Vercel-aligned React best practices.
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - nextjs
    - app-router
    - server-components
    - ssr
    - caching
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
  - nextjs
  - app router
  - server components
  - ssr
  - caching
  - hydration
  - Vercel

---

# nextjs

This skill owns Next.js-first engineering work: App Router architecture, data fetching, caching, and deployment optimization.

## When to use

- Building or reviewing Next.js applications
- Implementing data fetching, caching, and revalidation strategies
- Working with React Server Components and Server Actions
- Optimizing Next.js performance (Core Web Vitals, bundle size)

## Do not use

- The project uses Pages Router exclusively and won't migrate
- The task is about React without Next.js (use `$react`)
- The task is about pure API development without a frontend

## Task ownership and boundaries

This skill owns:
- Next.js App Router architecture and routing
- Server Components and Server Actions
- data fetching and caching strategies
- middleware and edge functions
- deployment and optimization

This skill does not own:
- pure React component design without Next.js → `$react`
- general web performance outside Next.js → `$performance-expert`
- backend API design without Next.js context → `$node-backend`
- broader React rendering mechanics without Next.js context → `$react`

If the task shifts to adjacent skill territory, route to:
- `$react` for pure React patterns
- `$performance-expert` for broader web perf
- **Dual-Dimension Audit (Pre: RSC/Caching, Post: Streaming/Hydration Results)** → runtime verification gate

## Required workflow

1. Confirm the task shape:
   - object: route, layout, page, API handler, middleware, cache
   - action: build, refactor, debug, optimize, deploy, migrate
   - constraints: Next.js version, App/Pages Router, deployment target, caching needs
   - deliverable: route code, architecture design, optimization, or migration plan
2. Clarify Next.js version and router strategy.
3. Design with Server Components by default.
4. Implement proper data fetching and caching strategies.
5. Verify with `next build` and performance checks.

## Core workflow

### 1. Intake
- Identify Next.js version and whether App Router or Pages Router.
- Check deployment target (Vercel, self-hosted, Docker, static export).
- Inspect existing route structure and data fetching patterns.

### 2. Execution
- Use Server Components by default; only add `'use client'` for interactivity.
- Colocate data fetching with the component that needs it.
- Use proper caching and revalidation strategies.
- Avoid client-side waterfalls.

### 3. Validation / recheck
- Run `next build` to verify no build-time errors.
- Check for waterfall patterns in data fetching.
- Verify server/client boundaries are optimally placed.
- Validate with Lighthouse if performance is a concern.

## Capabilities

### App Router Architecture
- File-based routing with `app/` directory
- `layout.tsx`, `page.tsx`, `loading.tsx`, `error.tsx`, `not-found.tsx`
- Route groups `(group)` for layout organization
- Parallel Routes `@slot` and Intercepting Routes `(.)`, `(..)`, `(...)`
- Dynamic routes `[slug]`, catch-all `[...slug]`, optional `[[...slug]]`

### Server Components & Actions
- React Server Components (RSC) as default
- `'use client'` boundary placement strategy
- Server Actions with `'use server'` for mutations
- Extract Server Actions into dedicated files (e.g., `actions.ts` or `actions/`) instead of inline inside components
- `useFormStatus`, `useFormState`, `useOptimistic`
- Progressive enhancement with form actions
- `next-safe-action` for type-safe, validated Server Actions
- Zod schema validation for form inputs and API payloads
- Model expected errors as return values (e.g., using `ActionResponse`)
- Handle unexpected errors gracefully using `error.tsx`, and catch root layout errors with `global-error.tsx`

### Architecture & Project Structure
- Organize files by feature or domain (e.g., `features/`, `components/`, `lib/`) in App Router
- Implement a Data Access Layer (DAL) (e.g., `services/` or `data-access/`) to encapsulate database queries, keeping Server Components free of direct DB logic
- Centralize environment configurations and validate them using Zod (e.g., T3 Env pattern)

### Data Fetching & Caching
- Acknowledge Next.js 15 cache default changes (fetch is `no-store` by default, whereas 14 was `force-cache`)
- Use `fetch` with explicit caching strategies (`{ cache: 'force-cache' }` or `{ next: { revalidate } }`)
- Use `cache()`, `unstable_cache()` for expensive request memoization and data transformations
- ISR: `revalidatePath()`, `revalidateTag()` for targeted cache invalidation
- Dynamic rendering: `cookies()`, `headers()`, `searchParams`
- Streaming with `Suspense` and `loading.tsx`
- `generateStaticParams` for static generation

### Middleware & Edge
- `middleware.ts` for request interception
- Edge Runtime vs Node.js Runtime tradeoffs
- Geo-based routing and A/B testing
- Authentication middleware patterns
- Rate limiting at the edge

### Performance & Web Vitals Clarification
- Image optimization with `next/image`
- Font optimization with `next/font`
- Script loading with `next/script`
- Bundle analysis with `@next/bundle-analyzer` and `source-map-explorer`
- Partial prerendering (PPR)
- React Compiler for automatic memoization
- For deeper Core Web Vitals (LCP, CLS, INP) analysis, delegate to `$performance-expert`

### Authentication & Security
- NextAuth.js / Auth.js integration
- Middleware-based auth guards
- CSRF protection with Server Actions
- Use React Taint APIs (`taintObject`, `taintUniqueValue`) to prevent leaking sensitive objects to the client
- Environment variable management

### Deployment
- Vercel deployment and configuration
- Self-hosting with Node.js or Docker
- Output modes: `standalone`, `export`
- Edge functions and serverless configuration

## Output defaults

Default output should contain:
- Next.js context and routing assumptions
- architecture / implementation approach
- validation notes and caching strategy

Recommended structure:

````markdown
## Next.js Summary
- Version: ...
- Router: App / Pages
- Deployment: ...

## Changes / Guidance
- ...

## Validation / Risks
- Build: ...
- Caching strategy: ...
````

## Hard constraints

- Do not default to `'use client'` when Server Components work.
- Do not fetch data client-side when server-side fetching is viable.
- Do not bypass Next.js caching without understanding revalidation impact.
- Do not mix App Router and Pages Router patterns in the same route.
- Do not ignore streaming/Suspense opportunities for large data loads.
- Verify that middleware does not block critical render paths.
- Use Zod for all form and API payload validation and environment variables.
- Extract database queries to a Data Access Layer (DAL); avoid complex DB logic directly in Server Components.
- Separate Server Actions into dedicated files to prevent cross-boundary issues.
- Use `next-safe-action` for type-safe Server Actions with consistent `ActionResponse` types.
- Delegate deep performance and Core Web Vitals profiling to `$performance-expert`.
- **Superior Quality Audit**: For production-critical routes, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## References

- [references/vercel-best-practices.md](references/vercel-best-practices.md)

## Trigger examples

- "Use $nextjs to build an App Router project with streaming."
- "帮我用 Next.js App Router 搭建项目。"
- "这个页面应该用 SSR 还是 SSG？"
- "Server Actions 怎么做表单提交？"
- "Next.js 15 caching 最佳实践怎么做？"
- "Next.js 项目怎么切分 Data Access Layer？"
- "强制进行 Next.js 生产环境审计 / 核心路由 RSC 边界核查。"
- "Use the runtime verification gate to audit this Next.js App Router implementation."
