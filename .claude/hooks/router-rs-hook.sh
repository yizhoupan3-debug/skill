#!/usr/bin/env bash
# Deprecated path: delegate to canonical launcher (fail-closed on critical events).
set -euo pipefail
ROOT="${CLAUDE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
exec "$ROOT/configs/framework/claude-router-rs-hook.sh" "$@"
