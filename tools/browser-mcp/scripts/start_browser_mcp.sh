#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
PACKAGE_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(cd -- "$PACKAGE_DIR/../.." && pwd)

cd "$PACKAGE_DIR"

if [ ! -d node_modules ]; then
  npm install >/dev/null
fi

if [ ! -f dist/index.js ] || [ src/index.ts -nt dist/index.js ] || [ src/runtime.ts -nt dist/runtime.js ] || [ src/server.ts -nt dist/server.js ] || [ src/types.ts -nt dist/types.js ] || [ src/errors.ts -nt dist/errors.js ]; then
  npm run build >/dev/null
fi

NODE_ARGS=()

if [ -n "${BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH:-}" ]; then
  NODE_ARGS+=(--runtime-attach-descriptor-path "$BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH:-}" ]; then
  NODE_ARGS+=(--runtime-attach-artifact-path "$BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH:-}" ]; then
  NODE_ARGS+=(--runtime-binding-artifact-path "$BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_HANDOFF_PATH:-}" ]; then
  NODE_ARGS+=(--runtime-handoff-path "$BROWSER_MCP_RUNTIME_HANDOFF_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_RESUME_MANIFEST_PATH:-}" ]; then
  NODE_ARGS+=(--runtime-resume-manifest-path "$BROWSER_MCP_RUNTIME_RESUME_MANIFEST_PATH")
else
  AUTO_ATTACH_ARTIFACT=$(node "$SCRIPT_DIR/resolve_runtime_attach_artifact.mjs" 2>/dev/null || true)
  if [ -n "$AUTO_ATTACH_ARTIFACT" ]; then
    NODE_ARGS+=(--runtime-attach-artifact-path "$AUTO_ATTACH_ARTIFACT")
  fi
fi

exec node dist/index.js "${NODE_ARGS[@]}" "$@"
