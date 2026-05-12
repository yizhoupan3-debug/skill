# Codex Hooks Projection

Codex hooks are enabled for this repo and are managed by the Rust `router-rs` control plane.

<!-- managed_by: router-rs codex sync -->

**Policy snapshot:** the `codex_agent_policy` payload embeds the repository `AGENTS.md` at **router-rs compile time** (`include_str!`), not from disk on each hook run. `codex sync` preserves an existing root `AGENTS.md` from disk and only uses the embedded copy to bootstrap a missing file; rebuild before sync when you need generated Codex payloads to carry policy edits (see `AGENTS.md` → **权威分层** → **Codex：`AGENTS.md` 构建快照（策略 A）**).

Project-local `.codex/hooks.json` uses the official Codex lifecycle surface: `SessionStart`, `PreToolUse`, `UserPromptSubmit`, `PostToolUse`, and `Stop`.

Feature enablement uses `[features] hooks = true`; older public examples may still show `codex_hooks`, which this repository treats as a deprecated compatibility key and rewrites to `hooks`.

`SessionStart` injects workspace pointer plus a short continuity digest when `artifacts/current/` is populated, `UserPromptSubmit` injects only trigger-specific context, `PreToolUse` blocks direct edits to generated Codex surfaces, `PostToolUse` records subagent/tool telemetry and appends verification-like shell commands (for example `cargo test`) to `EVIDENCE_INDEX.json` when continuity is active (disable with `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`), and `Stop` writes an automatic in-progress continuity checkpoint under `artifacts/current/` unless `ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=0`. Broad/deep review prompts also require an independent read-only reviewer subagent (`fork_context=false`) before Stop can close. Durable cleanup should use explicit session-artifact or snapshot commands rather than an extra end-of-session hook.

Hook state is transient and lives under `.codex/hook-state/` in the current repository while the session is active. Stable keys require `session_id` / `conversation_id` / `thread_id` in hook payloads (snake_case **or** camelCase, e.g. `sessionId`) or `CODEX_SESSION_ID` / `CODEX_CONVERSATION_ID` in the environment; otherwise hook-state may not persist across invocations (router-rs logs a one-time stderr warning per process).

Optional **`ROUTER_RS_CODEX_REQUIRE_STABLE_SESSION_KEY`**: when set to `1`/`true`/`yes`/`on`, `UserPromptSubmit`, `PostToolUse`, and `Stop` **block** if no stable identifier is present (`SessionStart` is unaffected).

Generated hook commands resolve `router-rs` in order: **`ROUTER_RS_BIN`** when set to an executable path, then `scripts/router-rs/target/{release,debug}/router-rs`, then repo `target/{release,debug}/router-rs`, finally `command -v router-rs` (last resort — prefer pinning `ROUTER_RS_BIN` or building into the repo). If the binary is missing, **all** lifecycle hooks fail closed with a JSON `decision:block` line.

Merged `additionalContext` for SessionStart/UserPromptSubmit is capped by UTF-8 **byte** length (not Unicode character count). Tune with `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` or legacy `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` (same semantics; clamped 256–8192; default 640 bytes).

Successful Codex hook processes always print one JSON object line on stdout (including `{}` when there is no hook-specific output).

Stop hook blocks when `.codex/hook-state` cannot be read or parsed (non-recoverable JSON/IO): fix permissions or delete corrupted state files before continuing.

Use `cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint install-codex-user-hooks` when you want to install the same Codex hook projection into a user-level `~/.codex/hooks.json`. The installer keeps existing hooks and idempotently appends the managed command hook without replacing unrelated handlers.

Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.

Regenerate with:

```sh
cargo run --manifest-path scripts/router-rs/Cargo.toml -- codex sync --repo-root "$PWD"
```

Steady-state documentation map (vs `docs/history/` archive): `docs/README.md`.
