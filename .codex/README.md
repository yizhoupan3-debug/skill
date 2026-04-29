# Codex Hooks Projection

Codex hooks are disabled for this repo.

Project-local `.codex/hooks.json` intentionally contains no active hooks.

The inactive hook scripts under `.codex/hooks/` remain available only as test fixtures or explicit audit helpers.

The Rust hook command remains available for explicit one-off audits.

Regenerate with:

```sh
./scripts/router-rs/run_router_rs.sh ./scripts/router-rs/Cargo.toml codex sync --repo-root "$PWD"
```
