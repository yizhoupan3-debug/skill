#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

python3 "$PROJECT_DIR/scripts/session_lifecycle_hook.py" subagent-stop   --repo-root "$PROJECT_DIR" >/dev/null
