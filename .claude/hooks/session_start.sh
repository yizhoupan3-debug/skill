#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
ROUTER_RS_RUNNER="$PROJECT_DIR/scripts/router_rs_runner.py"

run_router_rs() {
  if [ ! -f "$ROUTER_RS_RUNNER" ]; then
    echo "Missing required router-rs runner: $ROUTER_RS_RUNNER" >&2
    exit 1
  fi
  python3 "$ROUTER_RS_RUNNER" "$@"
}

run_router_rs --claude-hook-command session-start --repo-root "$PROJECT_DIR" >/dev/null
