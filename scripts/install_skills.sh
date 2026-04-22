#!/usr/bin/env bash
# install_skills.sh — Compatibility installer.
#
# Usage:
#   bash scripts/install_skills.sh init     # First-time setup
#   bash scripts/install_skills.sh all      # Install to all supported tools
#   bash scripts/install_skills.sh ls       # Show installation status
#   bash scripts/install_skills.sh <tool>   # Install to specific tool
#   bash scripts/install_skills.sh rm <tool> # Remove from specific tool

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SKILLS_ROOT="$REPO_ROOT/skills"
DEFAULT_BOOTSTRAP_PATH="$REPO_ROOT/artifacts/bootstrap/framework_default_bootstrap.json"
PLUGIN_NAME="skill-framework-native"
HOME_PLUGIN_ROOT="$HOME/.codex/plugins/$PLUGIN_NAME"
HOME_MARKETPLACE_PATH="$HOME/.agents/plugins/marketplace.json"
HOME_CLAUDE_SKILLS_PATH="$HOME/.claude/skills"
HOME_CLAUDE_REFRESH_PATH="$HOME/.claude/commands/refresh.md"
HOME_CLAUDE_MCP_CONFIG_PATH="$HOME/.claude.json"
PROJECT_INSTRUCTIONS_PATH="$REPO_ROOT/.codex/model_instructions.md"
FRAMEWORK_START_MARKER="<!-- FRAMEWORK_DEFAULT_RUNTIME_START -->"

# Supported tools and their skill paths
TOOLS="codex claude agents gemini"

get_tool_path() {
  case "$1" in
    codex)  echo "$HOME/.codex/skills" ;;
    claude) echo "$HOME/.claude/skills" ;;
    agents) echo "$HOME/.agents/skills" ;;
    gemini) echo "$HOME/.gemini/skills" ;;
    *)      echo "" ;;
  esac
}

usage() {
  echo "Usage: $(basename "$0") <command> [tool...]"
  echo ""
  echo "Commands:"
  echo "  init          First-time setup (Codex native integration + other tool skill links)"
  echo "  all           Install to all supported tools"
  echo "  ls            Show installation status"
  echo "  rm <tool>     Remove symlink for a specific tool"
  echo "  <tool>        Install to a specific tool"
  echo ""
  echo "Supported tools: $TOOLS"
  echo "Skills source: $SKILLS_ROOT"
  echo "Codex default path: native integration installer + default bootstrap bundle"
}

run_codex_native_install() {
  local cmd=(python3 "$SCRIPT_DIR/install_codex_native_integration.py")

  if [ -n "${CODEX_NATIVE_BOOTSTRAP_OUTPUT_DIR:-}" ]; then
    cmd+=(--bootstrap-output-dir "$CODEX_NATIVE_BOOTSTRAP_OUTPUT_DIR")
  fi
  if [ "${CODEX_NATIVE_SKIP_DEFAULT_BOOTSTRAP:-0}" = "1" ]; then
    cmd+=(--skip-default-bootstrap)
  fi

  "${cmd[@]}" >/dev/null
  echo "  ✓ codex — native integration installed"
}

bootstrap_payload_matches_contract() {
  local bootstrap_path="$1"
  if [ ! -f "$bootstrap_path" ]; then
    return 1
  fi

  python3 - "$bootstrap_path" "$REPO_ROOT" <<'PY' >/dev/null
import json
import sys
from pathlib import Path

bootstrap_path = Path(sys.argv[1])
repo_root = str(Path(sys.argv[2]).resolve())
payload = json.loads(bootstrap_path.read_text(encoding="utf-8"))

bootstrap = payload.get("bootstrap")
memory = payload.get("memory-bootstrap")
skills = payload.get("skills-export")
proposals = payload.get("evolution-proposals")

if not isinstance(bootstrap, dict):
    raise SystemExit(1)
if bootstrap.get("repo_root") != repo_root:
    raise SystemExit(1)
if not isinstance(memory, dict):
    raise SystemExit(1)
if not isinstance(skills, dict) or skills.get("source") != "skills/SKILL_ROUTING_RUNTIME.json":
    raise SystemExit(1)
if not isinstance(proposals, dict):
    raise SystemExit(1)
PY
}

marketplace_has_framework_plugin() {
  local marketplace_path="$1"
  if [ ! -f "$marketplace_path" ]; then
    return 1
  fi

  python3 - "$marketplace_path" "$PLUGIN_NAME" <<'PY' >/dev/null
import json
import sys
from pathlib import Path

marketplace_path = Path(sys.argv[1])
plugin_name = sys.argv[2]
payload = json.loads(marketplace_path.read_text(encoding="utf-8"))
plugins = payload.get("plugins")
if not isinstance(plugins, list):
    raise SystemExit(1)
if not any(isinstance(plugin, dict) and plugin.get("name") == plugin_name for plugin in plugins):
    raise SystemExit(1)
PY
}

