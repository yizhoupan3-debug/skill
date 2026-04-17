---
name: frontend-debugging
description: |
  Diagnose frontend runtime bugs with a five-layer model (component → state → rendering →
  network → environment). Use when: 白屏诊断、页面白屏、console 报错定位、组件不更新、状态不同步、
  交互失效、hydration error、路由空白、浏览器兼容问题、升级第三方库后崩溃。 Use when a frontend
  bug needs deeper investigation than generic systematic debugging.
metadata:
  version: "1.2.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - frontend
    - debugging
    - runtime
    - devtools
    - hydration
    - state-management
    - browser-compatibility
risk: low
source: local
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
allowed_tools:
  - shell
  - browser
  - python
  - node
approval_required_tools:
  - gui automation
---

# frontend-debugging

Frontend-specialized debugging skill. Provides domain-specific investigation
tools, mental models, and checklists for frontend runtime problems that generic
`$systematic-debugging` methodology cannot efficiently cover alone.

## When to use

- Frontend runtime bugs: blank screens, white screens, components not rendering
- Console errors: TypeError, ReferenceError, React warnings, Vue warnings
- Rendering anomalies: components not updating, flickering, style corruption, conditional rendering failures
- State management issues: Redux/Zustand/Pinia/Context desync, race conditions, stale closures
- Interaction bugs: clicks not responding, event bubbling/delegation anomalies, focus management
- Routing problems: blank routes, redirect loops, dynamic route parameter loss
- Browser compatibility: Safari/Firefox-specific bugs, missing polyfills
- DevTools-guided investigation: structured use of Network/Console/Elements/Performance/Sources panels
- SSR/SSG issues: hydration errors, `window is not defined`, async data not awaited
- React Server Components / Next.js App Router: `use client` boundary confusion, server-to-client data serialization failures, missing Suspense boundary
- Third-party library conflicts: version incompatibility, CSS overrides, global side effects
- Best for requests like:
  - "页面白屏了帮我排查"
  - "console 报了一堆 TypeError 但不知道哪里来的"
  - "这个组件为什么不更新"
  - "状态改了但视觉没变"
  - "点击按钮没反应"
  - "路由跳转后是空白页"
  - "Safari 下这个功能坏了"
  - "hydration mismatch 怎么排查"
  - "升级了第三方库后页面崩了"
  - "Next.js App Router `use client` 缺失 — 预渲染和客户端不匹配"

## Do not use

- The root cause is completely unknown and needs generic reproduction methodology first → use `$systematic-debugging`, then route here if the issue is frontend
- The issue is purely backend (API latency, database, server config) → use backend skills
- The issue is build/bundler failure, not runtime → use `$build-tooling`
- The issue is purely CSS layout without runtime behavior → use `$css-pro`
- The issue is API integration (CORS, 401, payload mismatch) without frontend-specific symptoms → use `$api-integration-debugging`
- The issue is purely performance (LCP, CLS, INP) without functional bugs → use `$performance-expert`
- The concern is code quality and patterns, not runtime bugs → use `$frontend-code-quality`

## Task ownership and boundaries

This skill owns:
- Frontend runtime bug classification and triage
- Five-layer diagnostic model for frontend issues
- DevTools-guided investigation workflows
- Framework-specific runtime debugging knowledge (React, Vue, Svelte, Next.js)
- SSR/hydration debugging patterns
- Browser compatibility investigation
- Third-party library conflict diagnosis

This skill does not own:
- Generic reproduction and hypothesis-testing methodology → `$systematic-debugging`
- Build/bundler/dependency resolution failures → `$build-tooling`
- CSS layout engineering → `$css-pro`
- API layer debugging → `$api-integration-debugging`
- Performance optimization → `$performance-expert`
- Code quality review → `$frontend-code-quality`
- Framework-specific feature implementation → `$react` / `$nextjs` / `$vue` / `$svelte`

If the task shifts to adjacent skill territory, route to:
- `$systematic-debugging` if the issue requires deeper generic root-cause isolation
- `$build-tooling` if the issue turns out to be a build/module problem
- `$performance-expert` if the issue is performance-related
- Framework skills for implementation fixes after the root cause is established

## Relationship with systematic-debugging

`$systematic-debugging` provides the generic debugging methodology (reproduce → evidence → trace → hypothesize → fix). This skill provides **frontend-specific diagnostic knowledge** that accelerates each of those steps:

