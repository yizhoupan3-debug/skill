#!/usr/bin/env bash
# claude-hook-wrapper.sh — Fallback hook dispatch for router-rs
#
# Used only when router-rs binary is unavailable or as a fallback path.
# Delegates to hook-dispatch.sh for registry-driven host resolution.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ -x "${SCRIPT_DIR}/hook-dispatch.sh" ]]; then
  exec "${SCRIPT_DIR}/hook-dispatch.sh" "$@"
fi

# Absolute fallback — should not normally reach here
echo '{"decision":"allow","reason":"hook-dispatch.sh not found","suppressOutput":true}'
exit 0