claude_mcp_has_shared_servers() {
  local config_path="$1"
  if [ ! -f "$config_path" ]; then
    return 1
  fi

  python3 - "$config_path" "$REPO_ROOT" <<'PY' >/dev/null
import json
import sys
from pathlib import Path

config_path = Path(sys.argv[1])
repo_root = str(Path(sys.argv[2]).resolve())
payload = json.loads(config_path.read_text(encoding="utf-8"))
servers = payload.get("mcpServers")
if not isinstance(servers, dict):
    raise SystemExit(1)
browser = servers.get("browser-mcp")
framework = servers.get("framework-mcp")
openai_docs = servers.get("openaiDeveloperDocs")
if not isinstance(browser, dict) or not isinstance(framework, dict) or not isinstance(openai_docs, dict):
    raise SystemExit(1)
if browser.get("command") != "bash":
    raise SystemExit(1)
if browser.get("cwd") != repo_root:
    raise SystemExit(1)
if framework.get("command") != "python3":
    raise SystemExit(1)
if framework.get("cwd") != repo_root:
    raise SystemExit(1)
env = framework.get("env")
if not isinstance(env, dict) or env.get("PYTHONPATH") != repo_root:
    raise SystemExit(1)
if openai_docs.get("type") != "http":
    raise SystemExit(1)
if openai_docs.get("url") != "https://developers.openai.com/mcp":
    raise SystemExit(1)
PY
}

skills_link_matches_repo() {
  local target_path="$1"

  if [ ! -L "$target_path" ]; then
    return 1
  fi

  local current_target resolved_target resolved_source
  current_target="$(readlink "$target_path")"
  resolved_target="$(cd "$(dirname "$target_path")" && cd "$(dirname "$current_target")" && pwd)/$(basename "$current_target")"
  resolved_source="$(cd "$SKILLS_ROOT" && pwd)"
  [ "$resolved_target" = "$resolved_source" ]
}

show_codex_status() {
  local config_path="$HOME/.codex/config.toml"
  local bootstrap_path="$DEFAULT_BOOTSTRAP_PATH"
  local config_ok="false"
  local bootstrap_ok="false"
  local plugin_ok="false"
  local marketplace_ok="false"
  local claude_skills_ok="false"
  local refresh_ok="false"
  local claude_mcp_ok="false"
  local overlay_ok="false"

  if [ -n "${CODEX_NATIVE_BOOTSTRAP_OUTPUT_DIR:-}" ]; then
    case "$CODEX_NATIVE_BOOTSTRAP_OUTPUT_DIR" in
      *.json) bootstrap_path="$CODEX_NATIVE_BOOTSTRAP_OUTPUT_DIR" ;;
      *) bootstrap_path="${CODEX_NATIVE_BOOTSTRAP_OUTPUT_DIR%/}/framework_default_bootstrap.json" ;;
    esac
  fi

  if [ -f "$config_path" ] \
    && grep -q '\[mcp_servers.browser-mcp\]' "$config_path" \
    && grep -q '\[mcp_servers.framework-mcp\]' "$config_path" \
    && grep -q '\[mcp_servers.openaiDeveloperDocs\]' "$config_path" \
    && grep -q '^\[tui\]' "$config_path" \
    && grep -Eq '^[[:space:]]*status_line[[:space:]]*=' "$config_path"; then
    config_ok="true"
  fi
  if skills_link_matches_repo "$HOME_CLAUDE_SKILLS_PATH"; then
    claude_skills_ok="true"
  fi
  if bootstrap_payload_matches_contract "$bootstrap_path"; then
    bootstrap_ok="true"
  fi
  if [ -f "$HOME_PLUGIN_ROOT/.codex-plugin/plugin.json" ]; then
    plugin_ok="true"
  fi
  if marketplace_has_framework_plugin "$HOME_MARKETPLACE_PATH"; then
    marketplace_ok="true"
  fi
  if [ -f "$HOME_CLAUDE_REFRESH_PATH" ] && cmp -s "$HOME_CLAUDE_REFRESH_PATH" "$REPO_ROOT/.claude/commands/refresh.md"; then
    refresh_ok="true"
  fi
  if claude_mcp_has_shared_servers "$HOME_CLAUDE_MCP_CONFIG_PATH"; then
    claude_mcp_ok="true"
  fi
  if [ ! -e "$PROJECT_INSTRUCTIONS_PATH" ] || ! grep -q "$FRAMEWORK_START_MARKER" "$PROJECT_INSTRUCTIONS_PATH"; then
    overlay_ok="true"
  fi

  if [ "$config_ok" = "true" ] \
    && [ "$bootstrap_ok" = "true" ] \
    && [ "$plugin_ok" = "true" ] \
    && [ "$marketplace_ok" = "true" ] \
    && [ "$claude_skills_ok" = "true" ] \
    && [ "$refresh_ok" = "true" ] \
    && [ "$claude_mcp_ok" = "true" ] \
    && [ "$overlay_ok" = "true" ]; then
    echo "  ✓ codex → native integration ready"
  else
    echo "  ⚠ codex → native integration incomplete (config:$config_ok bootstrap:$bootstrap_ok plugin:$plugin_ok marketplace:$marketplace_ok claude_skills:$claude_skills_ok refresh:$refresh_ok claude_mcp:$claude_mcp_ok overlay:$overlay_ok)"
  fi
}

