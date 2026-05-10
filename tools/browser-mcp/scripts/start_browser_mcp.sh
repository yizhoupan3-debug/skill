#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
PACKAGE_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(cd -- "$PACKAGE_DIR/../.." && pwd)

cd "$REPO_ROOT"

ROUTER_BIN=${BROWSER_MCP_ROUTER_RS_BIN:-}
ROUTER_EXTRA_ARGS=()

if [ -z "$ROUTER_BIN" ]; then
  if [ -x "$REPO_ROOT/scripts/router-rs/target/release/router-rs" ]; then
    ROUTER_BIN="$REPO_ROOT/scripts/router-rs/target/release/router-rs"
  elif [ -x "$REPO_ROOT/scripts/router-rs/target/debug/router-rs" ]; then
    ROUTER_BIN="$REPO_ROOT/scripts/router-rs/target/debug/router-rs"
  elif [ -x "$REPO_ROOT/scripts/router-rs/run_router_rs.sh" ]; then
    ROUTER_BIN="$REPO_ROOT/scripts/router-rs/run_router_rs.sh"
    ROUTER_EXTRA_ARGS=("$REPO_ROOT/scripts/router-rs/Cargo.toml")
  elif command -v router-rs >/dev/null 2>&1; then
    ROUTER_BIN="$(command -v router-rs)"
  else
    echo "router-rs binary not found; install with 'router-rs self install' or build: cargo build --release --manifest-path $REPO_ROOT/scripts/router-rs/Cargo.toml" >&2
    exit 127
  fi
fi

RUST_ARGS=(browser mcp-stdio --repo-root "$REPO_ROOT")

if [ -n "${BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH:-}" ]; then
  RUST_ARGS+=(--runtime-attach-descriptor-path "$BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH:-}" ]; then
  RUST_ARGS+=(--runtime-attach-artifact-path "$BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH")
fi

exec "$ROUTER_BIN" "${ROUTER_EXTRA_ARGS[@]}" "${RUST_ARGS[@]}" "$@"
