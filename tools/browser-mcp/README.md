# browser-mcp

A lean MCP browser server optimized for agent use, built on Playwright.

## v0.2.0 Highlights

- **Inline screenshot** — `browser_screenshot` returns a base64 PNG directly in the tool result (no separate file read needed)
- **Session persistence** — `browser_save_session` / `browser_restore_session` persist cookies + localStorage across conversations
- **Enhanced network tracking** — captures request bodies, failed requests (`errorText`), and timing (`durationMs`)
- **Stable element refs** — fingerprints prefer `data-testid` first; no DOM-ordinal dependency on dynamic pages
- **HTTP transport** — optional `--transport http --port <n>` for remote / multi-agent use
- **Runtime diagnostics** — `browser_diagnostics` for self-inspection
- **Screenshot housekeeping** — oldest files auto-removed when directory exceeds `maxScreenshots` (default 100)

## Scripts

```bash
npm run build   # compile TypeScript
npm run check   # typecheck only
npm run test    # run vitest integration tests
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
node dist/index.js
# Flags: --headless true|false  --engine chromium|firefox|webkit  --capture-body
#        --runtime-attach-artifact-path /abs/path/runtime-attach-descriptor.json|.../ATTACHED_RUNTIME_EVENT_HANDOFF.json|.../TRACE_RESUME_MANIFEST.json|.../runtime_event_transports/session__job.json
#        --runtime-attach-descriptor-path /abs/path/runtime-attach-descriptor.json
#        --runtime-binding-artifact-path /abs/path/runtime_event_transports/session__job.json
#        --runtime-handoff-path /abs/path/ATTACHED_RUNTIME_EVENT_HANDOFF.json
#        --runtime-resume-manifest-path /abs/path/TRACE_RESUME_MANIFEST.json
```

### HTTP (Streamable HTTP transport)
```bash
node dist/index.js --transport http --port 3721
```

## Codex integration

```bash
python3 /Users/joe/Documents/skill/scripts/install_browser_mcp_codex.py
```

Adds a `browser-mcp` stdio entry to `/Users/joe/.codex/config.toml`. Restart Codex after updating.

### Smoke test

```bash
/Users/joe/Documents/skill/tools/browser-mcp/scripts/start_browser_mcp.sh
```

### Routing

When `browser-mcp` is enabled in Codex, browser tasks prefer `browser_*` MCP tools.
The Playwright skill CLI flow is the fallback when the MCP server is unavailable.

## Network body capture

Disabled by default to keep token usage low. Enable via:

```bash
node dist/index.js --capture-body
# or in BrowserRuntime: new BrowserRuntime({ captureBody: true })
```

Captures request `postData` and JSON response bodies up to 4 KB each.

## Runtime attach diagnostics

If you already have a Rust-first runtime attach descriptor, browser-mcp can
consume it directly for self-inspection:

```bash
node dist/index.js --runtime-attach-artifact-path /abs/path/runtime-attach-descriptor.json
# or
BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH=/abs/path/runtime-attach-descriptor.json node dist/index.js
```

`--runtime-attach-artifact-path` is the preferred Rust-first entrypoint. The
older `--runtime-binding-artifact-path` and `--runtime-handoff-path` flags are
still accepted as compatibility aliases, and `TRACE_RESUME_MANIFEST.json` can
also flow through the same canonical attach-artifact flag when recovery is the
only persisted entrypoint you have. When an explicit attach descriptor already
contains enough artifact hints, browser-mcp now canonicalizes it through the
same Rust attach bridge first, so provenance and resolution fields stay aligned
with the artifact-based entrypoints. Once an entrypoint has been canonicalized
through that bridge, browser-mcp now also prefers the Rust-resolved replay
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

If you do not pass an explicit runtime attach input, the bundled
`start_browser_mcp.sh` launcher now auto-discovers the newest replay-capable
runtime attach input under `framework_runtime/artifacts/scratch/`. It now
keeps checkpoint resume manifests as canonical attach artifacts when they are
the freshest recovery anchor, falls back to transport binding artifacts when
needed, and also resolves sqlite-backed logical attach paths by reading
`runtime_checkpoint_store.sqlite3` when the binding JSON does not exist as a
local file.
