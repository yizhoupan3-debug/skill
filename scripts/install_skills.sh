#!/usr/bin/env bash
# install_skills.sh — Cross-platform AI tool skill installer via symlinks.
# Inspired by uaio/open-skills.
#
# Usage:
#   bash scripts/install_skills.sh init     # First-time setup
#   bash scripts/install_skills.sh all      # Install to all supported tools
#   bash scripts/install_skills.sh ls       # Show installation status
#   bash scripts/install_skills.sh <tool>   # Install to specific tool
#   bash scripts/install_skills.sh rm <tool> # Remove from specific tool

set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILLS_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)/skills"

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
  echo "  init          First-time setup (create symlinks for all tools)"
  echo "  all           Install to all supported tools"
  echo "  ls            Show installation status"
  echo "  rm <tool>     Remove symlink for a specific tool"
  echo "  <tool>        Install to a specific tool"
  echo ""
  echo "Supported tools: $TOOLS"
  echo "Skills source: $SKILLS_ROOT"
}

install_tool() {
  local tool="$1"
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
    ;;
  help|--help|-h)
    usage
    ;;
  *)
    install_tool "$command"
    ;;
esac
