#!/bin/zsh
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
PACKAGE_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd)

cd "$PACKAGE_DIR"

if [ ! -d node_modules ]; then
  npm install >/dev/null
fi

if [ ! -f dist/index.js ] || [ src/index.ts -nt dist/index.js ] || [ src/runtime.ts -nt dist/runtime.js ] || [ src/server.ts -nt dist/server.js ] || [ src/types.ts -nt dist/types.js ] || [ src/errors.ts -nt dist/errors.js ]; then
  npm run build >/dev/null
fi

exec node dist/index.js
