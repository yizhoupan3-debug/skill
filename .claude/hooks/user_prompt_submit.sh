#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
python3 "$PROJECT_DIR/scripts/claude_hook_automation.py" user-prompt-submit --repo-root "$PROJECT_DIR"
