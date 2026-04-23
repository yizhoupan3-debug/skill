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
  if command -v cargo >/dev/null 2>&1; then
    cargo build --manifest-path "$ROUTER_RS_CRATE_ROOT/Cargo.toml" >/dev/null
    if [ -x "$ROUTER_RS_DEBUG_BIN" ]; then
      "$ROUTER_RS_DEBUG_BIN" "$@"
      return
    fi
    if [ -x "$ROUTER_RS_RELEASE_BIN" ]; then
      "$ROUTER_RS_RELEASE_BIN" "$@"
      return
    fi
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

response="$(run_router_rs --claude-hook-audit-command pre-tool-use-quality --repo-root "$PROJECT_DIR")"
if printf '%s' "$response" | grep -Eq '"hookSpecificOutput"[[:space:]]*:'; then
  printf '%s\n' "$response"
fi
