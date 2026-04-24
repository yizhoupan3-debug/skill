# Claude Hooks Directory

Claude Code project hooks live here.

Generated-first maintenance:

- Update `scripts/router-rs/` first for Claude hook rules and host-entrypoint projections, then regenerate via `cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --sync-host-entrypoints-json --repo-root "$PWD"`.
- Host entrypoint sync runs directly through `router-rs`; do not put a Python wrapper back in front of it.
- Treat `.claude/settings.json` and this README as materialized outputs.
- Manual Claude host guidance belongs in `.claude/agents/*.md` unless noted.
- Codex uses `.codex/hooks.json` for a separate silent preflight guardrail
  layer on `Edit` / `MultiEdit` / `Write` / `Bash`; do not mirror Claude
  prompt hooks onto Codex.

Active hooks:

| Event | Runner | Purpose |
| --- | --- | --- |
| `UserPromptSubmit` | `router-rs --claude-host-hook-command user-prompt-submit` | Inject the repo-local shared memory and continuity truth on every real prompt, plus narrow execution-time hints when the current prompt clearly needs them. |
| `PreToolUse` | `router-rs --claude-host-hook-command pre-tool-use-quality` | Add a short path-aware implementation reminder before editing runtime, host-entrypoint sync, hook, or contract-test code that is already inside the narrow quality lane, and capture a lightweight pre-edit baseline for later delta-aware review. |
| `PreToolUse` | `router-rs --claude-host-hook-command pre-tool-use` | Deny direct edits to generated host outputs and the imported Claude projection before `Edit`, `MultiEdit`, `Write`, or targeted `Bash` writes run. |
| `PostToolUse` | `router-rs --claude-host-hook-command post-tool-audit` | Run a background implementation audit after real code edits and inspect the new delta first, so only newly introduced compatibility-heavy or wasteful patterns get fed back. |
| `PostToolUseFailure` | `router-rs --claude-host-hook-command post-tool-failure-audit` | When edits to generated host outputs fail, remind Claude to regenerate from Rust instead of retrying direct writes. |
| `PostToolBatch` | documented surface, not installed | Available for batch-level follow-up after parallel tool calls; avoid using it for single-tool checks already covered by `PostToolUse`. |
| `UserPromptExpansion` | documented surface, not installed | Available for slash/command expansion validation; repo slash command bodies stay static unless a concrete risk appears. |
| `SessionEnd` | `router-rs --claude-host-hook-command session-end` | Consolidate project-local memory, refresh the Claude projection, and repair stale terminal resume state when needed. |
| `ConfigChange` | `router-rs --claude-host-hook-command config-change` | Warn when generated Claude host files were edited directly instead of regenerated from source. |
| `StopFailure` | `router-rs --claude-host-hook-command stop-failure` | Emit a host-private hint for selected Claude stop failures without mutating shared continuity. |

Everything else stays intentionally uninstalled here so startup and tool turns remain lean.
`UserPromptSubmit` is installed here on purpose: this repo keeps memory truth under
`./.codex/memory/` plus continuity artifacts, so prompt-time injection is the
lowest-friction way to keep Claude aligned with repo-local state instead of stale
host-global recall.
Reply tone, "讲人话" rules, closeout shape, and broad implementation philosophy
still live in `AGENT.md`, not in hooks.
Static behavior rules belong in `AGENT.md` or `CLAUDE.md`; these hooks exist
for deterministic guardrails, lightweight execution-time context, and lifecycle
maintenance.

Project hook principles:

- Keep project hooks for repo-specific invariants only.
- Keep hooks fast, especially `PreToolUse`, because it runs inside the agent
  loop.
- Use `matcher` first and `if` to narrow further, so hook handlers do not spawn
  on unrelated tool calls and normal edits stay fast.
- Automation hooks should be additive and short: inject narrow repo context or
  launch cheap follow-up work, not essay-length prompt rewrites.
- Prefer `command` hooks for deterministic repo guardrails. `http`,
  `mcp_tool`, `prompt`, and `agent` hook handlers are supported by Claude Code,
  but should only be introduced for a real repo invariant that cannot be handled
  locally and cheaply.
