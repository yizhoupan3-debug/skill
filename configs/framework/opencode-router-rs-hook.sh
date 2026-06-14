#!/usr/bin/env bash
# Canonical OpenCode router-rs hook launcher.
# OpenCode hook plugins call this script; it delegates to `router-rs opencode hook`.
set -euo pipefail

HOOK_EVENT="${1:?usage: $0 <event>}"
HOOK_PAYLOAD="$(cat)"
ROOT="${OPENCODE_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

critical_event() {
  case "$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')" in
    tool.execute.before|tool.execute.after|session.idle|session.created)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

emit_fail_closed_json() {
  local msg='router-rs binary unavailable for critical OpenCode hook; fail-closed instead of silently bypassing gate enforcement'
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
  printf '%s\n' "[opencode-hook] router-rs binary unavailable for telemetry event $HOOK_EVENT; fail-open" >&2
  exit 0
fi

printf '%s' "$HOOK_PAYLOAD" | "$ROUTER_RS_BIN" host opencode hook --event="$HOOK_EVENT" --repo-root "$ROOT"

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
