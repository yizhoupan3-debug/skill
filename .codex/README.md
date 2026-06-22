# Codex Hooks Projection

Codex hooks are enabled for this repo and are managed by the Rust `router-rs` control plane.

<!-- managed_by: router-rs codex sync -->

**Policy snapshot:** the `codex_agent_policy` payload embeds repository `AGENTS.md` at **router-rs compile time** (`include_str!`), not from disk on each hook run. `codex sync` / `framework sync-entrypoints` materializes **`.codex/README.md`** (see `.codex/host_entrypoints_sync_manifest.json`). Rebuild before sync when hook payloads must carry policy edits.

Project-local `.codex/hooks.json` uses the official Codex lifecycle surface: `SessionStart`, `PreToolUse`, `UserPromptSubmit`, `PostToolUse`, and `Stop`.

Feature enablement uses `[features] hooks = true`; older public examples may still show `codex_hooks`, which this repository treats as a deprecated compatibility key and rewrites to `hooks`.

`SessionStart` injects a lightweight workspace pointer (`Repo:` and optional `source`) when operator inject is enabled; it does **not** inject a continuity digest or hook-driven `GOAL_CONTINUE`. `UserPromptSubmit` injects only trigger-specific context. `PreToolUse` blocks direct edits to generated Codex surfaces. `PostToolUse` records subagent/tool telemetry and, when opted in (`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=1`, default off), may append verification-like shell commands (for example `cargo test`) to `EVIDENCE_INDEX.json` when continuity is active. `Stop` enforces closeout; `CODEX_REVIEW_GATE` is **advisory-only** (no `decision: block` on review gate — see `docs/adr/003-runtime-core-split.md` §0.1). Clear gate (Claude canonical): PostTool countable deep-lane evidence → `independent_reviewer_seen`, or bounded `rg_clear` / reject override tokens; Stop may inject a one-line nudge until satisfied. Set **`ROUTER_RS_CODEX_REVIEW_GATE_DISABLE=1`** to suppress advisory nudge (unset keeps enabled). `interactive` lifecycle profile suppresses review Stop nudge and spawn-first. It does **not** write an automatic continuity checkpoint (`ROUTER_RS_CONTINUITY_STOP_CHECKPOINT` is a no-op). Resume work via `/implementx`, `framework_goal_drive` stdio, and manual boards under `artifacts/current/<task_id>/`. Durable cleanup should use explicit session-artifact or snapshot commands rather than an extra end-of-session hook.

Hook state is transient and lives under `.codex/hook-state/` in the current repository while the session is active. Stable keys require `session_id` / `conversation_id` / `thread_id` in hook payloads (snake_case **or** camelCase, e.g. `sessionId`) or `CODEX_SESSION_ID` / `CODEX_CONVERSATION_ID` in the environment; otherwise hook-state may not persist across invocations (router-rs logs a one-time stderr warning per process).

**`ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY`** defaults **on** (`unset` = require stable keys). Set `0`/`false`/`off`/`no` for legacy payloads without `session_id` / env fallbacks (`SessionStart` is unaffected). Without a stable id and with strict mode off, hook-state uses a deterministic fallback keyed by **repo + cwd** (optional `ROUTER_RS_CODEX_HOOK_STATE_SALT`), not a single global file per machine.

**`ROUTER_RS_CODEX_REVIEW_FORK_CONTEXT_MISSING_INFER_FALSE`** (default on): deep lane + omitted `fork_context` counts as independent reviewer evidence on PostTool. Set `0`/`false`/`off`/`no` to require explicit JSON `fork_context: false`.

Generated hook commands resolve `router-rs` in order: **`ROUTER_RS_BIN`** when set to an executable path, then `core/router-rs/target/{release,debug}/router-rs`, then repo `target/{release,debug}/router-rs`, finally `command -v router-rs` (last resort — prefer pinning `ROUTER_RS_BIN` or building into the repo). If the binary is missing, **all** lifecycle hooks fail closed with a JSON `decision:block` line.

Merged `additionalContext` for SessionStart/UserPromptSubmit is capped by UTF-8 **byte** length (not Unicode character count). Tune with `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` or legacy `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` (same semantics; clamped 256–8192; default 640 bytes).

Successful Codex hook processes always print one JSON object line on stdout (including `{}` when there is no hook-specific output).

Stop hook blocks when `.codex/hook-state` cannot be read or parsed (non-recoverable JSON/IO): fix permissions or delete corrupted state files before continuing.

Use `cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint install-codex-user-hooks` when you want to install the same Codex hook projection into a user-level `~/.codex/hooks.json`. The installer keeps existing hooks and idempotently appends the managed command hook without replacing unrelated handlers.

Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.

Regenerate with:

```sh
cargo run --manifest-path core/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"
```

Steady-state documentation map: `docs/README.md`.
