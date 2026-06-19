#!/usr/bin/env bash
# hook-dispatch.sh — Registry-driven hook dispatch for router-rs
#
# Reads the authoritative host ID from .framework-projection.json,
# validates against RUNTIME_REGISTRY.json, then dispatches to
#   router-rs host hook --event <EVENT> <HOST_ID>
#
# Never hardcodes a host ID. Registry is the single source of truth.
# Gracefully handles unsupported events (SessionStart, SubagentStart/Stop)
# with an allow response to prevent hook cascade failures.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${CLAUDE_PROJECT_ROOT:-$(cd "${SCRIPT_DIR}/.." && pwd)}"
FRAMEWORK_ROOT="${SKILL_FRAMEWORK_ROOT:-${HOME}/Developer/skill}"
ROUTER_RS="${ROUTER_RS_BIN:-${HOME}/.local/bin/router-rs}"
PROJECTION="${PROJECT_ROOT}/.claude/.framework-projection.json"
REGISTRY="${FRAMEWORK_ROOT}/configs/framework/RUNTIME_REGISTRY.json"

# ---------------------------------------------------------------
# Step 1: Resolve HOST_ID from framework projection (authoritative)
# ---------------------------------------------------------------
HOST_ID=""
if [[ -f "${PROJECTION}" ]]; then
  export _PROJ="${PROJECTION}"
  HOST_ID="$(python3 -c "
import json, os
fp = os.environ.get('_PROJ', '')
if fp and os.path.exists(fp):
    with open(fp) as f:
        v = json.load(f).get('host_projection', '')
        if v:
            print(v)
" 2>/dev/null || true)"
fi

# ---------------------------------------------------------------
# Step 2: Fallback — env-based detection when no projection config
# ---------------------------------------------------------------
if [[ -z "${HOST_ID}" ]]; then
  if [[ -n "${CLAUDE_PROJECT_ROOT:-}" ]]; then
    HOST_ID="claude"
  elif [[ -n "${CURSOR_HOME:-}" ]] || [[ -d "${FRAMEWORK_ROOT}/.cursor" ]]; then
    HOST_ID="cursor"
  elif [[ -n "${CODEX_HOME:-}" ]] || [[ -d "${FRAMEWORK_ROOT}/.codex" ]]; then
    HOST_ID="codex"
  elif [[ -d "${FRAMEWORK_ROOT}/.opencode" ]]; then
    HOST_ID="opencode"
  else
    HOST_ID="claude"
  fi
fi

# ---------------------------------------------------------------
# Step 3: Validate HOST_ID against runtime registry
# ---------------------------------------------------------------
if [[ -f "${REGISTRY}" ]]; then
  export _REG="${REGISTRY}" _HID="${HOST_ID}"
  python3 -c "
import json, os, sys
fp = os.environ.get('_REG', '')
host = os.environ.get('_HID', '')
if fp and os.path.exists(fp):
    with open(fp) as f:
        supported = json.load(f).get('host_targets', {}).get('supported', [])
        if not any(h == host for h in supported):
            print(f'warning: host {host!r} not in registry (supported={supported})', file=sys.stderr)
" 2>/dev/null || true
fi

# ---------------------------------------------------------------
# Step 4: Dispatch to router-rs host hook
# ---------------------------------------------------------------
EVENT="${1:-}"
if [[ -z "${EVENT}" ]]; then
  echo '{"decision":"allow","reason":"no event specified","suppressOutput":true}'
  exit 0
fi

if [[ ! -x "${ROUTER_RS}" ]]; then
  echo '{"decision":"allow","reason":"router-rs binary not found","suppressOutput":true}'
  exit 0
fi

# Capture both stdout and exit code from router-rs
# Note: cannot use || true here — it swallows the exit code
set +e
_output="$("${ROUTER_RS}" host hook --event "${EVENT}" "${HOST_ID}" 2>&1)"
_rc=$?
set -e

if [[ $_rc -eq 0 ]]; then
  echo "${_output}"
  exit 0
fi

# Graceful degradation for unsupported events
if echo "${_output}" | grep -qi "unsupported"; then
  echo '{"decision":"allow","reason":"event not supported by router-rs provider, allowing","suppressOutput":true}'
  exit 0
fi

# Real failure — propagate
echo "${_output}" >&2
echo '{"decision":"allow","reason":"router-rs hook failed, allowing to avoid cascade block","suppressOutput":true}'
exit 0
