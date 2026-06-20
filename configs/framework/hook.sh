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
    FAIL_MSG='router-rs binary unavailable for Claude hook'
    FAIL_JSON='{"decision":"block","reason":"FAIL_MSG","suppressOutput":true}'
    ;;
  codex)
    ROOT_ENV_VAR="CODEX_PROJECT_ROOT"
    ROOT_FALLBACK='git rev-parse --show-toplevel 2>/dev/null || pwd'
    CRITICAL_EVENTS="sessionstart|pretooluse|userpromptsubmit|posttooluse|stop"
    FAIL_MSG='router-rs binary unavailable for critical Codex hook; fail-closed instead of silently bypassing gate enforcement'
    FAIL_JSON='{"decision":"block","reason":"FAIL_MSG","suppressOutput":true}'
    ;;
  cursor)
    ROOT_ENV_VAR="CURSOR_WORKSPACE_ROOT"
    ROOT_FALLBACK='git rev-parse --show-toplevel 2>/dev/null || echo "$PWD"'
    CRITICAL_EVENTS="beforesubmitprompt|stop|posttooluse|subagentstart|subagentstop"
    FAIL_MSG='router-rs binary unavailable for critical Cursor hook; fail-closed instead of silently bypassing gate enforcement'
    CURSOR_FORMAT=1
    ;;
  opencode)
    ROOT_ENV_VAR="OPENCODE_PROJECT_ROOT"
    ROOT_FALLBACK='git rev-parse --show-toplevel 2>/dev/null || pwd'
    CRITICAL_EVENTS="tool.execute.before|tool.execute.after|session.idle|session.created"
    FAIL_MSG='router-rs binary unavailable for critical OpenCode hook; fail-closed instead of silently bypassing gate enforcement'
    FAIL_JSON='{"decision":"block","reason":"FAIL_MSG","suppressOutput":true}'
    ;;
  *)
    echo "Unknown host_id: $HOST_ID" >&2
    exit 1
    ;;
esac

# ── Resolve project root ─────────────────────────────────────────
ROOT="${!ROOT_ENV_VAR:-$(eval "$ROOT_FALLBACK")}"
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

# ── Source host-specific env if exists (unified for all hosts) ───
HOST_ENV_FILE="$ROOT/.$HOST_ID/router-rs-hook.env"
if [ -r "$HOST_ENV_FILE" ]; then
  set -a
  # shellcheck source=/dev/null
  . "$HOST_ENV_FILE"
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
# Performance: skip --help subprocess for -cli binaries (they can't be the shim).
is_redirect_shim() {
  case "$1" in *-cli) return 1 ;; esac
  "$1" --help 2>&1 | grep -q "binary moved"
}

resolve_bin() {
  local bin="${ROUTER_RS_BIN:-}"

  # If env var is set, verify it's not the redirect shim
  if [ -n "$bin" ] && [ -x "$bin" ]; then
    if is_redirect_shim "$bin"; then
      bin=""
    fi
  elif [ -n "$bin" ]; then
    bin=""
  fi

  # Early return if env var resolved successfully
  if [ -n "$bin" ]; then
    printf '%s' "$bin"
    return
  fi

  # Prefer router-rs-cli over router-rs (which is now a redirect shim)
  if [ -x "${HOME:-}/.local/bin/router-rs-cli" ]; then
    printf '%s' "${HOME}/.local/bin/router-rs-cli"
    return
  fi
  if [ -x "${HOME:-}/.local/bin/router-rs" ] && ! is_redirect_shim "${HOME}/.local/bin/router-rs"; then
    printf '%s' "${HOME}/.local/bin/router-rs"
    return
  fi

  local candidate
  candidate="$(command -v router-rs-cli 2>/dev/null || true)"
  if [ -n "$candidate" ]; then
    printf '%s' "$candidate"
    return
  fi
  candidate="$(command -v router-rs 2>/dev/null || true)"
  if [ -n "$candidate" ] && ! is_redirect_shim "$candidate"; then
    printf '%s' "$candidate"
    return
  fi

  local CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/skill-cargo-target}"
  for candidate in \
    "$CARGO_TARGET_DIR/release/router-rs-cli" \
    "$CARGO_TARGET_DIR/debug/router-rs-cli" \
    "$ROOT/core/router-rs/target/release/router-rs-cli" \
    "$FW/core/router-rs/target/release/router-rs-cli" \
    "$ROOT/core/router-rs/target/debug/router-rs-cli" \
    "$FW/core/router-rs/target/debug/router-rs-cli" \
    "$ROOT/target/release/router-rs-cli" \
    "$ROOT/target/debug/router-rs-cli" \
    "$FW/target/release/router-rs-cli" \
    "$FW/target/debug/router-rs-cli" \
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
    if [ -x "$candidate" ]; then
      printf '%s' "$candidate"
      return
    fi
  done
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

# ── Delegate to router-rs (with timeout guard) ───────────────────
# Command format: router-rs-cli host hook --event=<EVENT> --repo-root <ROOT> <HOST_ID>
_ROUTER_HOOK_TIMEOUT=10
printf '%s' "$HOOK_PAYLOAD" | "$ROUTER_RS_BIN" host hook --event="$HOOK_EVENT" --repo-root "$ROOT" "$HOST_ID" &
_ROUTER_PID=$!
(sleep "${_ROUTER_HOOK_TIMEOUT}" && kill "${_ROUTER_PID}" 2>/dev/null) &
_TIMER_PID=$!
_hook_rc=0
wait "${_ROUTER_PID}" 2>/dev/null || _hook_rc=$?
kill "${_TIMER_PID}" 2>/dev/null || true
wait "${_TIMER_PID}" 2>/dev/null || true  # reap zombie; suppress set -e (timer exit code irrelevant)
if [ "$_hook_rc" -eq 137 ] || [ "$_hook_rc" -eq 143 ]; then
  # 137 = SIGKILL (128+9), 143 = SIGTERM (128+15)
  echo "[$HOST_ID-hook] router-rs timed out after ${_ROUTER_HOOK_TIMEOUT}s for $HOOK_EVENT" >&2
  exit 2
fi
if [ "$_hook_rc" -ne 0 ]; then
  if critical_event; then
    emit_fail_closed
    exit 2
  fi
  printf '%s
' "[$HOST_ID-hook] router-rs failed for non-critical event $HOOK_EVENT (exit $_hook_rc); fail-open" >&2
  exit 0
fi

# ── Health monitor (optional) ────────────────────────────────────
HEALTH_MONITOR="$FW/configs/framework/agent-health-monitor.sh"
if [ -x "$HEALTH_MONITOR" ]; then
  case "$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')" in
    subagentstart)
      AGENT_NAME=$(printf '%s' "$HOOK_PAYLOAD" | jq -r '.tool_name // "unknown"' 2>/dev/null || echo "unknown")
      AGENT_ID=$(printf '%s' "$HOOK_PAYLOAD" | jq -r '(.tool_input.name // .tool_input.description // ("agent-" + ($$ | tostring)))' 2>/dev/null || echo "agent-$$")
      "$HEALTH_MONITOR" start "$AGENT_NAME" "$AGENT_ID" >/dev/null 2>&1 || true
      ;;
    subagentstop)
      AGENT_ID=$(printf '%s' "$HOOK_PAYLOAD" | jq -r '(.tool_input.name // .tool_input.description // ("agent-" + ($$ | tostring)))' 2>/dev/null || echo "agent-$$")
      "$HEALTH_MONITOR" stop "$AGENT_ID" >/dev/null 2>&1 || true
      ;;
  esac
fi
