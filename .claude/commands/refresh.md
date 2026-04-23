---
description: 使用 Rust refresh 命令生成并复制下一轮执行提示。
allowed-tools:
  - Bash(git rev-parse *)
  - Bash(./scripts/router-rs/target/release/router-rs *)
  - Bash(./scripts/router-rs/target/debug/router-rs *)
  - Bash(*scripts/router-rs/target/release/router-rs *)
  - Bash(*scripts/router-rs/target/debug/router-rs *)
  - Bash(cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- *)
  - Bash(cargo run --manifest-path *scripts/router-rs/Cargo.toml --release -- *)
---

运行：

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

如果 release 二进制不存在，用下面的命令重试：

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

如果两个常驻二进制都不存在，用下面的命令自修复：

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

然后严格回复：
`下一轮执行提示已准备好，并且已经复制到剪贴板。`
