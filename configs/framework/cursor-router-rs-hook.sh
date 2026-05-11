#!/usr/bin/env bash
set -u

EVENT="${1:-}"
ROOT="${CURSOR_WORKSPACE_ROOT:-$PWD}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

critical_event() {
  case "$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')" in
    beforesubmitprompt|stop|posttooluse|subagentstart|subagentstop)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

ROUTER_RS_BIN="${ROUTER_RS_BIN:-}"
for candidate in \
  "$ROOT/scripts/router-rs/target/release/router-rs" \
  "$ROOT/scripts/router-rs/target/debug/router-rs" \
  "$FW/scripts/router-rs/target/release/router-rs" \
  "$FW/scripts/router-rs/target/debug/router-rs" \
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
  if critical_event "$EVENT"; then
    printf '%s\n' '{"permission":"deny","user_message":"router-rs binary unavailable for critical Cursor hook; fail-closed instead of silently bypassing gate enforcement"}'
    exit 1
  fi
  printf '%s\n' "[cursor-hook] router-rs binary unavailable for telemetry event $EVENT; fail-open" >&2
  exit 0
fi

exec "$ROUTER_RS_BIN" cursor hook --event="$EVENT" --repo-root "$ROOT"
