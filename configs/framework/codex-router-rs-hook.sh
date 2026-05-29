#!/usr/bin/env bash
ROOT="${CLAUDE_PROJECT_ROOT:-$PWD}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"
if [[ -r "$ROOT/.codex/router-rs-hook.env" ]]; then
    set -a
    . "$ROOT/.codex/router-rs-hook.env"
    set +a
elif [[ -r "$ROOT/.claude/router-rs-hook.env" ]]; then
    set -a
    . "$ROOT/.claude/router-rs-hook.env"
    set +a
fi
# Codex lifecycle hook launcher — resolve router-rs and fail-closed when missing.
set -u

EVENT="${1:-}"
ROOT="${CODEX_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

FAIL_JSON='{"decision":"block","message":"router-rs binary unavailable for Codex hook","reason":"router-rs binary unavailable; fail-closed instead of silently bypassing critical hook enforcement"}'

ROUTER_RS_BIN="${ROUTER_RS_BIN:-}"
for candidate in \
  "$ROOT/core/router-rs/target/release/router-rs" \
  "$ROOT/core/router-rs/target/debug/router-rs" \
  "$FW/core/router-rs/target/release/router-rs" \
  "$FW/core/router-rs/target/debug/router-rs" \
  "$ROOT/target/release/router-rs" \
  "$ROOT/target/debug/router-rs" \
  "$FW/target/release/router-rs" \
  "$FW/target/debug/router-rs"
do
  if [ -z "$ROUTER_RS_BIN" ] && [ -x "$candidate" ]; then
    ROUTER_RS_BIN="$candidate"
  fi
done

if [ -z "$ROUTER_RS_BIN" ]; then
  ROUTER_RS_BIN="$(command -v router-rs 2>/dev/null || true)"
fi

if [ ! -x "$ROUTER_RS_BIN" ]; then
  printf '%s\n' "$FAIL_JSON"
  exit 1
fi

exec "$ROUTER_RS_BIN" host codex hook --event="$EVENT" --repo-root "$ROOT"
