#!/usr/bin/env bash
# Shared hook library for router-rs host hooks.
# Source this file; do not execute directly.
# Usage: . "$(dirname "${BASH_SOURCE[0]}")/_shared_hook_lib.sh"
set -euo pipefail

# resolve_binary ROOT FW — Resolve router-rs binary into global ROUTER_RS_BIN.
# Priority: ROUTER_RS_BIN env → ~/.local/bin → 10 build-tree candidates → command -v.
# If BINARY_VALIDATE_CMD is set, runs it as a validation gate (e.g. "host claude hook --help").
# Returns 0 if resolved, 1 if unavailable.
resolve_binary() {
  local _root="$1" _fw="$2" _ctd
  _ctd="${CARGO_TARGET_DIR:-/tmp/skill-cargo-target}"
  [ -n "${ROUTER_RS_BIN:-}" ] && [ -x "$ROUTER_RS_BIN" ] && { _validate_bin && return 0; }
  [ -x "${HOME:-}/.local/bin/router-rs" ] && ROUTER_RS_BIN="${HOME}/.local/bin/router-rs" && { _validate_bin && return 0; }
  local _candidates="$_root/core/router-rs/target/release/router-rs $_fw/core/router-rs/target/release/router-rs $_root/core/router-rs/target/debug/router-rs $_fw/core/router-rs/target/debug/router-rs $_ctd/release/router-rs $_ctd/debug/router-rs $_root/target/release/router-rs $_root/target/debug/router-rs $_fw/target/release/router-rs $_fw/target/debug/router-rs"
  local _c; for _c in $_candidates; do
    [ -x "$_c" ] && ROUTER_RS_BIN="$_c" && { _validate_bin && return 0; } || true
  done
  ROUTER_RS_BIN="$(command -v router-rs 2>/dev/null || true)"
  [ -n "$ROUTER_RS_BIN" ] && [ -x "$ROUTER_RS_BIN" ] && { _validate_bin && return 0; }
  ROUTER_RS_BIN=""; return 1
}

# _validate_bin — internal; runs BINARY_VALIDATE_CMD if set.
_validate_bin() {
  if [ -n "${BINARY_VALIDATE_CMD:-}" ]; then
    "$ROUTER_RS_BIN" $BINARY_VALIDATE_CMD >/dev/null 2>&1 || { ROUTER_RS_BIN=""; return 1; }
  fi
  return 0
}

# is_critical_event EVENT "event1|event2|..." — Parameterized event matching.
# Returns 0 if EVENT matches any listed event (case-insensitive), 1 otherwise.
is_critical_event() {
  local _ev; _ev="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  local _list="$2" _item
  local _old_ifs="$IFS"; IFS='|'
  for _item in $_list; do
    IFS="$_old_ifs"
    [ "$_ev" = "$_item" ] && return 0
  done
  IFS="$_old_ifs"
  return 1
}

# emit_fail_closed_json HOST EVENT FORMAT — Fail-closed JSON on stdout.
# FORMAT=simple: {"decision":"block",...}. FORMAT=cursor-per-event: event-aware.
emit_fail_closed_json() {
  local _host="$1" _event="$2" _fmt="$3"
  local _msg="router-rs binary unavailable for critical ${_host} hook; fail-closed"
  case "$_fmt" in
    simple)
      printf '%s\n' "{\"decision\":\"block\",\"reason\":\"${_msg}\",\"suppressOutput\":true}" ;;
    cursor-per-event)
      local _ev; _ev="$(printf '%s' "$_event" | tr '[:upper:]' '[:lower:]')"
      case "$_ev" in
        beforesubmitprompt)
          printf '%s\n' "{\"continue\":false,\"followup_message\":\"${_msg}\",\"user_message\":\"${_msg}\"}" ;;
        subagentstart)
          printf '%s\n' "{\"permission\":\"deny\",\"followup_message\":\"${_msg}\",\"user_message\":\"${_msg}\"}" ;;
        *)
          printf '%s\n' "{\"continue\":false,\"followup_message\":\"${_msg}\",\"user_message\":\"${_msg}\"}" ;;
      esac ;;
  esac
}

# emit_fail_open_json HOST [REASON] — Fail-open JSON on stdout.
emit_fail_open_json() {
  local _host="$1" _reason="${2:-router-rs unavailable, running without framework}"
  printf '%s\n' "{\"decision\":\"allow\",\"reason\":\"${_reason}\",\"suppressOutput\":true}"
}

# fail_open_warn HOST EVENT — Fail-open warning to stderr.
fail_open_warn() {
  printf '[%s-hook] router-rs unavailable for %s; fail-open (set ROUTER_RS_HOOK_FAIL_OPEN=0 to fail-closed)\n' "$1" "$2" >&2
}
