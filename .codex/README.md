# Codex Hooks Projection

Codex hooks are enabled for this repo and are managed by the Rust `router-rs` control plane.

Project-local `.codex/hooks.json` uses the official Codex lifecycle surface: `SessionStart`, `PreToolUse`, `UserPromptSubmit`, `PostToolUse`, and `Stop`.

`SessionStart` injects a compact workspace pointer, `UserPromptSubmit` injects only trigger-specific context, `PreToolUse` blocks direct edits to generated Codex surfaces, `PostToolUse` records lightweight evidence, and `Stop` enforces closeout gates. Durable cleanup should use explicit refresh commands rather than an extra end-of-session hook.

Hook state is transient and lives under `.codex/hook-state/` in the current repository while the session is active.

Use `scripts/install_codex_cli_hooks.sh` only when you want to install the same Codex hook projection into a user-level `~/.codex/hooks.json`. The installer keeps existing hooks and idempotently appends the managed command hook without replacing unrelated handlers.

Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.

Regenerate with:

```sh
router-rs codex sync --repo-root "$PWD"
```
