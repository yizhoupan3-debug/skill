#!/usr/bin/env bash
# Canonical Claude Code router-rs hook launcher (settings merge must reference this path).
set -euo pipefail

HOOK_EVENT="${1:?usage: $0 <event>}"
HOOK_PAYLOAD="$(cat)"
ROOT="${CLAUDE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

critical_event() {
  case "$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')" in
    pretooluse|userpromptsubmit|posttooluse|stop)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

ROUTER_RS_BIN="${ROUTER_RS_BIN:-}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/skill-cargo-target}"
for candidate in \
  "$CARGO_TARGET_DIR/release/router-rs" \
  "$CARGO_TARGET_DIR/debug/router-rs" \
  "$ROOT/core/router-rs/target/release/router-rs" \
  "$FW/core/router-rs/target/release/router-rs" \
  "$ROOT/core/router-rs/target/debug/router-rs" \
  "$FW/core/router-rs/target/debug/router-rs" \
  "$ROOT/target/release/router-rs" \
  "$ROOT/target/debug/router-rs" \
  "$FW/target/release/router-rs" \
  "$FW/target/debug/router-rs"
do
  if [ -z "$ROUTER_RS_BIN" ] && [ -x "$candidate" ] && "$candidate" host claude hook --help >/dev/null 2>&1; then
    ROUTER_RS_BIN="$candidate"
  fi
done

if [ -z "$ROUTER_RS_BIN" ]; then
  ROUTER_RS_BIN="$(command -v router-rs 2>/dev/null || true)"
  if [ -n "$ROUTER_RS_BIN" ] && ! "$ROUTER_RS_BIN" host claude hook --help >/dev/null 2>&1; then
    ROUTER_RS_BIN=""
  fi
fi

if [ ! -x "${ROUTER_RS_BIN:-}" ]; then
  if critical_event "$HOOK_EVENT" && [ "${ROUTER_RS_HOOK_FAIL_OPEN:-1}" != "1" ]; then
    printf '%s\n' '{"decision":"block","reason":"router-rs binary unavailable for Claude hook (set ROUTER_RS_HOOK_FAIL_OPEN=1 to block)","suppressOutput":true}'
    exit 1
  fi
  printf '%s\n' '{"decision":"allow","reason":"router-rs unavailable, running without framework","suppressOutput":true}'
  exit 0
fi

printf '%s' "$HOOK_PAYLOAD" | "$ROUTER_RS_BIN" host claude hook --event="$HOOK_EVENT" --repo-root "$ROOT"

# Health monitor integration for SubagentStart/SubagentStop
HEALTH_MONITOR="$FW/configs/framework/agent-health-monitor.sh"
if [ -x "$HEALTH_MONITOR" ]; then
  case "$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')" in
    subagentstart)
      AGENT_NAME=$(printf '%s' "$HOOK_PAYLOAD" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_name','unknown'))" 2>/dev/null || echo "unknown")
      AGENT_ID=$(printf '%s' "$HOOK_PAYLOAD" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('name','') or d.get('tool_input',{}).get('description','') or 'agent-'+str(id(d)))" 2>/dev/null || echo "agent-$$")
      "$HEALTH_MONITOR" start "$AGENT_NAME" "$AGENT_ID" >/dev/null 2>&1 || true
      ;;
    subagentstop)
      AGENT_ID=$(printf '%s' "$HOOK_PAYLOAD" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('name','') or d.get('tool_input',{}).get('description','') or 'agent-'+str(id(d)))" 2>/dev/null || echo "agent-$$")
      "$HEALTH_MONITOR" stop "$AGENT_ID" >/dev/null 2>&1 || true
      ;;
  esac
fi
