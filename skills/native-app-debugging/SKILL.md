---
name: native-app-debugging
description: |
  调试 Web-Native 边界的桌面 App 问题：Tauri、Electron、WkWebView、Wails、IPC、原生菜单与打包后失效。
  Use when a bug spans the frontend-web layer and the native OS/runtime layer.
short_description: Debug desktop app issues across the Web-Native boundary
trigger_hints:
  - Tauri crash
  - Electron IPC
  - WkWebView
  - macOS 原生问题
  - 原生菜单
  - clipboard 失效
metadata:
  version: "1.1.0"
  platforms: [codex, antigravity]
  tags:
    - native
    - desktop
    - tauri
    - electron
    - wkwebview
    - ipc
    - cross-layer
    - wails
risk: low
source: local
routing_layer: L3
routing_owner: owner
routing_gate: none
routing_priority: P3
session_start: n/a
allowed_tools:
  - shell
  - browser
  - python
approval_required_tools:
  - gui automation
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - runtime_evidence.md
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once
---

# native-app-debugging

Desktop-native debugging skill. Investigates bugs that arise at the boundary between
a web-based UI layer and the native OS/runtime layer — where generic frontend or backend
debugging skills lack the cross-layer context to efficiently diagnose the failure.

## When to use

- Tauri: Rust backend panic, Tauri command call failure, plugin initialization error, missing capability
- Tauri: IPC `invoke()` not responding, `tauri::command` return type mismatch
- WkWebView (macOS): HTML5 API silently failing (datalist, clipboard, file input quirks)
- WkWebView: WebView-to-native bridge deadlock or callback timeout
- macOS: missing default `Menu` causing system shortcuts (paste, select-all) to be dead
- Electron: main/renderer process crash, `ipcMain`/`ipcRenderer` channel mismatch
- Electron: `contextBridge` exposure errors, preload script failures
- **Wails** (Go): `runtime.EventsEmit` / `Bind` not reachable from JS, Go bridge panic at packaged runtime
- General native: clipboard/paste not working, drag-and-drop broken, system dialog fails
- Best for requests like:
  - "macOS 下 paste 快捷键没反应"
  - "Tauri invoke 没有响应"
  - "WkWebView 里 HTML5 XX API 不工作"
  - "Electron IPC 通信失败"
  - "打包后 app 失效，开发环境正常"
  - "原生菜单/右键菜单出不来"
  - "Wails Go 函数 JS 调不到"

## Do not use

- Pure frontend runtime bug without native layer involvement → use `$frontend-debugging`
- Pure Rust/Go backend crash without IPC/native context → use `$backend-runtime-debugging`
- Build/bundler failures (Vite, Webpack, cargo build, wails build) → use `$build-tooling`
- API integration debugging → use `$api-integration-debugging`
- The root cause is completely unknown → use `$systematic-debugging` first, then route here

## Task ownership and boundaries

This skill owns:
- Web-Native boundary failure classification
- WkWebView vs browser web standard divergence analysis
- Tauri command/plugin/IPC debugging
- Electron main↔renderer communication debugging
- macOS/Windows OS-level permission and API limitation diagnosis
- Packaging vs dev-mode behavioral divergence

This skill does not own:
- Pure frontend rendering bugs → `$frontend-debugging`
- Pure Rust panic without native interface involvement → `$backend-runtime-debugging`
- Tauri Rust business logic implementation → `$rust-pro`
- Build chain → `$build-tooling`
- **Dual-Dimension Audit (Pre: Cross-Layer-Logic, Post: IPC-Fidelity/OS-Native Results)** → `$execution-audit` [Overlay]

## Cross-layer diagnostic model

### Layer A: Web UI Layer
- Is the trigger (click/keyboard/paste) being captured at all?
- Is a web API (clipboard, file input, datalist) silently failing due to WkWebView/sandbox restrictions?
- Does the behavior change in a standard browser vs the embedded webview?

### Layer B: IPC / Bridge Layer
- For Tauri: is `invoke()` reaching the Rust handler? Add `println!` or `log::debug!` in the command.
- For Electron: is the `ipcRenderer.send()` landing on the correct channel in `ipcMain.on()`?
- Is a `contextBridge`/`allowlist` capability restricting the call?

### Layer C: Native / OS Layer
- Is a macOS system Menu absent, causing keyboard shortcuts to be disabled?
- Is a WkWebView `WKWebViewConfiguration` setting blocking an API?
- Is a macOS sandbox entitlement missing (e.g., `com.apple.security.files.user-selected.read-write`)?
- Is the native plugin compiled for the correct architecture (arm64 vs x86_64)?
- Does `RUST_LOG=debug` or a native crash reporter reveal a panic?

### Layer D: Packaging / Environment
- Does the failure only occur in the packaged app, not in `cargo tauri dev`?
- Are bundled assets (JS chunks, native plugins) being resolved differently?
- Is a sidecar binary or shell script missing from the bundle?

## Known platform gotchas

| Platform | API / Feature | Gotcha |
|---|---|---|
| macOS + WkWebView | `<datalist>` | Silently renders with no dropdown; use custom implementation |
| macOS + Tauri | Paste / Select-All | Requires explicit `Menu::os_default()` in `main.rs` |
| macOS + Tauri | File drag-and-drop | Requires `allowlist.protocol.asset = true` |
| macOS + Tauri | Clipboard read | Requires `tauri-plugin-clipboard-manager` + entitlement |
| Electron | `ipcRenderer` in renderer | Must be exposed via `contextBridge` in preload |
| Windows + WkWebView2 | `window.open` | Blocked by default; needs `WindowOpenDisposition` config |
| Windows + WkWebView2 | `navigator.clipboard` | Requires Focus + HTTPS or explicit permissions |
| Wails (Go) | `runtime.EventsEmit` | Events not received if frontend not mounted yet; use `ready` event guard |
| Wails (Go) | Go struct binding | Unexported fields silently omitted in JS; all fields must be exported |
| Linux + Tauri | WebKit `paste` | May require `xclip` or `xdotool` for clipboard access in some WMs |

## Output defaults

```markdown
## Native App Debugging Summary
- Framework: [Tauri / Electron / WkWebView standalone]
- Failure layer: [Web UI / IPC / Native OS / Packaging]
- Reproduction: dev mode / packaged / both

## Evidence
- Symptom: ...
- Layer where failure first appears: ...

## Root Cause
- ...

## Fix
- ...

## Verification
- Test in: dev mode / packaged app / both
```

## Hard constraints

- Always establish which layer the failure occurs in before proposing a fix.
- Do not assume a frontend fix when the Web-Native bridge is the likely cause.
- Always check if the issue is packaging-only vs dev-mode to narrow the layer.
- Label platform-version-specific behavior explicitly (macOS version, WkWebView version).
- **Superior Quality Audit**: For cross-boundary desktop apps, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).

## Trigger examples
- "强制进行原生应用深度审计 / 检查 IPC 通信与 OS 交互运行结果。"
- "Use $execution-audit to audit this native app bridge for IPC-fidelity idealism."

## References

- [references/platform-gotchas.md](references/platform-gotchas.md) — Extended platform-specific known issues: macOS/Tauri, WkWebView, Electron, Wails, Windows WkWebView2, Linux
