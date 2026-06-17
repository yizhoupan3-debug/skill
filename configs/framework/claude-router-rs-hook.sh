#!/usr/bin/env bash
# Claude Code router-rs hook launcher (shim → hook.sh)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
exec "$SCRIPT_DIR/hook.sh" claude "$@"
