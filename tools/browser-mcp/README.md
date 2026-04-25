# browser-mcp

A lean MCP browser server optimized for agent use. The stdio server is the Rust
implementation in `router-rs`; the launcher no longer falls back to the legacy
TypeScript runtime.

## Rust-first Highlights

- **Rust stdio entrypoint** — `router-rs browser mcp-stdio` is the default runtime path
- **Inline screenshot** — `browser_screenshot` returns a base64 PNG directly in the tool result (no separate file read needed)
- **Session persistence** — `browser_save_session` / `browser_restore_session` persist cookies across conversations
- **Network tracking** — records recent requests, response status/type, and failed requests (`errorText`)
- **Stable element refs** — fingerprints prefer `data-testid` first; no DOM-ordinal dependency on dynamic pages
- **Runtime diagnostics** — `browser_diagnostics` for self-inspection

## Scripts

```bash
/Users/joe/Documents/skill/tools/browser-mcp/scripts/start_browser_mcp.sh
```

## Tools (16 total)

| Tool | Description |
|---|---|
| `browser_open` | Open a URL in the current session |
| `browser_tabs` | List or switch tabs |
| `browser_close` | Close a tab or session |
| `browser_get_state` | Compressed page state + diff |
| `browser_get_elements` | Filtered interactive elements |
| `browser_get_text` | Visible text (page or scoped) |
| `browser_get_network` | Recent requests incl. failed + timing |
| `browser_screenshot` | **Inline PNG** in tool result |
| `browser_click` | Click an indexed element |
| `browser_fill` | Fill an input, optionally submit |
| `browser_press` | Keyboard key press |
| `browser_wait_for` | Wait for text / element / URL / idle |
| `browser_save_session` | Save cookies + localStorage to disk |
| `browser_restore_session` | Restore a saved session snapshot |
| `browser_get_attached_runtime_events` | Replay runtime events via a Rust attach descriptor |
| `browser_diagnostics` | Runtime health info |

## Startup options

### stdio (default)
```bash
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml browser mcp-stdio --repo-root /Users/joe/Documents/skill
# Flags: --headless true|false
#        --runtime-attach-artifact-path /abs/path/runtime-attach-descriptor.json|.../ATTACHED_RUNTIME_EVENT_HANDOFF.json|.../TRACE_RESUME_MANIFEST.json|.../runtime_event_transports/session__job.json
#        --runtime-attach-descriptor-path /abs/path/runtime-attach-descriptor.json
```

## Smoke test

```bash
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n' | ./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml browser mcp-stdio --repo-root /Users/joe/Documents/skill
```

## Routing

Browser tasks prefer the first-class `browser_*` MCP tools when this server is
available. Project install state is Rust-owned and no longer managed by a Python
installer under `scripts/`.

## Network body capture

Rust browser-mcp keeps the default network surface compact. The old TypeScript
`--capture-body` path is not part of the Rust stdio entrypoint.

## Runtime attach diagnostics

If you already have a Rust-first runtime attach descriptor, browser-mcp can
consume it directly for self-inspection:

```bash
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml browser mcp-stdio --repo-root /Users/joe/Documents/skill --runtime-attach-artifact-path /abs/path/runtime-attach-descriptor.json
# or
BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH=/abs/path/runtime-attach-descriptor.json ./tools/browser-mcp/scripts/start_browser_mcp.sh
```

`--runtime-attach-artifact-path` is the canonical Rust-first entrypoint for
persisted attach artifacts, including `TRACE_RESUME_MANIFEST.json`,
`ATTACHED_RUNTIME_EVENT_HANDOFF.json`, and
`runtime_event_transports/session__job.json`. When an explicit attach descriptor
already contains enough artifact hints, browser-mcp canonicalizes it through
the same Rust attach bridge first, so provenance and resolution fields stay
aligned with the artifact-based entrypoint. Once an entrypoint has been
canonicalized through that bridge, browser-mcp prefers the Rust-resolved replay
payload directly instead of re-deriving the trace path locally.

Then `browser_diagnostics` includes an `attachedRuntime` block with descriptor
status, replay readiness, trace path, the concrete input artifact kind
(`attach_descriptor` / `binding_artifact` / `handoff` / `resume_manifest`),
resolution-source hints for binding/handoff/resume/trace paths, and the latest
replayable event summary. You can also call
`browser_get_attached_runtime_events` to consume replayable runtime events
through that same attach descriptor; replay results now include a lighter
`replayContext` mirror so consumers can read attach provenance without
re-parsing the full diagnostics block.

The legacy `start_browser_mcp.sh` launcher delegates directly to the Rust
launcher and does not require `dist/index.js`, `node`, or `npm`; MCP config can
call the same Rust-owned launcher directly.