- **Reproduce**: knows how to reproduce frontend issues using browser refresh states, incognito mode, device emulation, and framework dev mode flags
- **Evidence**: knows which DevTools panels to check, which console messages matter, which React/Vue devtools to inspect
- **Trace**: knows component trees, state flow, hydration boundaries, event propagation paths
- **Hypothesize**: maintains checklist of common frontend failure patterns per layer

Typical routing:
1. `$systematic-debugging` gates first if root cause is totally unknown
2. Once the issue is identified as frontend runtime, route to `$frontend-debugging`
3. Or route here directly if the symptoms are clearly frontend-specific from the start

## Five-layer diagnostic model

When diagnosing a frontend runtime issue, work through these layers in order:

### Layer 1: Component Layer
- Is the component mounted at all?
- Is the error boundary catching a crash?
- Is the component tree correct in React/Vue DevTools?
- Is `key` prop causing unexpected unmount/remount?
- Is lazy loading / code splitting failing silently?

### Layer 2: State Layer
- Is the state value correct in DevTools?
- Is the state update triggering a re-render?
- Is there a stale closure capturing old state?
- Is there a race condition between async state updates?
- Is the state management store (Redux/Zustand/Pinia) in sync?

### Layer 3: Rendering Layer
- Is the DOM actually updating (check Elements panel)?
- Is CSS hiding the update (display:none, opacity:0, z-index)?
- Is a conditional render `&&` or ternary evaluating wrong?
- Is a memoization (React.memo, useMemo, computed) preventing re-render?
- Is SSR/hydration producing a mismatch?

### Layer 4: Network/Resource Layer
- Is a required resource (JS chunk, CSS, image, font) failing to load?
- Is an API call failing silently?
- Is CORS blocking a request?
- Is a Service Worker serving stale content?
- Is a CDN or caching issue returning old assets?

### Layer 5: Environment Layer
- Is the issue browser-specific (Safari quirks, Firefox differences)?
- Is the issue device-specific (mobile viewport, touch events)?
- Is a browser extension interfering?
- Is an environment variable missing or wrong?
- Is there a polyfill gap?

## Pattern checklists & DevTools guide

> Full checklists: [references/checklists.md](references/checklists.md)

Key patterns covered: blank screen, component not updating, event handler not firing, hydration mismatch, third-party library conflict. DevTools panels: Console, Elements, Network, Sources, Application.

## Tool Selection Matrix

When browser tooling is available (Codex with `browser-mcp` or `playwright`):

| Five-Layer | Preferred Tool | Key Call |
|---|---|---|
| **Layer 1 (Component)** | React/Vue DevTools → console injection | `console.log(document.querySelector('[data-reactroot]'))` |
| **Layer 2 (State)** | Browser console / Redux DevTools | `window.__REDUX_DEVTOOLS_EXTENSION__` / Zustand `getState()` |
| **Layer 3 (Rendering)** | `browser_get_state` | `browser_get_state(include=["summary", "interactive_elements"])` |
| **Layer 4 (Network)** | `browser_get_network` | `browser_get_network(resourceTypes=["fetch","xhr"], sinceSeconds=30)` |
| **Layer 5 (Environment)** | `browser_screenshot` + UA check | `browser_screenshot()` + `navigator.userAgent` console injection |

> **Layer 5 UA check example**: `mcp_browser-mcp_browser_get_text` → scan for browser/OS info or inject `console.log(navigator.userAgent, navigator.platform)` via `mcp_browser-mcp_browser_click` on console.

> When NOT in a browser-enabled environment, fall back to DevTools manual inspection per the checklists reference.

## Output defaults

Default output: layer identification → symptom & evidence → root cause → fix or next step → verification.
## Anti-laziness integration

This skill activates `$anti-laziness` enforcement when:
- A fix is applied to a component without first checking the DevTools Evidence layer.
- Output claims "should work now" without providing screenshot or network trace.
- The debugging jumps directly to code edits without working through the five layers.
- Two or more Layer-2 (state) fixes attempted without verifying Layer-3 (rendering) evidence.

## Hard constraints

- Do not skip the five-layer model; work through it systematically.
- Do not guess-and-patch without checking DevTools evidence first.
- Do not conflate build errors with runtime errors (route to `$build-tooling`).
- Always identify which framework before applying framework-specific debugging steps.
- If the issue spans multiple layers, address the lowest layer first.
- **No passive finish**: must provide `browser_screenshot` or DevTools output before closing the investigation.
- **No context-begging**: use `browser_get_state`, `browser_get_network`, or `browser_get_text` before asking the user to "open DevTools".
