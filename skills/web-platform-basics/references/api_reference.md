# Web Platform API Reference

## Storage & Persistence
- `localStorage` / `sessionStorage` — synchronous, origin-scoped, ~5 MB
- `IndexedDB` — async, structured, large-capacity; use via `idb` wrapper when complexity warrants
- `Cache API` — paired with Service Worker for offline-first patterns
- Selection guide: ephemeral UI state → sessionStorage; small config → localStorage; structured/large data → IndexedDB

## Networking & Streaming
- `fetch()` — request/response model, AbortController for cancellation, streaming body via ReadableStream
- `Streams API` — ReadableStream, WritableStream, TransformStream for incremental processing
- `EventSource` (SSE) — server-push for real-time updates without WebSocket overhead
- `WebSocket` — full-duplex; prefer when bidirectional messaging is required
- `Beacon API` — reliable fire-and-forget analytics on page unload

## Navigation & Routing
- `History API` — pushState / replaceState / popstate for SPA-style navigation
- `Navigation API` (modern) — intercept navigations, transition lifecycle
- `URL` / `URLSearchParams` — safe URL construction and query parsing

## Observer APIs
- `IntersectionObserver` — lazy loading, infinite scroll, visibility tracking
- `ResizeObserver` — responsive component behavior without window resize
- `MutationObserver` — DOM mutation tracking for third-party or legacy integration
- `PerformanceObserver` — runtime performance monitoring (LCP, FCP, layout shifts)

## Web Components
- Custom Elements — `customElements.define()`, lifecycle callbacks
- Shadow DOM — encapsulated styling, `::part()` / `::slotted()` for theming
- HTML `<template>` and `<slot>` — declarative composition
- When to use: reusable widgets that must work across frameworks or in plain HTML

## Canvas & Graphics
- Canvas 2D context — drawing, compositing, pixel manipulation
- `OffscreenCanvas` — worker-based rendering for performance
- Basic WebGL bootstrapping — context creation, shader compilation, draw calls
- When to defer: complex 3D / game engines → recommend dedicated libraries (Three.js, PixiJS)

## Workers & Concurrency
- `Web Worker` — offload CPU-heavy computation (parsing, crypto, image processing)
- `SharedWorker` — shared state across tabs of the same origin
- `Transferable` objects — zero-copy transfer for ArrayBuffer, ImageBitmap
- `navigator.hardwareConcurrency` — scale worker pool sizing

## Clipboard & Drag-and-Drop
- `Clipboard API` — async read/write (text, rich content, files)
- `Drag and Drop API` — DataTransfer, drag events, custom drop zones
- Permission model: Clipboard read requires user gesture or Permissions API grant

## Media & Display
- `Fullscreen API` — element-level fullscreen toggle
- `Picture-in-Picture API` — floating video window
- `Screen Wake Lock API` — prevent screen dimming during media playback or presentations
- `MediaSession API` — customize OS media controls (play/pause, track info, artwork)

## PWA Fundamentals
- **Service Worker lifecycle**: install → waiting → activate → fetch interception
- **Caching strategies**: Cache-First (static assets), Network-First (dynamic), Stale-While-Revalidate (balance)
- **Web App Manifest**: `name`, `icons`, `start_url`, `display`, `theme_color`, `scope`
- **Workbox**: precaching, runtime caching, background sync, navigation preload
- **Install prompts**: `beforeinstallprompt` event, add-to-homescreen UX
- **Push notifications**: Push API + Notification API plumbing (server-side push subscription management is out of scope)
