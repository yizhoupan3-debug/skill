#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

run_router_rs() {
  cargo run --quiet --manifest-path "$PROJECT_DIR/scripts/router-rs/Cargo.toml" -- "$@"
}

run_router_rs --claude-hook-audit-command config-change --repo-root "$PROJECT_DIR" >/dev/null
