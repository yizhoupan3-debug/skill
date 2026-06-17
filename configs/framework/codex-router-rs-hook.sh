#!/usr/bin/env bash
# Codex router-rs hook launcher (shim → hook.sh)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
exec "$SCRIPT_DIR/hook.sh" codex "$@"
