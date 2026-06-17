#!/usr/bin/env bash
# Cursor router-rs hook launcher (shim → hook.sh)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
exec "$SCRIPT_DIR/hook.sh" cursor "$@"
