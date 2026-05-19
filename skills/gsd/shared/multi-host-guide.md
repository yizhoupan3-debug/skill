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
**Default workflow (all hosts)**: GSD lifecycle (`/gsd-new-project` … `/gsd-ship`); `/autopilot` remains opt-in legacy goal execution

### CLI-Specific Notes

- Hooks auto-inject continuity context on `UserPromptSubmit` / `Stop`
- **Execution-zone** `/gsd-execute-phase`, `/gsd-verify-work`, `/gsd-ship`, and `/autopilot` arm goal drive; **pre-exec** `/gsd-new-project`, `/gsd-plan-phase`, `/gsd-discuss-phase` do **not** (see `phase-boundaries.md`)
- `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=1` enables goal drive on Cursor; Claude uses the same artifact layout
- `ROUTER_RS_RFV_LOOP_HOOK=1` enables RFV continuation hints where the host injects them

## Default framework (all hosts)

- **GSD is the default lifecycle** on every closed-set host (`codex-cli`, `cursor`, `claude-code`, `claude-desktop`). Entrypoints are registered in `RUNTIME_REGISTRY.json` → `framework_commands.gsd`.
- Host **projection** (`.claude/rules/framework.md`, `~/.cursor/rules/framework.mdc`, `.codex/prompts/framework.md`) and root `AGENTS.md` all state the same GSD chain; host-specific differences are **hooks/MCP only**, not a different default workflow.

## Codex

**Integration**: Via codex_hooks.rs
**State Files**: Same filesystem
**Hooks**: codex-specific injection

### Codex-Specific Notes

- **Same default GSD lifecycle**; `/gsd` aliases registered for `codex-cli`
- Use `rust-session-supervisor` / tmux for long sessions
- Hooks inject goal/RFV hints at SessionStart
- `build_framework_continuity_digest_prompt` appends GOAL_STATE

## Cursor

**Integration**: Via cursor_hooks.rs
**State Files**: Same filesystem
**Hooks**: beforeSubmit, SessionEnd

### Cursor-Specific Notes

- **Same default GSD lifecycle** as other hosts; use `/gsd-*` commands explicitly when starting work
- No tmux supervisor, rely on `artifacts/current` continuity
- Hooks inject `AUTOPILOT_DRIVE` and `RFV_LOOP_CONTINUE` hints
- `ROUTER_RS_AUTOPILOT_DRIVE_HOOK=0` disables autopilot drive

## Universal Patterns

Regardless of host, GSD commands:

1. Read state from `artifacts/current/<task_id>/`
2. Write evidence to `EVIDENCE_INDEX.json`
3. Update goal/RFV state files
4. Use stdio JSON for router-rs operations

## Host detection

**Do not** infer capabilities from filenames alone. Closed-set host ids live in `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`. Per-host install scope and unsupported caps: `docs/hosts/*.md` and `host_projections.*.harness_capability_exceptions`.
