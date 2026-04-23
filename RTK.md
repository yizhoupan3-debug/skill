# RTK

RTK is the open-source project [`rtk-ai/rtk`](https://github.com/rtk-ai/rtk): a Rust CLI proxy that compresses noisy terminal output before it reaches the agent context window.

In the upstream README, RTK positions itself as a tool that reduces LLM token consumption by roughly 60-90% on common developer commands, and the official Codex install path is:

```bash
rtk init -g --codex
```

Upstream also states that the Codex integration method is `AGENTS.md + RTK.md instructions`, unlike Claude/Gemini hook integrations that can transparently rewrite shell commands.

## What this means in this repo

For this repository, treat RTK as **instruction-driven**, not as something you should assume is auto-rewriting every shell command.

Practical rule:

- If a shell command is likely to emit large, repetitive, or low-signal output, prefer the `rtk` form explicitly.
- If raw output fidelity matters more than token reduction, use the original command.

## Prefer RTK for

- directory and file discovery
- text search with many matches
- git status / diff / log style output
- test, lint, and build output
- repository maintenance commands that are already known to be noisy

Typical examples:

```bash
rtk ls
rtk find skills -name 'SKILL.md'
rtk grep "RTK\\.md|rtk" .
rtk git status
rtk git diff --stat
rtk cargo test
rtk npm test
```

When a direct RTK subcommand is not available or not needed, wrap the original command explicitly:

```bash
rtk cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --sync-host-entrypoints-json --repo-root "$PWD"
rtk cargo test --manifest-path ./scripts/router-rs/Cargo.toml
rtk npm test --prefix tools/browser-mcp
```

## Do not prefer RTK for

- commands where the user explicitly wants the full raw output
- machine-consumed output such as strict JSON parsing pipelines
- interactive TUI / full-screen programs
- tiny outputs where compression adds no value
- debugging cases where RTK summarization may hide the exact failing line and the raw output is required

## Repo-local configuration

This repository already contains a project-scoped RTK filter file at [`/.rtk/filters.toml`](/Users/joe/Documents/skill/.rtk/filters.toml).

Current local filters are targeted and narrow:

- `cargo test`
- `npm test`
- `git diff`

Do not assume every repo script has a custom RTK filter. If a command is uncommon, RTK may still help, but it may only provide generic compaction.

## Verification on this machine

Verified locally on April 19, 2026:

- `rtk` binary path: `/opt/homebrew/bin/rtk`
- version: `rtk 0.33.0-rc.54`
- `rtk gain` is working and reports accumulated savings on this machine

## Working rule for Codex in this repo

When you are about to run a shell command with potentially high output volume, ask:

1. Does the command mainly produce context noise?
2. Is exact raw output unnecessary for the immediate task?

If both are true, use `rtk ...`.

If not, use the raw command.

## References

- [`rtk-ai/rtk` GitHub repository](https://github.com/rtk-ai/rtk)
- [RTK official site](https://www.rtk-ai.app/)
