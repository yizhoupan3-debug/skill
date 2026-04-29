# Codex Hooks Projection

Codex hooks are enabled for this repo.

Project-local hooks live in `.codex/hooks.json` and `.codex/hooks/`.

The active review gate requires broad/deep review requests to either spawn independent reviewer subagents or record a clear reject reason before finalizing.

The Rust hook command remains available for explicit one-off audits.

Regenerate with:

```sh
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml codex sync --repo-root "$PWD"
```
