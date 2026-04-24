# Claude Code Entry Proxy

This file exists because Claude Code discovers `CLAUDE.md`.

Keep startup lean. Do not add `@...` imports here.

Treat `.claude/**` as host-shell glue, not repository truth.
The recovery projection lives at `.codex/memory/CLAUDE_MEMORY.md` for `/refresh`
or manual resume, not default startup injection.

GPT bridge rule:

- Claude Code may be pointed at a GPT model through an Anthropic-compatible bridge, but GPT-default work should prefer the native Codex/OpenAI-compatible path to avoid protocol translation and extra startup context.

Generated-first maintenance rule:

- Update `scripts/router-rs/` first for Claude hook rules and host-entrypoint projections, then regenerate via `cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --sync-host-entrypoints-json --repo-root "$PWD"`.
- Host entrypoint sync runs directly through `router-rs`; do not reintroduce a Python wrapper in front of it.
- Treat those files as materialized outputs, not hand-authored truth.
- `.claude/agents/*.md` stays manually maintained unless a file says otherwise.
- Event-level lifecycle decisions live in `.claude/hooks/README.md`.
