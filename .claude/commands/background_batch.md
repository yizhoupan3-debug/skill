---
description: Use the repo's Rust-owned background batch control and state surfaces.
allowed-tools:
  - Bash(git rev-parse *)
  - Bash(./scripts/router-rs/target/release/router-rs *)
  - Bash(./scripts/router-rs/target/debug/router-rs *)
  - Bash(*scripts/router-rs/target/release/router-rs *)
  - Bash(*scripts/router-rs/target/debug/router-rs *)
  - Bash(cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- *)
  - Bash(cargo run --manifest-path *scripts/router-rs/Cargo.toml --release -- *)
---

Use `router-rs` directly for durable background batch control. Do not call the legacy Python helper.

Common Rust entrypoints:

- Plan a batch lane group with background control:
  `PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --background-control-json --background-control-input-json '<json>' --repo-root "$PROJECT_DIR"`
- Read one persisted group summary with background state:
  `PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --background-state-json --background-state-input-json '<json>' --repo-root "$PROJECT_DIR"`
- List persisted group summaries with background state:
  `PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --background-state-json --background-state-input-json '<json>' --repo-root "$PROJECT_DIR"`

Use operation `batch-plan` for control, and `parallel_group_summary` or `parallel_group_summaries` for state.
Always relay the JSON result and then summarize it briefly in plain Chinese.