- Use `asyncRewake` only for background checks that may discover a real problem
  after Claude has moved on; ordinary async audits should stay quiet unless they
  have actionable feedback.
- Keep durable implementation philosophy in `AGENT.md`; hook-time nudges should
  stay concrete, local to the current path, and local to the current delta.
- Prefer async `PostToolUse` for cheap quality follow-up that should not block
  the main turn.
- Put personal notifications and local approval shortcuts in `~/.claude/settings.json`
  or `.claude/settings.local.json`, not in committed project settings.
- Use `"$CLAUDE_PROJECT_DIR"`-anchored paths in hook commands and treat hook
  stdin JSON as untrusted input.
- Prefer `PreToolUse` deny over `PostToolUse` cleanup for protected files.
- Keep the generated-surface guard intentionally narrow so normal edits stay fast.
- Keep `SessionEnd` as the only writer hook here; the others are guards or alerts.
- When debugging config drift, verify the installed hook set from Claude
  Code's `/hooks` menu before changing generated files.

Validation commands:

- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/claude_hooks.rs"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command pre-tool-use-quality --repo-root "$PWD"`
  Expected: stdout returns a JSON `permissionDecision: allow` payload with `additionalContext`.
- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/claude_hooks.rs"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command post-tool-audit --repo-root "$PWD"`
  Expected: stdout is empty for clean edits, or JSON with top-level `additionalContext` when the new delta still looks patchy, compatibility-heavy, or wasteful.
- `printf '{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/host_integration.rs"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command pre-tool-use-quality --repo-root "$PWD"`
  Expected: stdout returns a JSON `permissionDecision: allow` payload with Rust/runtime-oriented `additionalContext`.
- `cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --sync-host-entrypoints-json --repo-root "$PWD"`
  Expected: regenerate `AGENT.md`, `AGENTS.md`, `CLAUDE.md`, `.claude/settings.json`, `.codex/hooks.json`, and matching worktree projections directly from Rust.
- `printf '{"tool_name":"MultiEdit","tool_input":{"file_path":".claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command pre-tool-use --repo-root "$PWD"`
  Expected: stdout returns a JSON `permissionDecision: deny` payload.
- `printf '{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command pre-tool-use --repo-root "$PWD"`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for the targeted write.
- `printf '{"tool_name":"Bash","tool_input":{"command":"printf x > .claude/settings.json"}}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command pre-tool-use --repo-root "$PWD"`
  Expected: stdout returns a JSON `permissionDecision: deny` payload for shell redirection into a protected generated file.
- `printf '{"hook_event_name":"UserPromptSubmit","prompt":"继续修复这个仓库的共享记忆和 runtime"}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command user-prompt-submit --repo-root "$PWD"`
  Expected: stdout returns JSON with `hookSpecificOutput.additionalContext` containing repo-local memory and continuity reminders.
- `CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command session-end --repo-root "$PWD"`
  Expected: project-local memory bundle refresh plus projection refresh; may repair stale terminal resume state in `.supervisor_state.json`.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command config-change --repo-root "$PWD"`
  Expected: audit-only stderr guidance about regenerating generated Claude host files; exit 0.
- `printf '{"hook_event_name":"StopFailure","error":"server_error"}
' | CLAUDE_PROJECT_DIR="$PWD" ./scripts/router-rs/target/debug/router-rs --claude-host-hook-command stop-failure --repo-root "$PWD"`
  Expected: host-private failure classification hint on stderr; exit 0.
- `./scripts/router-rs/target/debug/router-rs --claude-host-hook-command session-end --repo-root "$PWD" --claude-hook-max-lines 4`
  Expected: silent lifecycle entrypoint for Claude host hooks; same consolidation and projection contract.
- `printf '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}
' | ./scripts/router-rs/target/debug/router-rs --claude-hook-audit-command config-change --repo-root "$PWD"`
  Expected: JSON on stdout plus audit-only stderr guidance; exit 0.
- In Claude Code, run `/hooks`
  Expected: the project shows `PreToolUse`, `PostToolUse`, `UserPromptSubmit`,
  `SessionEnd`, `ConfigChange`, and `StopFailure` from `.claude/settings.json`.

Shared routing policy still comes from `../../AGENT.md`.
