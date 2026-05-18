#!/usr/bin/env bash
# router-rs-hook.sh — Independent hook launcher for Claude Code
# Extracted from .claude/settings.json inline bash. Fail-open: if router-rs
# is unavailable, the hook allows the operation with a warning instead of blocking.
#
# Shell compatibility: requires bash 4+ or zsh. Process substitution (< <(...))
# is used for jq parsing — ensure shell supports it (bash 4+/zsh, not dash).
set -euo pipefail

HOOK_EVENT="${1:?usage: $0 <event>}"
HOOK_PAYLOAD="$(cat)"
CLAUDE_PROJECT_ROOT="${CLAUDE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"

# Detect terminal capabilities for diagnostic context
_DETECT_TERM="${TERM:-unknown}"
_DETECT_SHELL="$(basename "$SHELL" 2>/dev/null || echo "unknown")"

# Source env file if present (for cached binary path, etc.)
if [[ -r "$CLAUDE_PROJECT_ROOT/.claude/router-rs-hook.env" ]]; then
  set -a
  . "$CLAUDE_PROJECT_ROOT/.claude/router-rs-hook.env"
  set +a
fi

# Fast path: use cached binary path from env (skip search)
if [[ -n "${ROUTER_RS_BIN_CACHED:-}" ]] && [ -x "$ROUTER_RS_BIN_CACHED" ]; then
  ROUTER_RS_BIN="$ROUTER_RS_BIN_CACHED"
else
  # Slow path: search for binary
  ROUTER_RS_BIN=""
  for candidate in \
    "$CLAUDE_PROJECT_ROOT/scripts/router-rs/target/release/router-rs" \
    "$CLAUDE_PROJECT_ROOT/scripts/router-rs/target/debug/router-rs" \
    "$CLAUDE_PROJECT_ROOT/target/release/router-rs" \
    "$CLAUDE_PROJECT_ROOT/target/debug/router-rs" \
    "$(command -v router-rs 2>/dev/null || true)"; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
      ROUTER_RS_BIN="$candidate"
      break
    fi
  done
fi

if [ ! -x "${ROUTER_RS_BIN:-}" ]; then
  # FAIL-OPEN: warn but allow
  printf '{"decision":"allow","reason":"router-rs unavailable, running without framework","suppressOutput":true,"hookSpecificOutput":{"hookEventName":"%s","permissionDecision":"allow","permissionDecisionReason":"fail-open: router-rs binary not found","term":"%s","shell":"%s"}}' "$HOOK_EVENT" "$_DETECT_TERM" "$_DETECT_SHELL"
  exit 0
fi

# Ensure hook-state directory exists for file locking
CLAUDE_HOOK_STATE_DIR="$CLAUDE_PROJECT_ROOT/.claude/hook-state"
mkdir -p "$CLAUDE_HOOK_STATE_DIR" 2>/dev/null

# Pass payload to router-rs hook handler.
# HOOK_PAYLOAD comes from $(cat) which captures valid UTF-8 text (no raw null bytes in JSON).
printf '%s' "$HOOK_PAYLOAD" | "$ROUTER_RS_BIN" claude hook --event="$HOOK_EVENT" --repo-root "$CLAUDE_PROJECT_ROOT"
