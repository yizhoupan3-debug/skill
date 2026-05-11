# Codex Hooks Projection

Codex hooks are enabled for this repo and are managed by the Rust `router-rs` control plane.

<!-- managed_by: router-rs codex sync -->

**Policy snapshot:** the `codex_agent_policy` payload embeds the repository `AGENTS.md` at **router-rs compile time** (`include_str!`), not from disk on each hook run. `codex sync` preserves an existing root `AGENTS.md` from disk and only uses the embedded copy to bootstrap a missing file; rebuild before sync when you need generated Codex payloads to carry policy edits (see `AGENTS.md` → **权威分层** → **Codex：`AGENTS.md` 构建快照（策略 A）**).

Project-local `.codex/hooks.json` uses the official Codex lifecycle surface: `SessionStart`, `PreToolUse`, `UserPromptSubmit`, `PostToolUse`, and `Stop`.

Feature enablement uses `[features] hooks = true`; older public examples may still show `codex_hooks`, which this repository treats as a deprecated compatibility key and rewrites to `hooks`.

`SessionStart` injects workspace pointer plus a short continuity digest when `artifacts/current/` is populated, `UserPromptSubmit` injects only trigger-specific context, `PreToolUse` blocks direct edits to generated Codex surfaces, `PostToolUse` records best-effort subagent/tool telemetry and appends verification-like shell commands (for example `cargo test`) to `EVIDENCE_INDEX.json` when continuity is active (disable with `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`), and `Stop` writes an automatic in-progress continuity checkpoint under `artifacts/current/` unless `ROUTER_RS_CONTINUITY_STOP_CHECKPOINT=0`. Codex review/delegation policy is advisory in `AGENTS.md`; it is not enforced by a hard subagent gate. Durable cleanup should use explicit session-artifact or snapshot commands rather than an extra end-of-session hook.

Hook state is transient and lives under `.codex/hook-state/` in the current repository while the session is active.

Use `router-rs framework maint install-codex-user-hooks` when you want to install the same Codex hook projection into a user-level `~/.codex/hooks.json`. The installer keeps existing hooks and idempotently appends the managed command hook without replacing unrelated handlers.

Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.

Regenerate with:

```sh
cargo build --manifest-path scripts/router-rs/Cargo.toml
router-rs codex sync --repo-root "$PWD"
```

Steady-state documentation map (vs `docs/history/` archive): `docs/README.md`.
