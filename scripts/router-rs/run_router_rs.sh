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

pick_router_bin() {
  local best=""
  for candidate in \
    "$CRATE_ROOT/target/release/router-rs" \
    "$CRATE_ROOT/target/debug/router-rs" \
    "$SHARED_TARGET_DIR/release/router-rs" \
    "$SHARED_TARGET_DIR/debug/router-rs"
  do
    if [ -x "$candidate" ] && { [ -z "$best" ] || [ "$candidate" -nt "$best" ]; }; then
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
  CARGO_TARGET_DIR="$SHARED_TARGET_DIR" cargo build --manifest-path "$MANIFEST_PATH" --release >/dev/null
}

acquire_build_lock() {
  mkdir -p "$SHARED_TARGET_DIR"
  while true; do
    if mkdir "$BUILD_LOCK_DIR" 2>/dev/null; then
      printf '%s\n' "$$" >"$BUILD_LOCK_DIR/pid"
      return 0
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
    sleep 0.1
  done
}

release_build_lock() {
  rm -f "$BUILD_LOCK_DIR/pid"
  rmdir "$BUILD_LOCK_DIR" 2>/dev/null || true
}

ROUTER_BIN=$(pick_router_bin)

if [ -z "$ROUTER_BIN" ] || router_source_newer_than "$ROUTER_BIN"; then
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

cd "$REPO_ROOT"
exec "$ROUTER_BIN" "$@"
