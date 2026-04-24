#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
PACKAGE_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(cd -- "$PACKAGE_DIR/../.." && pwd)

cd "$REPO_ROOT"

ROUTER_BIN=${BROWSER_MCP_ROUTER_RS_BIN:-}
if [ -z "$ROUTER_BIN" ]; then
  for candidate in \
    "$REPO_ROOT/scripts/router-rs/target/release/router-rs" \
    "$REPO_ROOT/scripts/router-rs/target/debug/router-rs"
  do
    if [ -x "$candidate" ]; then
      ROUTER_BIN=$candidate
      break
    fi
  done
fi

if [ -z "$ROUTER_BIN" ] || [ ! -x "$ROUTER_BIN" ]; then
  echo "browser-mcp requires prebuilt router-rs; run cargo build --manifest-path scripts/router-rs/Cargo.toml --release before starting MCP." >&2
  exit 1
fi

RUST_ARGS=(--browser-mcp-stdio --repo-root "$REPO_ROOT")

if [ -n "${BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH:-}" ]; then
  RUST_ARGS+=(--runtime-attach-descriptor-path "$BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH:-}" ]; then
  RUST_ARGS+=(--runtime-attach-artifact-path "$BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH:-}" ]; then
  RUST_ARGS+=(--runtime-binding-artifact-path "$BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_HANDOFF_PATH:-}" ]; then
  RUST_ARGS+=(--runtime-handoff-path "$BROWSER_MCP_RUNTIME_HANDOFF_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_RESUME_MANIFEST_PATH:-}" ]; then
  RUST_ARGS+=(--runtime-resume-manifest-path "$BROWSER_MCP_RUNTIME_RESUME_MANIFEST_PATH")
fi

exec "$ROUTER_BIN" "${RUST_ARGS[@]}" "$@"
