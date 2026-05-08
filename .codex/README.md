# Codex Hooks Projection

Codex hooks are enabled for this repo.

Project-local `.codex/hooks.json` contains the active hook handlers for this repo.

Hook scripts live under `.codex/hooks/`.

The Rust hook commands remain available for explicit one-off audits.

Use `codex hook contract-guard` as an opt-in continuity audit. It compares a caller-provided expected `contract_digest`, owner, task, goal, and evidence intent against the live Rust `framework contract-summary` payload, then fails closed on drift unless the caller sets an explicit contract update intent.

Regenerate with:

```sh
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml codex sync --repo-root "$PWD"
```
