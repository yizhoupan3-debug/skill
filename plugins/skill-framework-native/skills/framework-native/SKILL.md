---
name: framework-native
description: Use router-rs and runtime artifacts when the task is about this repository's skill framework, routing, memory, or Codex-native integration surface.
---

When the task is about `/Users/joe/Documents/skill`, prefer the local Rust framework surfaces before broad repo scraping.

Workflow:
1. Run `scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml --framework-runtime-snapshot-json --repo-root /Users/joe/Documents/skill` to see the active supervisor and artifact state.
2. Run `scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml --route-json --repo-root /Users/joe/Documents/skill --query "<query>"` when routing, owner boundaries, triggers, or framework owners matter.
3. Run `scripts/router-rs/run_router_rs.sh scripts/router-rs/Cargo.toml --framework-memory-recall-json --repo-root /Users/joe/Documents/skill --query "<query>"` when cross-session memory matters.

Do not use this skill for unrelated repositories or generic MCP work outside this workspace.
