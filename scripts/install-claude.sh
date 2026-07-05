#!/usr/bin/env bash
# Install / refresh Claude (claude) framework projections (align with Cursor My lifecycle).
# Re-run after: git pull on skill framework, router-rs rebuild, or stale ~/.claude/rules/framework.md (GSD text).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install-claude.sh [options]

Installs harness projections for Claude (claude host; hooks + framework rule).

Writes:
  claude project  → .claude/rules/framework.md, .claude/CLAUDE.md, .claude/settings.json, .claude/.framework-projection.json
  claude user     → ~/.claude/rules/framework.md, ~/.claude/settings.json (global My lifecycle + hooks)

Options:
  --framework-root DIR   Framework repo (default: $SKILL_FRAMEWORK_ROOT or script ../..)
  --project-root DIR     Project root (default: $PWD)
  --scope SCOPE          project | user | both (default: both — matches Cursor publish parity)
  --skip-build           Do not run cargo build --release when router-rs missing
  -h, --help             Show help

Note: claude-desktop host retired 2026-06.

Example (framework repo):
  ./scripts/install-claude.sh

Example (another project):
  SKILL_FRAMEWORK_ROOT=~/Developer/skill ./scripts/install-claude.sh --project-root ~/Developer/my-app
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
  echo "error: cannot infer framework root; set SKILL_FRAMEWORK_ROOT or pass --framework-root" >&2
  exit 1
}

router_rs_cmd() {
  local candidates=()
  candidates+=("$FRAMEWORK_ROOT/core/router-rs/target/release/router-rs-cli")
  if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    candidates+=("${CARGO_TARGET_DIR}/release/router-rs-cli")
  fi
  candidates+=("/tmp/skill-${UID:-0}-cargo-target/release/router-rs-cli")
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
  if command -v router-rs-cli >/dev/null 2>&1; then
    command -v router-rs-cli
    return
  fi
  echo "error: router-rs not found; build: cargo build --release --manifest-path core/router-rs/Cargo.toml" >&2
  exit 1
}

ensure_router_rs() {
  if [[ "${SKIP_BUILD:-0}" == 1 ]]; then
    return
  fi
  if [[ -n "${ROUTER_RS_BIN:-}" && -x "$ROUTER_RS_BIN" ]]; then
    return
  fi
  if command -v router-rs-cli >/dev/null 2>&1; then
    return
  fi
  echo "==> building router-rs (release)..." >&2
  (cd "$FRAMEWORK_ROOT/core/router-rs" && cargo build --release)
}

install_host() {
  local tool="$1"
  local scope="$2"
  echo "==> install --to ${tool} --scope ${scope}" >&2
  "$ROUTER_RS" framework host-integration install \
    --framework-root "$FRAMEWORK_ROOT" \
    --project-root "$PROJECT_ROOT" \
    --artifact-root "$PROJECT_ROOT/artifacts" \
    --scope "$scope" \
    --to "$tool"
}

FRAMEWORK_ROOT_ARG=""
PROJECT_ROOT=""
SCOPE="both"
# Retired 2026-06 (claude-desktop removed)
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
    --scope)
      SCOPE="${2:?}"
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

# --code-only and --desktop-only retired 2026-06 (claude-desktop removed).

FRAMEWORK_ROOT="$(resolve_framework_root)"
PROJECT_ROOT="${PROJECT_ROOT:-$PWD}"
PROJECT_ROOT="$(cd "$PROJECT_ROOT" && pwd)"

ensure_router_rs
ROUTER_RS="$(router_rs_cmd)"

scopes=()
case "$SCOPE" in
  project) scopes=(project) ;;
  user) scopes=(user) ;;
  both) scopes=(project user) ;;
  *)
    echo "error: --scope must be project, user, or both" >&2
    exit 1
    ;;
esac

for scope in "${scopes[@]}"; do
  install_host claude "$scope"
done

# ---------- sync project-level MCP .mcp.json + settings + CLAUDE.md ----------
if [[ "$SCOPE" == "project" || "$SCOPE" == "both" ]]; then
  SYNC_SCRIPT="$FRAMEWORK_ROOT/scripts/sync-project.sh"
  if [ -x "$SYNC_SCRIPT" ]; then
    echo "==> Syncing project MCP bridge + settings + framework ref..." >&2
    "$SYNC_SCRIPT" --force "$PROJECT_ROOT"
  fi
fi

echo "==> status" >&2
"$ROUTER_RS" framework host-integration status \
  --framework-root "$FRAMEWORK_ROOT" \
  --project-root "$PROJECT_ROOT" \
  --artifact-root "$PROJECT_ROOT/artifacts"

echo "" >&2
echo "Done. Claude: confirm ~/.claude/rules/framework.md lifecycle is set." >&2
echo "Re-run after framework updates:" >&2
echo "  $FRAMEWORK_ROOT/scripts/install-claude.sh" >&2
