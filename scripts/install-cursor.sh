#!/usr/bin/env bash
# Install / refresh Cursor user-scope framework projection (framework.mdc + browser MCP).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install-cursor.sh [options]

Installs harness projections for Cursor (user scope only for framework.mdc).

Writes:
  user → ~/.cursor/rules/framework.mdc, ~/.cursor/mcp.json (browser-mcp)

Options:
  --framework-root DIR   Framework repo (default: $SKILL_FRAMEWORK_ROOT or script ../..)
  --project-root DIR     Project root for status (default: $PWD)
  --skip-build           Do not build router-rs when missing
  -h, --help             Show help

Example:
  ./scripts/install-cursor.sh
EOF
}

resolve_framework_root() {
  if [[ -n "${FRAMEWORK_ROOT_ARG:-}" ]]; then
    cd "$FRAMEWORK_ROOT_ARG" && pwd
    return
  fi
  if [[ -n "${SKILL_FRAMEWORK_ROOT:-}" ]]; then
    cd "$SKILL_FRAMEWORK_ROOT" && pwd
    return
  fi
  local here
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  if [[ "$here" == */scripts ]]; then
    cd "$here/.." && pwd
    return
  fi
  echo "error: cannot infer framework root" >&2
  exit 1
}

router_rs_cmd() {
  local candidates=()
  candidates+=("$FRAMEWORK_ROOT/core/router-rs/target/release/router-rs")
  if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    candidates+=("${CARGO_TARGET_DIR}/release/router-rs")
  fi
  candidates+=("/tmp/skill-${UID:-0}-cargo-target/release/router-rs")
  local c
  for c in "${candidates[@]}"; do
    if [[ -x "$c" ]]; then
      echo "$c"
      return
    fi
  done
  if [[ -n "${ROUTER_RS_BIN:-}" && -x "$ROUTER_RS_BIN" ]]; then
    echo "$ROUTER_RS_BIN"
    return
  fi
  if command -v router-rs >/dev/null 2>&1; then
    command -v router-rs
    return
  fi
  echo "error: router-rs not found" >&2
  exit 1
}

ensure_router_rs() {
  if [[ "${SKIP_BUILD:-0}" == 1 ]]; then
    return
  fi
  if command -v router-rs >/dev/null 2>&1; then
    return
  fi
  echo "==> building router-rs (release)..." >&2
  (cd "$FRAMEWORK_ROOT/core/router-rs" && cargo build --release)
}

FRAMEWORK_ROOT_ARG=""
PROJECT_ROOT=""
SKIP_BUILD=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --framework-root)
      FRAMEWORK_ROOT_ARG="${2:?}"
      shift 2
      ;;
    --project-root)
      PROJECT_ROOT="${2:?}"
      shift 2
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
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

FRAMEWORK_ROOT="$(resolve_framework_root)"
PROJECT_ROOT="${PROJECT_ROOT:-$PWD}"
PROJECT_ROOT="$(cd "$PROJECT_ROOT" && pwd)"

ensure_router_rs
ROUTER_RS="$(router_rs_cmd)"

echo "==> install --to cursor --scope user" >&2
"$ROUTER_RS" framework host-integration install \
  --framework-root "$FRAMEWORK_ROOT" \
  --project-root "$PROJECT_ROOT" \
  --artifact-root "$PROJECT_ROOT/artifacts" \
  --scope user \
  --to cursor

echo "==> status" >&2
"$ROUTER_RS" framework host-integration status \
  --framework-root "$FRAMEWORK_ROOT" \
  --project-root "$PROJECT_ROOT" \
  --artifact-root "$PROJECT_ROOT/artifacts"

echo "" >&2
echo "Done. Restart Cursor so user rules/MCP reload. Re-run after framework updates:" >&2
echo "  $FRAMEWORK_ROOT/scripts/install-cursor.sh" >&2
