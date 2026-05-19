# Multi-Host Adaptation Guide

GSD commands work across Desktop MCP, CLI, Codex, and Cursor.

## Desktop MCP

**Integration**: Via router-rs MCP server
**State Files**: Shared filesystem at `artifacts/current/`
**Stdio**: JSON-RPC over MCP protocol

```bash
# Equivalent to: router-rs --stdio-json
# But called via MCP invoke
```

### Desktop-Specific Notes

- Use `record_evidence` MCP tool after verification
- Use `session_checkpoint` MCP tool after checkpoints
- Use `goal_state_manage` for GOAL_STATE operations
- Use `rfv_loop_manage` for RFV loop operations

## Claude Code CLI

**Integration**: Via `configs/framework/claude-router-rs-hook.sh` → `router-rs claude hook`
**State Files**: Same filesystem under `artifacts/current/`
**Default workflow**: GSD lifecycle (`/gsd-new-project` … `/gsd-ship`); `/autopilot` remains opt-in legacy goal execution

### CLI-Specific Notes

- Hooks auto-inject continuity context on `UserPromptSubmit` / `Stop`
- `/gsd*` and `/autopilot` entries arm `GOAL_STATE` persistence (shared with Cursor)
- `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=1` enables goal drive on Cursor; Claude uses the same artifact layout
- `ROUTER_RS_RFV_LOOP_HOOK=1` enables RFV continuation hints where the host injects them

## Codex

**Integration**: Via codex_hooks.rs
**State Files**: Same filesystem
**Hooks**: codex-specific injection

### Codex-Specific Notes

- Use `rust-session-supervisor` / tmux for long sessions
- Hooks inject goal/RFV hints at SessionStart
- `build_framework_continuity_digest_prompt` appends GOAL_STATE

## Cursor

**Integration**: Via cursor_hooks.rs
**State Files**: Same filesystem
**Hooks**: beforeSubmit, SessionEnd

### Cursor-Specific Notes

- No tmux supervisor, rely on `artifacts/current` continuity
- Hooks inject `AUTOPILOT_DRIVE` and `RFV_LOOP_CONTINUE` hints
- `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0` disables autopilot drive

## Universal Patterns

Regardless of host, GSD commands:

1. Read state from `artifacts/current/<task_id>/`
2. Write evidence to `EVIDENCE_INDEX.json`
3. Update goal/RFV state files
4. Use stdio JSON for router-rs operations

## Host Detection

Check environment for host type:

```bash
# Desktop MCP
if [ -n "$ROUTER_RS_MCP_MODE" ]; then
    # Use MCP tools
fi

# CLI
if command -v rtk &> /dev/null; then
    # Use rtk hooks
fi

# Codex/Cursor
if [ -f ".cursor/hooks.json" ]; then
    # Use Cursor hooks
fi
```
