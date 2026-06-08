#!/usr/bin/env bash
# Idempotent install/refresh for all five closed-set hosts (codex, claude-code, antigravity, cursor, opencode).
# Run from the framework repo after pull or router-rs upgrade.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install-all-hosts.sh [options]

Refreshes host projections for the skill framework closed set:
  1) codex        — framework sync-entrypoints (AGENTS_CODEX, .codex/*)
  2) cursor       — host-integration install --to cursor --scope user
  3) claude-code  — install-claude.sh (project + user; includes legacy desktop MCP steps)
  4) antigravity  — host-integration install --to antigravity --scope project
  5) opencode     — host-integration install --to opencode --scope project

Environment:
  SKILL_FRAMEWORK_ROOT   Framework repo root (default: parent of scripts/)
  ROUTER_RS_BIN          Optional pinned router-rs binary (else PATH or release build)
  PROJECT_ROOT           Target project (default: framework root when run in-repo)

Options:
  --framework-root DIR   Same as SKILL_FRAMEWORK_ROOT
  --project-root DIR     Project receiving project-scoped projections
  --skip-claude          Skip install-claude.sh (cursor/antigravity/opencode/codex only)
  --skip-build           Pass --skip-build to install-claude.sh
  --dry-run              Print steps only
  -h, --help             Show help

Example (framework source repo):
  ./scripts/install-all-hosts.sh

Example (consumer project):
  SKILL_FRAMEWORK_ROOT=~/Developer/skill ./scripts/install-all-hosts.sh --project-root ~/Developer/my-app
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
  if [[ -n "${ROUTER_RS_BIN:-}" && -x "$ROUTER_RS_BIN" ]]; then
    echo "$ROUTER_RS_BIN"
    return
  fi
  local release="${FRAMEWORK_ROOT}/core/router-rs/target/release/router-rs"
  if [[ -x "$release" ]]; then
    echo "$release"
    return
  fi
  if command -v router-rs >/dev/null 2>&1; then
    command -v router-rs
    return
  fi
  echo "error: router-rs not found; install to PATH or build core/router-rs" >&2
  exit 1
}

FRAMEWORK_ROOT_ARG=""
PROJECT_ROOT=""
SKIP_CLAUDE=0
SKIP_BUILD=0
DRY_RUN=0

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
    --skip-claude)
      SKIP_CLAUDE=1
      shift
      ;;
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
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
PROJECT_ROOT="${PROJECT_ROOT:-$FRAMEWORK_ROOT}"
PROJECT_ROOT="$(cd "$PROJECT_ROOT" && pwd)"
ARTIFACT_ROOT="${PROJECT_ROOT}/artifacts"
ROUTER_RS="$(router_rs_cmd)"
INSTALL_CLAUDE="${FRAMEWORK_ROOT}/scripts/install-claude.sh"

run() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[dry-run] $*" >&2
    return 0
  fi
  echo "==> $*" >&2
  "$@"
}

host_install() {
  local host="$1"
  local scope="$2"
  run "$ROUTER_RS" framework host-integration install \
    --framework-root "$FRAMEWORK_ROOT" \
    --project-root "$PROJECT_ROOT" \
    --artifact-root "$ARTIFACT_ROOT" \
    --scope "$scope" \
    --to "$host"
}

echo "framework_root=$FRAMEWORK_ROOT project_root=$PROJECT_ROOT router_rs=$ROUTER_RS" >&2

run "$ROUTER_RS" framework sync-entrypoints --repo-root "$FRAMEWORK_ROOT"

host_install cursor user

if [[ "$SKIP_CLAUDE" -eq 0 ]]; then
  if [[ ! -x "$INSTALL_CLAUDE" ]]; then
    echo "error: missing $INSTALL_CLAUDE" >&2
    exit 1
  fi
  CLAUDE_ARGS=(--framework-root "$FRAMEWORK_ROOT" --project-root "$PROJECT_ROOT")
  if [[ "$SKIP_BUILD" -eq 1 ]]; then
    CLAUDE_ARGS+=(--skip-build)
  fi
  run env SKILL_FRAMEWORK_ROOT="$FRAMEWORK_ROOT" "$INSTALL_CLAUDE" "${CLAUDE_ARGS[@]}"
else
  echo "==> skip install-claude.sh (--skip-claude)" >&2
fi

host_install antigravity project
host_install opencode project

if [[ "$DRY_RUN" -eq 0 ]]; then
  echo "==> framework doctor" >&2
  "$ROUTER_RS" framework doctor --repo-root "$FRAMEWORK_ROOT" || true
  echo "==> host-integration status (summary)" >&2
  "$ROUTER_RS" framework host-integration status \
    --framework-root "$FRAMEWORK_ROOT" \
    --project-root "$PROJECT_ROOT" \
    --artifact-root "$ARTIFACT_ROOT" || true
fi

echo "" >&2
echo "Done. Ensure router-rs is on PATH for OpenCode MCP (command: router-rs)." >&2
echo "Re-run: SKILL_FRAMEWORK_ROOT=$FRAMEWORK_ROOT $FRAMEWORK_ROOT/scripts/install-all-hosts.sh" >&2
