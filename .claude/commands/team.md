---
description: Enter the repo's Rust-owned team lane.
allowed-tools:
  - Bash(git rev-parse *)
  - Bash(./scripts/router-rs/target/release/router-rs *)
  - Bash(./scripts/router-rs/target/debug/router-rs *)
  - Bash(*scripts/router-rs/target/release/router-rs *)
  - Bash(*scripts/router-rs/target/debug/router-rs *)
  - Bash(cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- *)
  - Bash(cargo run --manifest-path *scripts/router-rs/Cargo.toml --release -- *)
---

Treat `/team` as a thin Rust-first alias.
This command now enters the repo through the resident Rust binary directly.

Run:

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-alias-json --framework-alias team --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

If the release binary is missing, rerun the same command with:

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-alias-json --framework-alias team --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

If both resident binaries are missing, self-heal with:

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-alias-json --framework-alias team --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"`

Use `alias.state_machine` and `alias.entry_contract` as the working contract for this turn.
This alias only enters through explicit entrypoints: `/team`, `$team`.
Implicit routing policy: `strong-orchestration-only`.
Prefer the Rust alias payload over opening long docs or restating OMC background.
Only open `skills/team/SKILL.md` if the alias payload is missing something you still need.
Keep execution inside the repo's native Rust/continuity lane.
    