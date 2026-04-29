---
name: web-platform-basics
description: |
  Explain and fix browser-native behavior at the platform layer before reaching
  for framework abstractions.
  Use when the user asks about vanilla frontend, 原生 JS, HTML/CSS 布局, DOM
  操作, 事件冒泡, 表单行为, z-index/overflow, Web APIs, Service Worker, Web
  Components, or platform-level diagnosis before framework-specific solutions.
metadata:
  version: "1.0.1"
  platforms: [codex]
  tags:
    - html
    - css
    - dom
    - forms
    - flexbox
    - grid
    - responsive

routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 原生 JS
  - HTML
  - CSS 布局
  - DOM 操作
  - 事件冒泡
  - 表单行为
  - z-index
  - overflow
  - Web APIs
  - Service Worker

---

# web-platform-basics

This skill owns browser-native frontend fundamentals when the problem should be solved at the HTML/CSS/DOM/platform layer before reaching for a framework skill.

## When to use

- The user asks about plain HTML, CSS, DOM, events, forms, or browser APIs
- The task involves Flexbox, Grid, responsive layout, semantic HTML, native validation, or browser storage
- The user wants to debug vanilla JS interactions or page behavior without introducing React/Vue/Svelte
- The task is about browser defaults such as focus, submit behavior, event propagation, layout flow, stacking, overflow, or rendering primitives
- Best for requests like:
  - "帮我改这个纯 HTML/CSS 页面布局"
  - "为什么这个 DOM 事件冒泡/默认行为不对"
  - "原生表单校验、提交、focus 怎么工作"
  - "不要上框架，直接用浏览器原生能力实现"

## Do not use

- The task is CSS **engineering**: architecture decisions, design systems, animation optimization, CSS methodology selection → use `$css-pro`
- The task is **Tailwind CSS** configuration, theming, or plugins → use `$css-pro`
- The main task is framework-specific React/Next/Vue/Svelte implementation → use the relevant framework skill
- The task is primarily a11y auditing against WCAG rather than platform implementation → use `$accessibility-auditor`
- The task is browser automation or live-page reproduction → use the built-in browser/browser-use capability
- The task is visual critique from screenshots rather than DOM/CSS mechanics → use `$visual-review`
- The task is SEO audit or meta tag / structured data optimization → use `$seo-web`
- The task is HTML email template development (table-based layout, Outlook compatibility) → use `$email-template`
- The task is web scraping / data extraction strategy → use `$web-scraping`

> **Decision guide: `web-platform-basics` vs `css-pro`**
> - "Why does this element overflow/stack/collapse this way?" → `web-platform-basics` (mechanism)
> - "How should I architect a responsive layout system?" → `css-pro` (engineering)
> - "How does Flexbox alignment work?" → `web-platform-basics` (mechanism)
> - "Design a Grid-based component layout strategy" → `css-pro` (engineering)

## Task ownership and boundaries

This skill owns:
- semantic HTML and document structure
- CSS layout and rendering primitives
- DOM querying, mutation, events, and browser APIs
- form behavior, validation, submission, and input states
- browser-native responsiveness and progressive enhancement
- Web Storage (localStorage / sessionStorage), IndexedDB, Cache API
- Fetch API, Streams API (ReadableStream / WritableStream)
- History API, Navigation API, URL / URLSearchParams
- Observer APIs (IntersectionObserver, ResizeObserver, MutationObserver, PerformanceObserver)
- Web Components (Custom Elements, Shadow DOM, HTML templates, slots)
- Canvas 2D drawing and basic WebGL bootstrapping
- Web Workers and SharedWorker (compute offloading, not framework-level state)
- Clipboard API, Drag and Drop API
- Fullscreen API, Picture-in-Picture API
- PWA fundamentals (Service Worker lifecycle, Web App Manifest, offline caching strategies, Workbox integration, install prompts, push notification plumbing)

This skill does not own:
- framework component architecture
- full accessibility compliance audits by itself
- browser automation workflows
- asset pipeline or bundler configuration

If the task shifts to adjacent skill territory, route to:
- `$react`
- `$nextjs`
- `$vue`
- `$svelte`
- `$accessibility-auditor`
- `$build-tooling`
- `$seo-web` for SEO-related meta/structured-data work
- `$email-template` for HTML email development
- `$web-scraping` for web data extraction

## Required workflow

1. Confirm the task shape:
   - object: page, DOM subtree, form, layout, browser API usage
   - action: build, debug, explain, refactor, review
   - constraints: browser support, no framework, responsive behavior, accessibility baseline
   - deliverable: code, diagnosis, review findings, or implementation plan
2. Identify whether the root cause is structure, style, event flow, browser default behavior, or API misuse.
3. Solve at the lowest correct layer first.
4. Validate behavior across layout, interaction, and fallback expectations.
5. Call out any browser-compatibility or accessibility follow-up explicitly.

## Core workflow

### 1. Intake
- Inspect the relevant HTML/CSS/JS before proposing abstractions.
- Determine whether the issue comes from DOM structure, CSS cascade/layout, event handling, or browser defaults.
- Note any constraints on browser support, no-JS fallback, or responsive breakpoints.

### 2. Execution
- Prefer semantic elements and native behavior over custom reimplementation.
- Use CSS layout primitives intentionally: normal flow before positioning hacks, Flexbox/Grid before nested workaround structures.
- For JS behavior, trace event target/currentTarget, default actions, timing, and DOM state transitions.
- When forms are involved, verify names, values, validation, submission method, and focus/error behavior.
- Keep fixes minimal and platform-native unless there is a clear product requirement otherwise.

### 3. Validation / recheck
- Recheck layout at likely viewport sizes.
- Recheck keyboard and pointer behavior for affected elements.
- Verify DOM state and browser defaults after the change.
- If browser-specific caveats remain, state them explicitly.

## Capabilities

Covers: Storage & Persistence, Networking & Streaming, Navigation & Routing, Observer APIs, Web Components, Canvas & Graphics, Workers & Concurrency, Clipboard & D&D, Media & Display, PWA Fundamentals.

> Full API reference: [references/api_reference.md](references/api_reference.md)

## Output defaults

Default output should contain:
- platform-level diagnosis or implementation summary
- concrete HTML/CSS/DOM changes
- browser or accessibility caveats

## Hard constraints

- Do not introduce a framework when the task is explicitly browser-native.
- Do not replace semantic HTML with div-heavy structures without justification.
- Tie layout bugs to actual DOM/CSS mechanics, not just visual descriptions.
- Always distinguish browser default behavior from custom script behavior.
- Proactively add accessibility attributes to all interactive elements.
- Use semantic elements over generic `<div>` where appropriate.
