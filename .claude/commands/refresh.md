---
description: Generate and copy the next-turn execution prompt with the Rust refresh command.
allowed-tools:
  - Bash(git rev-parse *)
  - Bash(./scripts/router-rs/target/release/router-rs *)
  - Bash(./scripts/router-rs/target/debug/router-rs *)
  - Bash(*scripts/router-rs/target/release/router-rs *)
  - Bash(*scripts/router-rs/target/debug/router-rs *)
  - Bash(cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- *)
  - Bash(cargo run --manifest-path *scripts/router-rs/Cargo.toml --release -- *)
---

Run:

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

If the release binary is missing, rerun the same command with:

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

If both resident binaries are missing, self-heal with:

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

Then reply with exactly:
`下一轮执行 prompt 已准备好，并且已经复制到剪贴板。`
