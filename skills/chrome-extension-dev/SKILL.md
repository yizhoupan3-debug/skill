---
name: chrome-extension-dev
description: |
  Produce Chrome extensions for Manifest V3: Service Workers, minimal permissions, secure
  content scripts, and Chrome API usage. Deliver manifest/background/content-script/popup
  packages ready for Chrome Web Store. Use when the user asks about Chrome extension
  development, Manifest V3 migration, content-script/Service-Worker communication, or
  phrases like 'Chrome 插件', '浏览器扩展', 'content script'.
metadata:
  version: "1.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - chrome-extension
    - manifest-v3
    - browser-extension
    - service-worker
    - chrome-apis

routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - Chrome 插件
  - 浏览器扩展
  - content script
  - Manifest V3 migration
  - content-script
  - Service-Worker communication
  - chrome extension
  - manifest v3
  - browser extension
  - service worker
---

# chrome-extension-dev

This skill owns Chrome extension engineering: Manifest V3 architecture, Service Workers, content scripts, Chrome APIs, and extension publishing.

## When to use

- Building or reviewing Chrome browser extensions
- Migrating from Manifest V2 to V3
- Working with Chrome APIs (storage, tabs, messaging, alarms)
- Designing extension architecture (popup, options, content scripts, background)
- Best for requests like:
  - "帮我开发一个 Chrome 插件"
  - "Manifest V2 怎么迁移到 V3"
  - "content script 和 background 怎么通信"
  - "Chrome 扩展怎么请求权限"

## Do not use

- The task is general web development without extension context → use framework skills
- The task is Firefox-only extension → adapt patterns but note divergences
- The task is primarily about web scraping without extension context → use `$web-scraping` (or `$playwright` for browser automation)

## Task ownership and boundaries

This skill owns:
- Manifest V3 configuration and structure
- Service Worker lifecycle and patterns
- Content scripts injection and DOM interaction
- Chrome APIs usage (storage, tabs, messaging, alarms, notifications, contextMenus)
- Extension popup and options page design
- Extension security and permissions model
- Chrome Web Store publishing

This skill does not own:
- general web app development without extension context
- Firefox/Safari extension-specific APIs
- server-side APIs that the extension consumes
- **Dual-Dimension Audit (Pre: Manifest/Script-Logic, Post: Runtime-Permission/DOM-Interaction Results)** → `$execution-audit` [Overlay]

## Capabilities

### Manifest V3
- `manifest.json` configuration and fields
- Permissions: `activeTab`, `storage`, `tabs`, `contextMenus`, `alarms`, host permissions
- Content Security Policy for extensions
- `web_accessible_resources` for asset exposure
- Migration from Manifest V2 to V3

### Service Worker (Background)
- Event-driven architecture (no persistent background page)
- `chrome.runtime.onInstalled`, `chrome.runtime.onMessage`
- `chrome.alarms` for periodic tasks
- Handling extension lifecycle events
- Offscreen documents for DOM access in background

### Content Scripts
- Injection via `manifest.json` or `chrome.scripting.executeScript`
- CSS injection and DOM manipulation
- Isolated world vs main world execution
- `MutationObserver` for dynamic page monitoring
- Message passing to/from Service Worker

### Chrome APIs
- `chrome.storage` (local, sync, session) for data persistence
- `chrome.tabs` for tab management
- `chrome.runtime.sendMessage` / `chrome.runtime.onMessage` for messaging
- `chrome.contextMenus` for right-click menus
- `chrome.notifications` for user notifications
- `chrome.action` for toolbar icon behavior
- `chrome.sidePanel` for side panel UI

### Extension UI
- Popup page (HTML/CSS/JS or framework-based)
- Options page (embedded or standalone)
- Side panel
- DevTools panels
- Using React/Vue/Svelte in extension pages

### Security
- Content Security Policy enforcement
- Minimum required permissions
- Host permission best practices
- XSS prevention in content scripts
- Secure message passing patterns

## Hard constraints

- Do not request permissions beyond what the extension needs.
- Do not use Manifest V2 patterns (`background.page`, `chrome.browserAction`) in V3 projects.
- Do not assume persistent background; Service Workers are event-driven.
- Always validate message origins in `chrome.runtime.onMessage` handlers.
- Do not inject content scripts into all URLs unless explicitly required.
- Store sensitive data in `chrome.storage.session` (not `local`) when possible.
- **Superior Quality Audit**: For production browser extensions, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## Cross-Browser Compatibility

### WebExtensions API (Firefox)
- Firefox supports the same core APIs via the `browser.*` namespace (Promise-based)
- Use polyfill `webextension-polyfill` (Mozilla) for unified `browser.*` API across Chrome and Firefox
- Key differences:
  - Firefox uses `browser_specific_settings.gecko.id` in manifest
  - No `chrome.sidePanel` API in Firefox (use sidebar_action instead)
  - `declarativeNetRequest` support is newer and may differ in rule limits
  - Firefox supports Promise-based API natively; Chrome requires callbacks or polyfill

### Safari Web Extensions
- Requires Xcode and `safari-web-extension-converter` to package
- Subset of WebExtensions API; check compatibility per-API
- No background Service Worker; uses non-persistent background page
- Must be distributed via Mac App Store or TestFlight

### Cross-Browser Development Workflow
- **`web-ext` CLI** (Mozilla): lint, run, build, and sign extensions
  ```bash
  npx web-ext lint
  npx web-ext run --target=firefox-desktop
  npx web-ext build
  ```
- Use feature detection over browser sniffing:
  ```javascript
  if (typeof browser !== 'undefined' && browser.sidebarAction) { /* Firefox */ }
  if (chrome.sidePanel) { /* Chrome 114+ */ }
  ```
- Maintain a compatibility matrix for APIs used in the extension
- Test on Chrome, Firefox, and Edge (Chromium) at minimum

## Trigger examples

- "Use $chrome-extension-dev to build a Chrome extension with Manifest V3."
- "帮我开发一个 Chrome 插件，要用 Manifest V3。"
- "content script 怎么和 Service Worker 通信？"
- "Chrome 扩展怎么发布到 Web Store？"
- "强制进行 Chrome 扩展深度审计 / 检查 Manifest 配置与 Runtime 运行结果。"
- "Use $execution-audit to audit this Chrome extension for runtime-interaction idealism."
