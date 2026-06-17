#!/usr/bin/env bash
# RETIRED 2026-06: claude-desktop host removed from closed set. Use install-claude.sh for claude-code.
echo "RETIRED: claude-desktop is no longer supported. Use install-claude.sh instead." >&2
exit 1

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

APPLIED_ID="$(jq -r '.appliedId // empty' "$META")"
if [[ -z "$APPLIED_ID" ]]; then
  echo "error: failed to read appliedId from $META" >&2
  exit 1
fi
CONFIG_FILE="$CONFIG_LIB/${APPLIED_ID}.json"

if [[ ! -f "$CONFIG_FILE" ]]; then
  echo "error: applied config missing: $CONFIG_FILE" >&2
  exit 1
fi

export PATCH_CONFIG_FILE="$CONFIG_FILE"
export PATCH_HOSTS_JSON="$HOSTS_JSON"

is_same=$(jq --argjson hosts "$HOSTS_JSON" '.coworkEgressAllowedHosts == $hosts' "$CONFIG_FILE")
if [[ "$is_same" == "true" ]]; then
  echo "ok: coworkEgressAllowedHosts already $HOSTS_JSON in $(basename "$CONFIG_FILE")"
  exit 0
fi
tmp="${CONFIG_FILE}.tmp"
jq --argjson hosts "$HOSTS_JSON" '.coworkEgressAllowedHosts = $hosts' "$CONFIG_FILE" > "$tmp" && mv "$tmp" "$CONFIG_FILE"
echo "patched: $CONFIG_FILE"
echo "  coworkEgressAllowedHosts -> $HOSTS_JSON"
echo ""
echo "Next: Cmd+Q quit Claude Desktop, reopen, then Cowork 联网测试 (browser-mcp)."
