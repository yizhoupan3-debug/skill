#!/bin/sh
set -eu

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
ROUTER_RS_RELEASE_BIN="$PROJECT_DIR/scripts/router-rs/target/release/router-rs"
ROUTER_RS_DEBUG_BIN="$PROJECT_DIR/scripts/router-rs/target/debug/router-rs"
ROUTER_RS_CRATE_ROOT="$PROJECT_DIR/scripts/router-rs"

router_rs_is_fresh() {
  bin_path="$1"
  [ -x "$bin_path" ] || return 1
  [ "$ROUTER_RS_CRATE_ROOT/Cargo.toml" -nt "$bin_path" ] && return 1
  find "$ROUTER_RS_CRATE_ROOT/src" -type f -newer "$bin_path" | grep -q . && return 1
  return 0
}

run_router_rs() {
  if router_rs_is_fresh "$ROUTER_RS_RELEASE_BIN"; then
    "$ROUTER_RS_RELEASE_BIN" "$@"
    return
  fi
  if router_rs_is_fresh "$ROUTER_RS_DEBUG_BIN"; then
    "$ROUTER_RS_DEBUG_BIN" "$@"
    return
  fi
  if [ -x "$ROUTER_RS_RELEASE_BIN" ]; then
    "$ROUTER_RS_RELEASE_BIN" "$@"
    return
  fi
  if [ -x "$ROUTER_RS_DEBUG_BIN" ]; then
    "$ROUTER_RS_DEBUG_BIN" "$@"
    return
  fi
  echo "Missing required router-rs binary: $ROUTER_RS_RELEASE_BIN or $ROUTER_RS_DEBUG_BIN" >&2
  exit 1
}

command_name="${1:-}"
if [ -z "$command_name" ]; then
  echo "Missing Claude hook command name" >&2
  exit 1
fi

case "$command_name" in
  session-end)
    run_router_rs --claude-hook-command session-end --repo-root "$PROJECT_DIR" >/dev/null
    ;;
  config-change|stop-failure)
    run_router_rs --claude-hook-audit-command "$command_name" --repo-root "$PROJECT_DIR" >/dev/null
    ;;
  pre-tool-use)
    response="$(run_router_rs --claude-hook-audit-command pre-tool-use --repo-root "$PROJECT_DIR")"
    if printf '%s' "$response" | grep -Eq '"permissionDecision"[[:space:]]*:[[:space:]]*"deny"'; then
      printf '%s\n' "$response"
    fi
    ;;
  user-prompt-submit)
    response="$(run_router_rs --claude-hook-audit-command "$command_name" --repo-root "$PROJECT_DIR")"
    if [ -n "$response" ]; then
      if printf '%s' "$response" | grep -Eq '"hookSpecificOutput"[[:space:]]*:'; then
        printf '%s\n' "$response"
      else
        printf '[claude-user-prompt-submit] shared hook returned no hookSpecificOutput; continuing with degraded context.\n' >&2
      fi
    fi
    ;;
  pre-tool-use-quality|post-tool-audit)
    response="$(run_router_rs --claude-hook-audit-command "$command_name" --repo-root "$PROJECT_DIR")"
    if printf '%s' "$response" | grep -Eq '"hookSpecificOutput"[[:space:]]*:'; then
      printf '%s\n' "$response"
    fi
    ;;
  *)
    echo "Unsupported Claude hook command: $command_name" >&2
    exit 1
    ;;
esac
