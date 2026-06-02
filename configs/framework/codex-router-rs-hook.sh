#!/usr/bin/env bash
# Canonical Codex router-rs hook launcher (hooks.json merge must reference this path).
set -euo pipefail

HOOK_EVENT="${1:?usage: $0 <event>}"
HOOK_PAYLOAD="$(cat)"
ROOT="${CODEX_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

critical_event() {
  case "$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')" in
    sessionstart|pretooluse|userpromptsubmit|posttooluse|stop)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

# Fail-closed JSON when router-rs binary is unavailable.
# Codex CLI expects JSON on stdout for critical events.
emit_fail_closed_json() {
  local msg='router-rs binary unavailable for critical Codex hook; fail-closed instead of silently bypassing gate enforcement'
  printf '%s\n' "{\"decision\":\"block\",\"reason\":\"$msg\",\"suppressOutput\":true}"
}

# Resolve router-rs binary. Priority:
#   1. ROUTER_RS_BIN env (if set and executable)
#   2. ~/.local/bin/router-rs
#   3. command -v router-rs
#   4. Build-tree candidates (release/debug, ROOT/FW/CARGO_TARGET_DIR)
ROUTER_RS_BIN="${ROUTER_RS_BIN:-}"
if [ -z "$ROUTER_RS_BIN" ] && [ -x "${HOME:-}/.local/bin/router-rs" ]; then
  ROUTER_RS_BIN="${HOME}/.local/bin/router-rs"
fi
if [ -z "$ROUTER_RS_BIN" ]; then
  ROUTER_RS_BIN="$(command -v router-rs 2>/dev/null || true)"
fi
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/skill-cargo-target}"
for candidate in \
  "$ROOT/core/router-rs/target/release/router-rs" \
  "$FW/core/router-rs/target/release/router-rs" \
  "$ROOT/core/router-rs/target/debug/router-rs" \
  "$FW/core/router-rs/target/debug/router-rs" \
  "$CARGO_TARGET_DIR/release/router-rs" \
  "$CARGO_TARGET_DIR/debug/router-rs" \
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

if [ ! -x "${ROUTER_RS_BIN:-}" ]; then
  if critical_event "$HOOK_EVENT"; then
    emit_fail_closed_json
    exit 2
  fi
  printf '%s\n' "[codex-hook] router-rs binary unavailable for telemetry event $HOOK_EVENT; fail-open" >&2
  exit 0
fi

printf '%s' "$HOOK_PAYLOAD" | "$ROUTER_RS_BIN" host codex hook --event="$HOOK_EVENT" --repo-root "$ROOT"
