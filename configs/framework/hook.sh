#!/usr/bin/env bash
# Unified router-rs hook launcher (registry-driven, zero host-level defaults).
# Usage: hook.sh <host_id> <event>
#   stdin: payload JSON
set -euo pipefail

HOST_ID="${1:?usage: hook.sh <host_id> <event>}"
HOOK_EVENT="${2:?usage: hook.sh <host_id> <event>}"
HOOK_PAYLOAD="$(cat)"

# ── Find registry relative to this script ───────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REGISTRY_PATH="${RUNTIME_REGISTRY_JSON:-$SCRIPT_DIR/RUNTIME_REGISTRY.json}"

if ! command -v jq &>/dev/null || [ ! -r "$REGISTRY_PATH" ]; then
  echo "[hook] registry unavailable" >&2
  echo '{"decision":"block","reason":"hook registry unavailable","suppressOutput":true}'
  exit 2
fi

if ! jq -e --arg h "$HOST_ID" '.host_targets.metadata | has($h)' "$REGISTRY_PATH" &>/dev/null; then
  echo "[hook] unknown host_id: $HOST_ID" >&2
  exit 1
fi

# ── Read all host config from registry (single jq call, zero local defaults) ──
{ read -r ROOT_ENV_VAR; read -r ROOT_FALLBACK_CMD; read -r CRITICAL_EVENTS; read -r FAIL_MSG; read -r FAIL_JSON_TEMPLATE; } < <(
  jq -r --arg h "$HOST_ID" '
    .host_targets.metadata[$h].hook_launcher |
    .root_env_var, .root_fallback_cmd, .critical_events, .fail_msg, .fail_json_template
  ' "$REGISTRY_PATH"
)

# ── Resolve project root ─────────────────────────────────────────
ROOT="${!ROOT_ENV_VAR:-$(eval "$ROOT_FALLBACK_CMD")}"
if [ -z "$ROOT" ] || [ ! -d "$ROOT" ]; then
  echo "[$HOST_ID-hook] cannot resolve project root" >&2
  exit 2
fi
FW="${SKILL_FRAMEWORK_ROOT:-$ROOT}"

# ── Source host-specific env if exists ───────────────────────────
HOST_ENV_FILE="$ROOT/.$HOST_ID/router-rs-hook.env"
if [ -r "$HOST_ENV_FILE" ]; then
  set -a
  # shellcheck source=/dev/null
  . "$HOST_ENV_FILE"
  set +a
fi

# ── Dev-exempt: unified toggle (single switch for all hook interception) ─
# When ROUTER_RS_DEV_EXEMPT=1 (set in router-rs-hook.env), bypass all
# hook logic and allow the tool call through. Critical events still
# pass through to maintain lifecycle safety.
if [ "${ROUTER_RS_DEV_EXEMPT:-0}" = "1" ]; then
  exit 0
fi

# ── Critical event check ─────────────────────────────────────────
critical_event() {
  local ev; ev="$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')"
  [[ "$ev" =~ ^($CRITICAL_EVENTS)$ ]]
}

# ── Fail-closed JSON emitter (host-agnostic, uses registry template) ──
emit_fail_closed() {
  local tmpl="${FAIL_JSON_TEMPLATE:-{\"decision\":\"block\",\"reason\":\"{{MSG}}\",\"suppressOutput\":true}}"
  local msg="${FAIL_MSG:-router-rs binary unavailable}"
  local json="${tmpl//\{\{MSG\}\}/$msg}"
  printf '%s\n' "$json"
}

# ── Binary resolution ────────────────────────────────────────────
resolve_bin() {
  local bin="${ROUTER_RS_BIN:-}"
  [ -n "$bin" ] && [ -x "$bin" ] && { printf '%s' "$bin"; return; }

  if [ -x "${HOME:-}/.local/bin/router-rs-cli" ]; then
    printf '%s' "${HOME}/.local/bin/router-rs-cli"; return
  fi

  local candidate
  candidate="$(command -v router-rs-cli 2>/dev/null || true)"
  [ -n "$candidate" ] && { printf '%s' "$candidate"; return; }

  local cargo_target_dir="${CARGO_TARGET_DIR:-/tmp/skill-cargo-target}"
  for candidate in \
    "$cargo_target_dir/release/router-rs-cli" \
    "$cargo_target_dir/debug/router-rs-cli" \
    "$ROOT/core/router-rs/target/release/router-rs-cli" \
    "$FW/core/router-rs/target/release/router-rs-cli" \
    "$ROOT/core/router-rs/target/debug/router-rs-cli" \
    "$FW/core/router-rs/target/debug/router-rs-cli" \
    "$ROOT/target/release/router-rs-cli" \
    "$ROOT/target/debug/router-rs-cli" \
    "$FW/target/release/router-rs-cli" \
    "$FW/target/debug/router-rs-cli"
  do
    [ -x "$candidate" ] && { printf '%s' "$candidate"; return; }
  done
}

ROUTER_RS_BIN="$(resolve_bin)"

if [ ! -x "${ROUTER_RS_BIN:-}" ]; then
  if critical_event; then
    emit_fail_closed
    exit 2
  fi
  printf '%s\n' "[$HOST_ID-hook] router-rs binary unavailable for $HOOK_EVENT; fail-open" >&2
  exit 0
fi

# ── Delegate to router-rs ────────────────────────────────────────
printf '%s' "$HOOK_PAYLOAD" | "$ROUTER_RS_BIN" host hook --event="$HOOK_EVENT" --repo-root "$ROOT" "$HOST_ID"

# ── Health monitor (optional) ────────────────────────────────────
HEALTH_MONITOR="$FW/configs/framework/agent-health-monitor.sh"
if [ -x "$HEALTH_MONITOR" ]; then
  case "$(printf '%s' "$HOOK_EVENT" | tr '[:upper:]' '[:lower:]')" in
    subagentstart)
      (
        AGENT_NAME=$(printf '%s' "$HOOK_PAYLOAD" | jq -r '.tool_name // "unknown"' 2>/dev/null || echo "unknown")
        AGENT_ID=$(printf '%s' "$HOOK_PAYLOAD" | jq -r '(.tool_input.name // .tool_input.description // ("agent-" + ($$ | tostring)))' 2>/dev/null || echo "agent-$$")
        "$HEALTH_MONITOR" start "$AGENT_NAME" "$AGENT_ID"
      ) >/dev/null 2>&1 || true
      ;;
    subagentstop)
      (
        AGENT_ID=$(printf '%s' "$HOOK_PAYLOAD" | jq -r '(.tool_input.name // .tool_input.description // ("agent-" + ($$ | tostring)))' 2>/dev/null || echo "agent-$$")
        "$HEALTH_MONITOR" stop "$AGENT_ID"
      ) >/dev/null 2>&1 || true
      ;;
  esac
fi
