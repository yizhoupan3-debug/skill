#\!/usr/bin/env bash
# AntigravityCLI lifecycle hook launcher -- resolve router-rs and fail-closed when missing.
set -euo pipefail

ANTIGRAVITY_CLI_PROJECT_ROOT="${ANTIGRAVITY_CLI_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
FW="${SKILL_FRAMEWORK_ROOT:-$ANTIGRAVITY_CLI_PROJECT_ROOT}"

FAIL_JSON='{"decision":"block","message":"router-rs binary unavailable for Antigravity CLI hook","reason":"router-rs binary unavailable; fail-closed instead of silently bypassing critical hook enforcement"}'

ROUTER_RS_BIN="${ROUTER_RS_BIN:-}"
for candidate in \
  "$ANTIGRAVITY_CLI_PROJECT_ROOT/core/router-rs/target/release/router-rs" \
  "$ANTIGRAVITY_CLI_PROJECT_ROOT/core/router-rs/target/debug/router-rs" \
  "$FW/core/router-rs/target/release/router-rs" \
  "$FW/core/router-rs/target/debug/router-rs" \
  "$ANTIGRAVITY_CLI_PROJECT_ROOT/target/release/router-rs" \
  "$ANTIGRAVITY_CLI_PROJECT_ROOT/target/debug/router-rs" \
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

if [ \! -x "${ROUTER_RS_BIN:-}" ]; then
  printf '%s\n' "$FAIL_JSON"
  exit 1
fi

exec "$ROUTER_RS_BIN" host antigravity-cli hook lifecycle-context --repo-root "$ANTIGRAVITY_CLI_PROJECT_ROOT"
