#!/usr/bin/env bash
# Merge coworkEgressAllowedHosts into Claude Desktop 3P configLibrary (Cowork VM egress).
set -euo pipefail

CONFIG_LIB="${CLAUDE_3P_CONFIG_LIBRARY:-$HOME/Library/Application Support/Claude-3p/configLibrary}"
HOSTS_JSON='["*"]'

usage() {
  cat <<'EOF'
Usage: patch-claude-desktop-3p-cowork-egress.sh [options]

Adds or updates coworkEgressAllowedHosts in the active Claude-3p configLibrary JSON.
Required for Cowork web access under 3P/gateway (default egress = inference gateway only).

Options:
  --allow-all          Set coworkEgressAllowedHosts to ["*"] (default)
  --hosts JSON         Custom JSON array, e.g. '["*.example.com","pypi.org"]'
  --config-library DIR Override configLibrary path
  -h, --help           Show help

After patch: fully quit Claude Desktop (Cmd+Q) and reopen.

Re-run when CC Switch re-exports config and removes egress settings.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --allow-all)
      HOSTS_JSON='["*"]'
      shift
      ;;
    --hosts)
      HOSTS_JSON="${2:?}"
      shift 2
      ;;
    --config-library)
      CONFIG_LIB="${2:?}"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ ! -d "$CONFIG_LIB" ]]; then
  echo "skip: no 3P configLibrary at $CONFIG_LIB (not in gateway/3P mode)" >&2
  exit 0
fi

META="$CONFIG_LIB/_meta.json"
if [[ ! -f "$META" ]]; then
  echo "error: missing _meta.json in $CONFIG_LIB" >&2
  exit 1
fi

APPLIED_ID="$(python3 -c "
import json, sys
try:
    data = json.load(open(sys.argv[1]))
    print(data['appliedId'])
except Exception as e:
    print(f'error: failed to read appliedId from {sys.argv[1]}: {e}', file=sys.stderr)
    sys.exit(1)
" "$META")"
CONFIG_FILE="$CONFIG_LIB/${APPLIED_ID}.json"

if [[ ! -f "$CONFIG_FILE" ]]; then
  echo "error: applied config missing: $CONFIG_FILE" >&2
  exit 1
fi

export PATCH_CONFIG_FILE="$CONFIG_FILE"
export PATCH_HOSTS_JSON="$HOSTS_JSON"

python3 <<'PY'
import json
import os
from pathlib import Path

path = Path(os.environ["PATCH_CONFIG_FILE"])
hosts = json.loads(os.environ["PATCH_HOSTS_JSON"])
data = json.loads(path.read_text())
current = data.get("coworkEgressAllowedHosts")
if current == hosts:
    print(f"ok: coworkEgressAllowedHosts already {hosts!r} in {path.name}")
    raise SystemExit(0)
data["coworkEgressAllowedHosts"] = hosts
path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
print(f"patched: {path}")
print(f"  coworkEgressAllowedHosts -> {hosts!r}")
print("")
print("Next: Cmd+Q quit Claude Desktop, reopen, then Cowork 联网测试 (browser-mcp).")
PY
