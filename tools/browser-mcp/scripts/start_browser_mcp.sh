#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
PACKAGE_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(cd -- "$PACKAGE_DIR/../.." && pwd)

cd "$REPO_ROOT"

ROUTER_BIN=${BROWSER_MCP_ROUTER_RS_BIN:-}
ROUTER_LAUNCHER_ARGS=()
if [ -z "$ROUTER_BIN" ]; then
  ROUTER_BIN="$REPO_ROOT/scripts/router-rs/run_router_rs.sh"
  ROUTER_LAUNCHER_ARGS=("$REPO_ROOT/scripts/router-rs/Cargo.toml")
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

exec "$ROUTER_BIN" "${ROUTER_LAUNCHER_ARGS[@]}" "${RUST_ARGS[@]}" "$@"
