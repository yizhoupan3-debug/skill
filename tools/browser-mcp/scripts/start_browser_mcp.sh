#!/bin/zsh
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

typeset -a NODE_ARGS
NODE_ARGS=()

if [ -n "${BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH:-}" ]; then
  NODE_ARGS+=(--runtime-attach-descriptor-path "$BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH:-}" ]; then
  NODE_ARGS+=(--runtime-binding-artifact-path "$BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH")
elif [ -n "${BROWSER_MCP_RUNTIME_HANDOFF_PATH:-}" ]; then
  NODE_ARGS+=(--runtime-handoff-path "$BROWSER_MCP_RUNTIME_HANDOFF_PATH")
else
  typeset -a AUTO_BINDING_CANDIDATES
  AUTO_BINDING_CANDIDATES=(
    "$REPO_ROOT"/codex_agno_runtime/artifacts/scratch/**/runtime_event_transports/*.json(N.Om[1])
  )
  if [ ${#AUTO_BINDING_CANDIDATES[@]} -gt 0 ]; then
    NODE_ARGS+=(--runtime-binding-artifact-path "$AUTO_BINDING_CANDIDATES[1]")
  fi
fi

exec node dist/index.js "${NODE_ARGS[@]}" "$@"
