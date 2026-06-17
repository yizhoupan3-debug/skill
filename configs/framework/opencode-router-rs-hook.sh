#!/usr/bin/env bash
# OpenCode router-rs hook launcher (shim → hook.sh)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
exec "$SCRIPT_DIR/hook.sh" opencode "$@"
