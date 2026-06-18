#!/usr/bin/env bash
# Link framework skills/AGENTS.md into another project and install Claude projections.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: claude-bootstrap-framework.sh [options]

Run from the target project root (or pass --project-root). Symlinks framework skills +
AGENTS.md, writes .claude/router-rs-hook.env, and runs install-claude.sh (project scope).

Options:
  --framework-root DIR   Framework repo root
  --project-root DIR     Project to modify (default: $PWD)
  --with-configs         Symlink configs/ -> <framework>/configs
  --skip-desktop         Claude project install only (no Desktop MCP in project)
  -h, --help             Show help

Global Claude rules (My lifecycle, same as Cursor user framework.mdc) require user scope once:
  SKILL_FRAMEWORK_ROOT=<framework> ./scripts/install-claude.sh --scope user

Environment:
  SKILL_FRAMEWORK_ROOT   Default framework root
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

safe_symlink() {
  local target=$1
  local linkpath=$2
  local name
  name=$(basename "$linkpath")
  if [[ -e "$linkpath" || -L "$linkpath" ]]; then
    if [[ -L "$linkpath" ]]; then
      local cur
      cur=$(readlink "$linkpath" || true)
      if [[ "$cur" == "$target" ]]; then
        echo "$name: symlink already -> $target"
        return
      fi
    elif [[ -d "$linkpath" ]] || [[ -f "$linkpath" ]]; then
      echo "error: $linkpath exists and is not a symlink" >&2
      exit 1
    fi
  fi
  ln -sfn "$target" "$linkpath"
  echo "symlink $linkpath -> $target"
}

FRAMEWORK_ROOT_ARG=""
PROJECT_ROOT=""
WITH_CONFIGS=0
SKIP_DESKTOP=0

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
    --with-configs)
      WITH_CONFIGS=1
      shift
      ;;
    --skip-desktop)
      SKIP_DESKTOP=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

FRAMEWORK_ROOT="$(resolve_framework_root)"
PROJECT_ROOT="${PROJECT_ROOT:-$PWD}"
PROJECT_ROOT="$(cd "$PROJECT_ROOT" && pwd)"

for need in "${FRAMEWORK_ROOT}/skills" "${FRAMEWORK_ROOT}/AGENTS.md" "${FRAMEWORK_ROOT}/scripts/install-claude.sh"; do
  if [[ ! -e "$need" ]]; then
    echo "error: missing $need" >&2
    exit 1
  fi
done

mkdir -p "${PROJECT_ROOT}/.claude"
install_hook_env() {
  local dest="${PROJECT_ROOT}/.claude/router-rs-hook.env"
  if [[ -f "$dest" ]] && grep -q '^SKILL_FRAMEWORK_ROOT=' "$dest" 2>/dev/null; then
    echo "router-rs-hook.env already present; skipping"
    return
  fi
  {
    if [[ -f "${FRAMEWORK_ROOT}/.claude/router-rs-hook.env" ]]; then
      cat "${FRAMEWORK_ROOT}/.claude/router-rs-hook.env"
    elif [[ -f "${FRAMEWORK_ROOT}/configs/framework/claude-router-rs-hook.env" ]]; then
      cat "${FRAMEWORK_ROOT}/configs/framework/claude-router-rs-hook.env"
    fi
    echo "SKILL_FRAMEWORK_ROOT=${FRAMEWORK_ROOT}"
  } > "$dest"
  echo "wrote $dest"
}

install_hook_env
safe_symlink "${FRAMEWORK_ROOT}/skills" "${PROJECT_ROOT}/skills"
safe_symlink "${FRAMEWORK_ROOT}/AGENTS.md" "${PROJECT_ROOT}/AGENTS.md"

if [[ "$WITH_CONFIGS" -eq 1 ]]; then
  safe_symlink "${FRAMEWORK_ROOT}/configs" "${PROJECT_ROOT}/configs"
fi

INSTALL_ARGS=(--framework-root "$FRAMEWORK_ROOT" --project-root "$PROJECT_ROOT" --scope project)
if [[ "$SKIP_DESKTOP" -eq 1 ]]; then
  INSTALL_ARGS+=(--code-only)
fi
"${FRAMEWORK_ROOT}/scripts/install-claude.sh" "${INSTALL_ARGS[@]}"

echo "done. For global My lifecycle (like Cursor framework.mdc), also run:"
echo "  SKILL_FRAMEWORK_ROOT=${FRAMEWORK_ROOT} ${FRAMEWORK_ROOT}/scripts/install-claude.sh --scope user"
