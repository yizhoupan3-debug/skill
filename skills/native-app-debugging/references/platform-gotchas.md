# Native App Platform Gotchas Reference

> Expanded platform-specific known issues and workarounds for Tauri, Electron, Wails, and WkWebView.

## macOS + Tauri

| Feature | Problem | Diagnostic | Fix |
|---|---|---|---|
| Paste / Select-All / Cut | Keyboard shortcuts dead | Try Edit menu; check if app has a Menu | Add `Menu::os_default(&app.package_info().name)` in `main.rs` |
| Clipboard read/write | Silent failure | `tauri::api::clipboard` returns `Ok(None)` | Add `tauri-plugin-clipboard-manager` + entitlement `com.apple.security.automation.apple-events` |
| File drag-and-drop | Drop event not fired | Check `window.addEventListener("tauri://drag-drop")` | Set `allowlist.protocol.asset = true` in `tauri.conf.json` |
| Native file dialog | Returns empty path in packaged app | Works in dev, fails packaged | Add `com.apple.security.files.user-selected.read-write` entitlement |
| Deep links | App not launched from URL scheme | Check URL scheme registration | Register `protocol` in `tauri.conf.json` under `macOS.exceptionDomain` |

## macOS + WkWebView

| Feature | Problem | Diagnostic | Fix |
|---|---|---|---|
| `<datalist>` | No dropdown shown | Inspect DOM: element exists but no popup | Replace with custom `<input>` + `<ul>` dropdown |
| `navigator.clipboard.readText()` | Permission denied / silent fail | Check DevTools console | Use Tauri clipboard plugin instead of web clipboard API |
| `window.open()` | New window blocked | Check if `WKUIDelegate` is set | Implement `webView(_:createWebViewWith:for:windowFeatures:)` |
| IndexedDB persistence | Data lost on app restart in some configs | Check WkWebView data store type | Use persistent data store: `WKWebsiteDataStore.default()` |
| `fetch` to `localhost` | Blocked by App Transport Security | Network tab shows ATS error | Add `NSExceptionDomains` for `localhost` in `Info.plist` |
| Service Workers | Not supported in WkWebView | SW registration silently fails | Do not rely on SW in WkWebView; use Tauri's asset protocol |

## Electron

| Feature | Problem | Diagnostic | Fix |
|---|---|---|---|
| `ipcRenderer.invoke` | Returns undefined / hangs | Add `console.log` in `ipcMain.handle` | Verify channel name matches exactly (case-sensitive) |
| `contextBridge.exposeInMainWorld` | API not available in renderer | Check preload script is loaded | Set `preload: path.join(__dirname, 'preload.js')` in `BrowserWindow` |
| `ipcMain` not receiving | Message sent but never handled | Add catch-all `ipcMain.on('*', ...)` temporarily | Verify `webContents.send` vs `ipcRenderer.send` direction |
| `shell.openExternal` blocked | Opens nothing | Check Electron security policy | Add `shell` to `enabledCapabilities` or use `allowedURISchemes` |
| Protocol custom scheme | Resource not loaded | Network tab shows `ERR_FAILED` | Register with `protocol.registerFileProtocol` in `app.whenReady()` |
| GPU crash renderer | Renderer process crashes on launch | Check GPU flags in crash log | Add `--disable-gpu` flag for debugging; check GPU process logs |

## Wails (Go)

| Feature | Problem | Diagnostic | Fix |
|---|---|---|---|
| `runtime.EventsEmit` not received | JS listener fires but event never arrives | Add `console.log` in listener AND `fmt.Println` before emit | Ensure frontend is fully mounted before emitting; use `runtime.EventsOnce("ready", ...)` pattern |
| Go struct binding | JS receives `{}` or partial data | Log struct in Go before return | All struct fields must be **exported** (uppercase) |
| `wails.Run` panics | Crash on startup | Run with `wails dev` and check stderr | Check `AppID` uniqueness and `Assets` fs path |
| Window not appearing | App starts but no window | Check `wails dev` output | Verify `Assets` embed path and that `index.html` is at root |
| Hot reload not working | Changes not reflected | Check `wails dev` process | Use `wails dev -loglevel debug` to trace asset serving |
| Packaged app crashes | Works in dev, fails packaged | Run packaged binary from terminal to see stderr | Check if sidecar binaries are listed in `wails.json` `info.executableName` |

## Windows + WkWebView2

| Feature | Problem | Diagnostic | Fix |
|---|---|---|---|
| `window.open()` | Blocked by default | No new window appears | Implement `ICoreWebView2NewWindowRequestedEventArgs` handler |
| `navigator.clipboard` | Throws in insecure context | Console: `NotAllowedError` | Serve from HTTPS in prod; for dev, use `--allow-insecure-localhost` |
| `localStorage` | Cleared between sessions | Data disappears after app restart | Set user data folder path explicitly with `CoreWebView2EnvironmentOptions` |
| High-DPI rendering | Blurry UI | UI looks blurry on 150%+ scale | Set `SetProcessDpiAwareness` to `PROCESS_PER_MONITOR_DPI_AWARE` |
| DevTools | Cannot open DevTools in packaged app | Right-click context menu missing | Call `OpenDevToolsWindow()` programmatically in debug builds |

## Linux + Tauri

| Feature | Problem | Diagnostic | Fix |
|---|---|---|---|
| Clipboard paste | Silent fail in some WMs | `xclip -selection clipboard -o` works from terminal | Install `xclip` or `xdotool`; check if compositor supports clipboard protocol |
| System tray | Icon missing in GNOME | No tray icon shown | Install `gnome-shell-extension-appindicator`; or use `tray.set_icon_as_template(false)` |
| WebRTC / media | Camera/mic not accessible | Permission prompt never shows | Add `com.canonical.AppIndicator3` DBus rules |
| File open dialog | Opens behind app window | Dialog appears hidden | Use `window.set_focus()` before dialog; check window stacking |

## Debugging Logs by Framework

```bash
# Tauri (Rust backend logs)
RUST_LOG=debug cargo tauri dev 2>&1 | grep -E "ERROR|WARN|tauri"

# Tauri (frontend logs captured from WebView)
# Add to src-tauri/src/main.rs:
tauri::Builder::default()
  .plugin(tauri_plugin_log::Builder::default().build())

# Electron (main process)
electron . --enable-logging 2>&1

# Wails (dev mode with full log)
wails dev -loglevel debug 2>&1

# WkWebView (macOS) — enable WebInspector in Safari
defaults write NSGlobalDomain WebKitDeveloperExtras -bool true
# Then right-click in app → Inspect Element
```
