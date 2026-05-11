#!/usr/bin/env bash
# Link framework skills/AGENTS.md (and optional Cursor rules) into another project and
# install .cursor/hooks.json that calls router-rs on PATH. Template:
#   configs/framework/cursor-hooks.workspace-template.json
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: cursor-bootstrap-framework.sh [options]

Run from the target project root (or pass --project-root). Symlinks framework skills +
AGENTS.md and writes .cursor/hooks.json (PATH-based router-rs).

Options:
  --framework-root DIR   Framework repo root (default: $SKILL_FRAMEWORK_ROOT, else
                         directory containing this script's ../.. when script lives
                         under <framework>/scripts/)
  --project-root DIR     Project to modify (default: $PWD)
  --with-cursor-rules    Symlink .cursor/rules -> <framework>/.cursor/rules
  --with-configs         Symlink configs/ -> <framework>/configs (推荐：与框架根目录
                         共享 HARNESS_OPERATOR_NUDGES 等；否则 hooks 仅用内置默认)
  --force                Overwrite .cursor/hooks.json even if it differs from template
  -h, --help             Show this help

Environment:
  SKILL_FRAMEWORK_ROOT   Default framework root if --framework-root omitted
  ROUTER_RS_BIN          Not read by this script; set in your shell profile if the
                         hooks should use a non-default binary (hooks expand
                         "${ROUTER_RS_BIN:-router-rs}").

Example:
  cd /tmp/foo
  /path/to/skill/scripts/cursor-bootstrap-framework.sh --framework-root /path/to/skill \\
    --with-cursor-rules --with-configs
EOF
}

resolve_framework_root() {
  if [[ -n "${FRAMEWORK_ROOT_ARG:-}" ]]; then
    local p
    p=$(cd "$FRAMEWORK_ROOT_ARG" && pwd)
    echo "$p"
    return
  fi
  if [[ -n "${SKILL_FRAMEWORK_ROOT:-}" ]]; then
    local p
    p=$(cd "$SKILL_FRAMEWORK_ROOT" && pwd)
    echo "$p"
    return
  fi
  local here
  here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  if [[ "$here" == */scripts ]]; then
    (cd "$here/.." && pwd)
    return
  fi
  echo "error: cannot infer framework root; set SKILL_FRAMEWORK_ROOT or pass --framework-root" >&2
  exit 1
}

FRAMEWORK_ROOT_ARG=""
PROJECT_ROOT=""
WITH_RULES=0
WITH_CONFIGS=0
FORCE=0

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
    --with-cursor-rules)
      WITH_RULES=1
      shift
      ;;
    --with-configs)
      WITH_CONFIGS=1
      shift
      ;;
    --force)
      FORCE=1
      shift
      ;;
    -h|--help)
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
if [[ -z "${PROJECT_ROOT}" ]]; then
  PROJECT_ROOT=$(pwd)
else
  PROJECT_ROOT=$(cd "$PROJECT_ROOT" && pwd)
fi

TEMPLATE="${FRAMEWORK_ROOT}/configs/framework/cursor-hooks.workspace-template.json"
for need in "$TEMPLATE" "${FRAMEWORK_ROOT}/skills" "${FRAMEWORK_ROOT}/AGENTS.md"; do
  if [[ ! -e "$need" ]]; then
    echo "error: missing required path under framework root: $need" >&2
    exit 1
  fi
done

mkdir -p "${PROJECT_ROOT}/.cursor"

install_hooks() {
  local dest="${PROJECT_ROOT}/.cursor/hooks.json"
  if [[ -f "$dest" ]] && [[ "$FORCE" -eq 0 ]]; then
    if cmp -s "$TEMPLATE" "$dest"; then
      echo "hooks.json already matches workspace template; skipping"
      return
    fi
    echo "error: $dest exists and differs from template; use --force to replace" >&2
    exit 1
  fi
  cp "$TEMPLATE" "$dest"
  echo "wrote $dest"
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
      echo "error: $linkpath exists and is not a symlink; remove or move it first" >&2
      exit 1
    fi
  fi
  ln -sfn "$target" "$linkpath"
  echo "symlink $linkpath -> $target"
}

install_hooks
safe_symlink "${FRAMEWORK_ROOT}/skills" "${PROJECT_ROOT}/skills"
safe_symlink "${FRAMEWORK_ROOT}/AGENTS.md" "${PROJECT_ROOT}/AGENTS.md"

if [[ "$WITH_RULES" -eq 1 ]]; then
  if [[ ! -d "${FRAMEWORK_ROOT}/.cursor/rules" ]]; then
    echo "error: ${FRAMEWORK_ROOT}/.cursor/rules not found" >&2
    exit 1
  fi
  safe_symlink "${FRAMEWORK_ROOT}/.cursor/rules" "${PROJECT_ROOT}/.cursor/rules"
fi

if [[ "$WITH_CONFIGS" -eq 1 ]]; then
  if [[ ! -d "${FRAMEWORK_ROOT}/configs" ]]; then
    echo "error: ${FRAMEWORK_ROOT}/configs not found" >&2
    exit 1
  fi
  safe_symlink "${FRAMEWORK_ROOT}/configs" "${PROJECT_ROOT}/configs"
fi

echo "done. Ensure router-rs is on PATH (e.g. cargo install --path \"${FRAMEWORK_ROOT}/scripts/router-rs\")."
