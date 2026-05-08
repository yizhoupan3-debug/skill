# Codex Hooks Projection

Codex hooks are **disabled for this repo by default**.

Project-local `.codex/hooks.json` intentionally contains **no active hooks**.

After running `scripts/install_codex_cli_hooks.sh`, `~/.codex/hooks.json` will include a codex-cli command hook that invokes the Rust `router-rs` review-subagent gate on `UserPromptSubmit`, `PostToolUse`, and `Stop`.

The Rust hook commands remain available for explicit one-off audits.

Use `scripts/install_codex_cli_hooks.sh` to install user-level hooks into `~/.codex/` for codex-cli only. The installer validates `python3` for config editing, enables `[features].codex_hooks = true` in `~/.codex/config.toml`, keeps existing hooks, and idempotently appends the review-subagent command hook without replacing unrelated handlers.

Hook state paths are host-specific and transient:

- Codex review-subagent gate state lives under `.codex/hook-state/`.
- Cursor review-subagent gate state lives under `.cursor/hook-state/`.

Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.

Regenerate with:

```sh
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml codex sync --repo-root "$PWD"
```
