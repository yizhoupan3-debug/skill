#!/usr/bin/env bash
set -u

EVENT="${1:-}"
ROOT="${CURSOR_WORKSPACE_ROOT:-$PWD}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

if [[ -r "$ROOT/.cursor/router-rs-hook.env" ]]; then
  set -a
  # shellcheck source=/dev/null
  . "$ROOT/.cursor/router-rs-hook.env"
  set +a
fi

FAIL_MSG='router-rs binary unavailable for critical Cursor hook; fail-closed instead of silently bypassing gate enforcement'

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

emit_fail_closed_json() {
  local ev
  ev="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  case "$ev" in
    beforesubmitprompt)
      printf '%s\n' "{\"continue\":false,\"user_message\":\"$FAIL_MSG\"}"
      ;;
    subagentstart)
      printf '%s\n' "{\"permission\":\"deny\",\"user_message\":\"$FAIL_MSG\"}"
      ;;
    stop|posttooluse|subagentstop)
      printf '%s\n' '{"continue":false,"user_message":"'"$FAIL_MSG"'"}'
      ;;
    *)
      printf '%s\n' "{\"permission\":\"deny\",\"user_message\":\"$FAIL_MSG\"}"
      ;;
  esac
}

ROUTER_RS_BIN="${ROUTER_RS_BIN:-}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/skill-cargo-target}"
for candidate in \
  "$ROOT/scripts/router-rs/target/release/router-rs" \
  "$FW/scripts/router-rs/target/release/router-rs" \
  "$CARGO_TARGET_DIR/release/router-rs" \
  "$ROOT/scripts/router-rs/target/debug/router-rs" \
  "$FW/scripts/router-rs/target/debug/router-rs" \
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

if [ ! -x "$ROUTER_RS_BIN" ]; then
  if critical_event "$EVENT"; then
    emit_fail_closed_json "$EVENT"
    exit 2
  fi
  printf '%s\n' "[cursor-hook] router-rs binary unavailable for telemetry event $EVENT; fail-open" >&2
  exit 0
fi

exec "$ROUTER_RS_BIN" host cursor hook --event="$EVENT" --repo-root "$ROOT"
