---
description: 使用 Rust refresh 命令继续当前活跃任务，并复制下一轮执行提示。
allowed-tools:
  - Bash(git rev-parse *)
  - Bash(./scripts/router-rs/run_router_rs.sh *)
  - Bash(*scripts/router-rs/run_router_rs.sh *)
---

把 `/refresh` 当作当前仓库唯一显式的 continue / next 入口。
它会读取现有 continuity 真源，为当前活跃任务生成下一轮执行提示。

运行：

`PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"; "$PROJECT_DIR"/scripts/router-rs/run_router_rs.sh "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"`

然后严格回复：
`下一轮执行提示已准备好，并且已经复制到剪贴板。`
