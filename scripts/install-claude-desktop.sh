#!/usr/bin/env bash
# Install / refresh Claude Desktop MCP projection (project + user scope).
# Re-run after: git pull on skill framework, router-rs rebuild, or MCP/registry changes.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: install-claude-desktop.sh [options]

Installs harness MCP for Claude Desktop via router-rs host-integration.
Prefer ./scripts/install-claude.sh for Claude Code + Desktop + user-scope My lifecycle (Cursor parity).

Writes:
  project  → .claude/mcp.json, .claude/CLAUDE.md, .claude/settings.json (network research sandbox), .claude/.framework-projection-desktop.json
  user     → ~/Library/Application Support/Claude/claude_desktop_config.json (macOS)
             + ~/Library/Application Support/Claude-3p/claude_desktop_config.json (macOS 3P mode — merges mcpServers, keeps preferences)
             + ~/.local/share/skill-framework/bin/router-rs (stable MCP binary, adhoc codesign on macOS)
             + ~/.claude/CLAUDE.md (Desktop 短指针)
             MCP: router-rs-framework + browser-mcp + web_fetch tool

After install: restart Claude Desktop Chat, then Settings → Developer → Connectors
(or + → Connectors) and confirm router-rs-framework is connected.

Options:
  --framework-root DIR   Framework repo (default: $SKILL_FRAMEWORK_ROOT or script ../..)
  --project-root DIR     Project root (default: $PWD)
  --scope SCOPE          project | user | both (default: both)
  --skip-build           Do not run cargo build --release when router-rs missing/stale
  -h, --help             Show help

Environment:
  SKILL_FRAMEWORK_ROOT   Default framework root
  ROUTER_RS_BIN          Pin router-rs binary (optional)

When to re-run:
  - Pulled updates to core/router-rs, configs/framework/, or docs/hosts/claude-desktop.md
  - Rebuilt router-rs (cargo build --release in core/router-rs)
  - Claude Desktop shows MCP disconnected or wrong repo-root behavior
  - Switched machines (run with --scope user on each Mac)

Example (framework repo itself):
  ./scripts/install-claude-desktop.sh

Example (another project checked out with .claude/ in repo):
  SKILL_FRAMEWORK_ROOT=~/Developer/skill ./scripts/install-claude-desktop.sh --project-root ~/Developer/my-app
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
  candidates+=("$FRAMEWORK_ROOT/core/router-rs/target/release/router-rs")
  if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    candidates+=("${CARGO_TARGET_DIR}/release/router-rs")
  fi
  candidates+=("/tmp/skill-cargo-target/release/router-rs")
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
  echo "error: router-rs not found; build with: cargo build --release --manifest-path core/router-rs/Cargo.toml" >&2
  exit 1
}

ensure_router_rs() {
  if [[ "${SKIP_BUILD:-0}" == 1 ]]; then
    return
  fi
  if [[ -n "${ROUTER_RS_BIN:-}" && -x "$ROUTER_RS_BIN" ]]; then
    return
  fi
  if command -v router-rs >/dev/null 2>&1; then
    return
  fi
  echo "==> building router-rs (release)..." >&2
  (cd "$FRAMEWORK_ROOT/core/router-rs" && cargo build --release)
}

install_scope() {
  local scope="$1"
  echo "==> install --to claude-desktop --scope ${scope}" >&2
  "$ROUTER_RS" framework host-integration install \
    --framework-root "$FRAMEWORK_ROOT" \
    --project-root "$PROJECT_ROOT" \
    --artifact-root "$PROJECT_ROOT/artifacts" \
    --scope "$scope" \
    --to claude-desktop
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

FRAMEWORK_ROOT="$(resolve_framework_root)"
PROJECT_ROOT="${PROJECT_ROOT:-$PWD}"
PROJECT_ROOT="$(cd "$PROJECT_ROOT" && pwd)"

INSTALL_CLAUDE="${FRAMEWORK_ROOT}/scripts/install-claude.sh"
if [[ -x "$INSTALL_CLAUDE" ]]; then
  EXTRA=()
  if [[ "$SKIP_BUILD" -eq 1 ]]; then
    EXTRA+=(--skip-build)
  fi
  exec "$INSTALL_CLAUDE" \
    --framework-root "$FRAMEWORK_ROOT" \
    --project-root "$PROJECT_ROOT" \
    --scope "$SCOPE" \
    --desktop-only \
    "${EXTRA[@]}"
fi

ensure_router_rs
ROUTER_RS="$(router_rs_cmd)"

case "$SCOPE" in
  project)
    install_scope project
    ;;
  user)
    install_scope user
    ;;
  both)
    install_scope project
    install_scope user
    ;;
  *)
    echo "error: --scope must be project, user, or both" >&2
    exit 1
    ;;
esac

echo "==> status" >&2
"$ROUTER_RS" framework host-integration status \
  --framework-root "$FRAMEWORK_ROOT" \
  --project-root "$PROJECT_ROOT" \
  --artifact-root "$PROJECT_ROOT/artifacts"

echo "" >&2
echo "Done. Restart Claude Desktop (Cmd+Q), then verify Connectors → router-rs-framework + browser-mcp." >&2
echo "3P mode: MCP merged into Claude-3p/claude_desktop_config.json; binary: ~/.local/share/skill-framework/bin/router-rs" >&2
PATCH_EGRESS="${FRAMEWORK_ROOT}/scripts/patch-claude-desktop-3p-cowork-egress.sh"
if [[ -x "$PATCH_EGRESS" ]]; then
  echo "==> 3P Cowork egress (coworkEgressAllowedHosts)" >&2
  "$PATCH_EGRESS" --allow-all || true
fi
echo "Networking runbook: docs/hosts/claude-desktop-networking.md" >&2
echo "Re-run after framework updates:" >&2
echo "  $FRAMEWORK_ROOT/scripts/install-claude-desktop.sh --project-root $PROJECT_ROOT" >&2
