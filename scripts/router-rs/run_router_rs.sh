#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "usage: run_router_rs.sh /abs/path/to/Cargo.toml [router-rs args...]" >&2
  exit 2
fi

MANIFEST_PATH=$1
shift

CRATE_ROOT=$(cd -- "$(dirname -- "$MANIFEST_PATH")" && pwd)
REPO_ROOT=$(cd -- "$CRATE_ROOT/../.." && pwd)
SHARED_TARGET_DIR=${CARGO_TARGET_DIR:-/tmp/skill-cargo-target}
BUILD_LOCK_DIR="$SHARED_TARGET_DIR/.router-rs-build.lock"
BUILD_LOCK_TIMEOUT_SEC=${ROUTER_RS_BUILD_LOCK_TIMEOUT_SEC:-30}

router_bin_compatible() {
  local candidate=$1
  [ -x "$candidate" ] || return 1
  case "$(uname -s 2>/dev/null):$(uname -m 2>/dev/null)" in
    Linux:x86_64) file "$candidate" 2>/dev/null | grep -Eq 'ELF 64-bit.*x86-64' ;;
    Linux:aarch64|Linux:arm64) file "$candidate" 2>/dev/null | grep -Eq 'ELF 64-bit.*(ARM aarch64|ARM64)' ;;
    Darwin:arm64) file "$candidate" 2>/dev/null | grep -Eq 'Mach-O 64-bit.*arm64' ;;
    Darwin:x86_64) file "$candidate" 2>/dev/null | grep -Eq 'Mach-O 64-bit.*x86_64' ;;
    *) "$candidate" --help >/dev/null 2>&1 ;;
  esac
}

pick_router_bin() {
  local best=""
  for candidate in \
    "$CRATE_ROOT/target/release/router-rs" \
    "$CRATE_ROOT/target/debug/router-rs" \
    "$SHARED_TARGET_DIR/release/router-rs" \
    "$SHARED_TARGET_DIR/debug/router-rs"
  do
    if router_bin_compatible "$candidate" && { [ -z "$best" ] || [ "$candidate" -nt "$best" ]; }; then
      best=$candidate
    fi
  done
  printf '%s' "$best"
}

router_source_newer_than() {
  local binary=$1
  local source

  for source in "$CRATE_ROOT/Cargo.toml" "$CRATE_ROOT/Cargo.lock" "$REPO_ROOT/AGENTS.md"; do
    if [ -e "$source" ] && [ "$source" -nt "$binary" ]; then
      return 0
    fi
  done

  if [ -d "$CRATE_ROOT/src" ] && find "$CRATE_ROOT/src" -type f -newer "$binary" -print -quit | grep -q .; then
    return 0
  fi

  return 1
}

build_router_bin() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "router-rs launcher needs cargo to build a compatible router-rs binary, but cargo was not found on PATH." >&2
    echo "Install Rust/Cargo or provide a compatible router-rs binary under $SHARED_TARGET_DIR/{release,debug}/router-rs." >&2
    return 127
  fi
  CARGO_TARGET_DIR="$SHARED_TARGET_DIR" cargo build --manifest-path "$MANIFEST_PATH" --release >/dev/null
}

acquire_build_lock() {
  mkdir -p "$SHARED_TARGET_DIR"
  local lock_start=${SECONDS:-0}
  local lock_owner=""
  local sleep_ms=50
  local max_sleep_ms=1000
  local jitter_ms=0
  local wait_ms=0
  while true; do
    if mkdir "$BUILD_LOCK_DIR" 2>/dev/null; then
      if printf '%s\n' "$$" >"$BUILD_LOCK_DIR/pid" 2>/dev/null; then
        return 0
      fi
      rmdir "$BUILD_LOCK_DIR" 2>/dev/null || true
      continue
    fi
    if [ -f "$BUILD_LOCK_DIR/pid" ]; then
      lock_owner=$(cat "$BUILD_LOCK_DIR/pid" 2>/dev/null || true)
      if [ -n "$lock_owner" ] && ! kill -0 "$lock_owner" 2>/dev/null; then
        rm -f "$BUILD_LOCK_DIR/pid"
        rmdir "$BUILD_LOCK_DIR" 2>/dev/null || true
        continue
      fi
    elif rmdir "$BUILD_LOCK_DIR" 2>/dev/null; then
      continue
    fi
    if [ "${BUILD_LOCK_TIMEOUT_SEC:-0}" -gt 0 ] && [ $((SECONDS - lock_start)) -ge "$BUILD_LOCK_TIMEOUT_SEC" ]; then
      echo "router-rs launcher timed out waiting for build lock at $BUILD_LOCK_DIR after ${BUILD_LOCK_TIMEOUT_SEC}s." >&2
      if [ -n "$lock_owner" ]; then
        echo "lock owner pid: $lock_owner" >&2
      fi
      echo "Set ROUTER_RS_BUILD_LOCK_TIMEOUT_SEC to tune this threshold." >&2
      return 1
    fi
    jitter_ms=$((RANDOM % ((sleep_ms / 4) + 1)))
    wait_ms=$((sleep_ms + jitter_ms))
    sleep "$(awk "BEGIN {printf \"%.3f\", ${wait_ms}/1000}")"
    if [ "$sleep_ms" -lt "$max_sleep_ms" ]; then
      sleep_ms=$((sleep_ms * 2))
      if [ "$sleep_ms" -gt "$max_sleep_ms" ]; then
        sleep_ms=$max_sleep_ms
      fi
    fi
  done
}

release_build_lock() {
  local lock_owner=""
  if [ -f "$BUILD_LOCK_DIR/pid" ]; then
    lock_owner=$(cat "$BUILD_LOCK_DIR/pid" 2>/dev/null || true)
  fi
  if [ "$lock_owner" = "$$" ]; then
    rm -f "$BUILD_LOCK_DIR/pid"
    rmdir "$BUILD_LOCK_DIR" 2>/dev/null || true
  fi
}

ROUTER_BIN=$(pick_router_bin)

if [ "${ROUTER_RS_NO_REBUILD:-}" != "1" ] && { [ -z "$ROUTER_BIN" ] || router_source_newer_than "$ROUTER_BIN"; }; then
  acquire_build_lock
  trap release_build_lock EXIT
  ROUTER_BIN=$(pick_router_bin)
  if [ -z "$ROUTER_BIN" ] || router_source_newer_than "$ROUTER_BIN"; then
    build_router_bin
  fi
  release_build_lock
  trap - EXIT
  ROUTER_BIN=$(pick_router_bin)
fi

if [ -z "$ROUTER_BIN" ]; then
  echo "router-rs launcher could not find or build router-rs" >&2
  exit 1
fi

if [ "$#" -gt 0 ] && { [ "$1" = "route" ] || [ "$1" = "search" ]; }; then
  HAS_ROUTE_SOURCE=0
  for arg in "$@"; do
    case "$arg" in
      --runtime|--runtime=*|--manifest|--manifest=*) HAS_ROUTE_SOURCE=1 ;;
    esac
  done
  if [ "$HAS_ROUTE_SOURCE" = "0" ] && [ -f "$REPO_ROOT/skills/SKILL_ROUTING_RUNTIME.json" ]; then
    ROUTER_SUBCOMMAND=$1
    shift
    set -- "$ROUTER_SUBCOMMAND" --runtime "$REPO_ROOT/skills/SKILL_ROUTING_RUNTIME.json" "$@"
  fi
fi

cd "$REPO_ROOT"
exec "$ROUTER_BIN" "$@"
