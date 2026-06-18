#!/usr/bin/env bash
# Unified router-rs hook launcher.
# Usage: hook.sh <host_id> <event>
#   stdin: payload JSON
#   host_id: claude | codex | cursor | opencode
set -euo pipefail

HOST_ID="${1:?usage: hook.sh <host_id> <event>}"
HOOK_EVENT="${2:?usage: hook.sh <host_id> <event>}"
HOOK_PAYLOAD="$(cat)"

# ── Host-specific configuration ──────────────────────────────────
case "$HOST_ID" in
  claude)
    ROOT_ENV_VAR="CLAUDE_PROJECT_ROOT"
    ROOT_FALLBACK='git rev-parse --show-toplevel 2>/dev/null || pwd'
    CRITICAL_EVENTS="pretooluse|userpromptsubmit|posttooluse|stop"
    DELEGATE_CMD="host claude hook"
    FAIL_MSG='router-rs binary unavailable for Claude hook'
    FAIL_JSON='{"decision":"block","reason":"FAIL_MSG","suppressOutput":true}'
    ALLOW_JSON='{"decision":"allow","reason":"router-rs unavailable, running without framework","suppressOutput":true}'
    ;;
  codex)
    ROOT_ENV_VAR="CODEX_PROJECT_ROOT"
    ROOT_FALLBACK='git rev-parse --show-toplevel 2>/dev/null || pwd'
    CRITICAL_EVENTS="sessionstart|pretooluse|userpromptsubmit|posttooluse|stop"
    DELEGATE_CMD="host codex hook"
    FAIL_MSG='router-rs binary unavailable for critical Codex hook; fail-closed instead of silently bypassing gate enforcement'
    FAIL_JSON='{"decision":"block","reason":"FAIL_MSG","suppressOutput":true}'
    ;;
  cursor)
    ROOT_ENV_VAR="CURSOR_WORKSPACE_ROOT"
    ROOT_FALLBACK='echo "$PWD"'
    CRITICAL_EVENTS="beforesubmitprompt|stop|posttooluse|subagentstart|subagentstop"
    DELEGATE_CMD="host cursor hook"
    FAIL_MSG='router-rs binary unavailable for critical Cursor hook; fail-closed instead of silently bypassing gate enforcement'
    CURSOR_FORMAT=1
    ;;
  opencode)
    ROOT_ENV_VAR="OPENCODE_PROJECT_ROOT"
    ROOT_FALLBACK='git rev-parse --show-toplevel 2>/dev/null || pwd'
    CRITICAL_EVENTS="tool.execute.before|tool.execute.after|session.idle|session.created"
    DELEGATE_CMD="host opencode hook"
    FAIL_MSG='router-rs binary unavailable for critical OpenCode hook; fail-closed instead of silently bypassing gate enforcement'
    FAIL_JSON='{"decision":"block","reason":"FAIL_MSG","suppressOutput":true}'
    ;;
  *)
    echo "Unknown host_id: $HOST_ID" >&2
    exit 1
    ;;
esac

# ── Source host-specific env if exists ───────────────────────────
ROOT="${!ROOT_ENV_VAR:-$(eval "$ROOT_FALLBACK")}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

if [ "$HOST_ID" = "cursor" ] && [ -r "$ROOT/.cursor/router-rs-hook.env" ]; then
  set -a
  # shellcheck source=/dev/null
  . "$ROOT/.cursor/router-rs-hook.env"
  set +a
fi

# ── Critical event check ─────────────────────────────────────────
critical_event() {
  local ev; ev="$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')"
  [[ "$ev" =~ ^($CRITICAL_EVENTS)$ ]]
}

# ── Fail-closed JSON emitter ─────────────────────────────────────
emit_fail_closed() {
  if [ "${CURSOR_FORMAT:-}" = "1" ]; then
    local ev; ev="$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')"
    case "$ev" in
      beforesubmitprompt)
        printf '%s\n' "{\"continue\":false,\"followup_message\":\"$FAIL_MSG\",\"user_message\":\"$FAIL_MSG\"}" ;;
      subagentstart)
        printf '%s\n' "{\"permission\":\"deny\",\"followup_message\":\"$FAIL_MSG\",\"user_message\":\"$FAIL_MSG\"}" ;;
      *)
        printf '%s\n' "{\"continue\":false,\"followup_message\":\"$FAIL_MSG\",\"user_message\":\"$FAIL_MSG\"}" ;;
    esac
  else
    local json="${FAIL_JSON//FAIL_MSG/$FAIL_MSG}"
    printf '%s\n' "$json"
  fi
}

# ── Binary resolution ────────────────────────────────────────────
resolve_bin() {
  local bin="${ROUTER_RS_BIN:-}"
  if [ -z "$bin" ] && [ -x "${HOME:-}/.local/bin/router-rs" ]; then
    bin="${HOME}/.local/bin/router-rs"
  fi
  if [ -z "$bin" ]; then
    bin="$(command -v router-rs 2>/dev/null || true)"
  fi
  local CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/skill-cargo-target}"
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
    if [ -z "$bin" ] && [ -x "$candidate" ]; then
      bin="$candidate"
    fi
  done
  printf '%s' "$bin"
}

ROUTER_RS_BIN="$(resolve_bin)"

if [ ! -x "${ROUTER_RS_BIN:-}" ]; then
  if critical_event; then
    emit_fail_closed
    exit 2
  fi
  printf '%s\n' "[$HOST_ID-hook] router-rs binary unavailable for telemetry event $HOOK_EVENT; fail-open" >&2
  exit 0
fi

# ── Delegate to router-rs ────────────────────────────────────────
printf '%s' "$HOOK_PAYLOAD" | "$ROUTER_RS_BIN" $DELEGATE_CMD --event="$HOOK_EVENT" --repo-root "$ROOT"

# ── Health monitor (optional) ────────────────────────────────────
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
