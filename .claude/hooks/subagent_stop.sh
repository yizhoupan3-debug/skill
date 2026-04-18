#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/claude_memory_bridge.py" subagent-stop   --repo-root "$PROJECT_DIR" >/dev/null
