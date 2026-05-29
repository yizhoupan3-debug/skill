#!/usr/bin/env bash
# Install / refresh Claude Desktop framework projections (delegates to install-claude.sh).
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install-claude-desktop.sh [options]

Delegates to install-claude.sh --desktop-only (MCP + research settings + optional 3P egress patch).

Options match install-claude.sh: --framework-root, --project-root, --scope, --skip-build, -h
EOF
}

FRAMEWORK_ROOT_ARG=""
PROJECT_ROOT=""
SCOPE="both"
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

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_CLAUDE="${here}/install-claude.sh"
if [[ ! -x "$INSTALL_CLAUDE" ]]; then
  echo "error: missing $INSTALL_CLAUDE" >&2
  exit 1
fi

EXTRA=()
[[ -n "$FRAMEWORK_ROOT_ARG" ]] && EXTRA+=(--framework-root "$FRAMEWORK_ROOT_ARG")
[[ -n "$PROJECT_ROOT" ]] && EXTRA+=(--project-root "$PROJECT_ROOT")
EXTRA+=(--scope "$SCOPE" --desktop-only)
[[ "$SKIP_BUILD" -eq 1 ]] && EXTRA+=(--skip-build)

exec "$INSTALL_CLAUDE" "${EXTRA[@]}"