install_tool() {
  local tool="$1"
  if [ "$tool" = "codex" ]; then
    run_codex_native_install
    return 0
  fi

  local target
  target="$(get_tool_path "$tool")"

  if [ -z "$target" ]; then
    echo "Unknown tool: $tool"
    echo "Supported tools: $TOOLS"
    return 1
  fi

  local parent_dir
  parent_dir="$(dirname "$target")"

  # Create parent directory if needed
  if [ ! -d "$parent_dir" ]; then
    mkdir -p "$parent_dir"
    echo "  Created directory: $parent_dir"
  fi

  # Check if already correctly linked
  if [ -L "$target" ]; then
    local current_target resolved_target resolved_source
    current_target="$(readlink "$target")"
    resolved_target="$(cd "$(dirname "$target")" && cd "$(dirname "$current_target")" && pwd)/$(basename "$current_target")"
    resolved_source="$(cd "$SKILLS_ROOT" && pwd)"
    if [ "$resolved_target" = "$resolved_source" ]; then
      echo "  ✓ $tool — already linked → $SKILLS_ROOT"
      return 0
    else
      echo "  ⚠ $tool — symlink exists but points to $current_target, updating..."
      rm "$target"
    fi
  elif [ -e "$target" ]; then
    echo "  ⚠ $tool — $target exists but is not a symlink, backing up..."
    mv "$target" "${target}.bak"
  fi

  ln -s "$SKILLS_ROOT" "$target"
  echo "  ✓ $tool — linked $target → $SKILLS_ROOT"
}

remove_tool() {
  local tool="$1"
  local target
  target="$(get_tool_path "$tool")"

  if [ -z "$target" ]; then
    echo "Unknown tool: $tool"
    return 1
  fi

  if [ -L "$target" ]; then
    rm "$target"
    echo "  ✓ $tool — removed symlink $target"
  elif [ -e "$target" ]; then
    echo "  ⚠ $tool — $target exists but is not a symlink, skipping"
  else
    echo "  ℹ $tool — no symlink found at $target"
  fi
}

show_status() {
  local skill_count
  skill_count=$(find "$SKILLS_ROOT" -maxdepth 1 -type d -not -name ".*" -not -name "dist" | wc -l | tr -d ' ')
  skill_count=$((skill_count - 1))

  echo "Skills source: $SKILLS_ROOT"
  echo "Total skills: $skill_count"
  echo ""
  echo "Installation status:"
  for tool in $TOOLS; do
    if [ "$tool" = "codex" ]; then
      show_codex_status
      continue
    fi
    local target
    target="$(get_tool_path "$tool")"
    if [ -L "$target" ]; then
      local link_target resolved_target resolved_source
      link_target="$(readlink "$target")"
      resolved_target="$(cd "$(dirname "$target")" && cd "$(dirname "$link_target")" && pwd)/$(basename "$link_target")"
      resolved_source="$(cd "$SKILLS_ROOT" && pwd)"
      if [ "$resolved_target" = "$resolved_source" ]; then
        echo "  ✓ $tool → $target"
      else
        echo "  ⚠ $tool → $target (points to $link_target)"
      fi
    elif [ -e "$target" ]; then
      echo "  ⚠ $tool → $target (exists but not a symlink)"
    else
      echo "  ✗ $tool → $target (not installed)"
    fi
  done
}

# Main
if [ $# -lt 1 ]; then
  usage
  exit 1
fi

command="$1"
shift

case "$command" in
  init|all)
    echo "Installing skills to all supported tools..."
    echo "Source: $SKILLS_ROOT"
    echo ""
    for tool in $TOOLS; do
      install_tool "$tool"
    done
    echo ""
    echo "Done!"
    ;;
  ls|status)
    show_status
    ;;
  rm|remove)
    if [ $# -lt 1 ]; then
      echo "Usage: $(basename "$0") rm <tool>"
      exit 1
    fi
    remove_tool "$1"
    if [ "$1" = "codex" ]; then
      echo "  ℹ codex — native config/plugin/bootstrap surfaces are left in place"
    fi
    ;;
  help|--help|-h)
    usage
    ;;
  *)
    install_tool "$command"
    ;;
esac
