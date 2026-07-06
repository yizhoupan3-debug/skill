#!/usr/bin/env bash
# Skill Framework — Fresh Clone Setup
# Usage: bash setup/install.sh
# Run from repo root after cloning.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
echo "[setup] Installing skill framework from: $REPO_ROOT"

# ── 1. Detect platform ──────────────────────────────────────────────
OS="$(uname -s)"
IS_WINDOWS=false
CLAUDE_DIR="$HOME/.claude"
case "$OS" in
  MINGW*|MSYS*|CYGWIN*)
    IS_WINDOWS=true
    CLI_DIR="$HOME/AppData/Local/Claude Code"
    echo "[setup] Windows (Git Bash) detected"
    ;;
  Darwin|Linux)
    echo "[setup] Unix-like system detected: $OS"
    ;;
  *)
    echo "[setup] Unknown OS: $OS — proceeding with Unix conventions"
    ;;
esac

# ── 2. Ensure ~/.claude directory ────────────────────────────────────
mkdir -p "$CLAUDE_DIR"
mkdir -p "$CLAUDE_DIR/rules"
mkdir -p "$CLAUDE_DIR/skills"

# ── 3. Install user-level CLAUDE.md (global output rules) ──────────
if [ -f "$REPO_ROOT/setup/CLAUDE_USER.md" ]; then
  cp "$REPO_ROOT/setup/CLAUDE_USER.md" "$CLAUDE_DIR/CLAUDE.md"
  echo "[setup] ✓ Installed user-level CLAUDE.md → $CLAUDE_DIR/CLAUDE.md"
fi

# ── 4. Install framework rules ─────────────────────────────────────
if [ -f "$REPO_ROOT/setup/framework.md" ]; then
  cp "$REPO_ROOT/setup/framework.md" "$CLAUDE_DIR/rules/framework.md"
  echo "[setup] ✓ Installed framework rules → $CLAUDE_DIR/rules/framework.md"
fi

# ── 5. Install project CLAUDE.md (self-referential, tracks the repo) ─
echo "[setup] Project CLAUDE.md is at .claude/CLAUDE.md in the repo — Claude loads it automatically."

# ── 6. Install native skills (symlinks from repo skills/ → ~/.claude/skills/) ──
echo "[setup] Installing native skill symlinks..."
for skill_dir in "$REPO_ROOT"/skills/*/; do
  skill_name="$(basename "$skill_dir")"
  # Skip internal directories
  case "$skill_name" in
    SKILL_ROUTING_*|research-harness|research) continue ;;
  esac
  # Only symlink if the skill has a SKILL.md
  if [ -f "$skill_dir/SKILL.md" ]; then
    target="$CLAUDE_DIR/skills/$skill_name"
    if [ ! -L "$target" ] && [ ! -d "$target" ]; then
      if $IS_WINDOWS; then
        # Windows: copy instead of symlink (Git Bash often lacks symlink support)
        mkdir -p "$target"
        cp -r "$skill_dir"/* "$target/" 2>/dev/null || true
        echo "[setup]   ✓ Copied native skill: $skill_name"
      else
        ln -sf "$skill_dir" "$target"
        echo "[setup]   ✓ Symlinked native skill: $skill_name"
      fi
    else
      echo "[setup]   - Already exists: $skill_name"
    fi
  fi
done

# Also handle skills like good-question, good-story (submodules)
for extra in gitx simplify update deepinterview smoke goalx initx; do
  src="$REPO_ROOT/skills/$extra"
  tgt="$CLAUDE_DIR/skills/$extra"
  if [ -d "$src" ] && [ ! -L "$tgt" ] && [ ! -d "$tgt" ]; then
    if $IS_WINDOWS; then
      mkdir -p "$tgt"
      cp -r "$src"/* "$tgt/" 2>/dev/null || true
    else
      ln -sf "$src" "$tgt"
    fi
    echo "[setup]   ✓ Installed native skill: $extra"
  fi
done

# ── 7. Router-rs binary ────────────────────────────────────────────
if ! command -v router-rs-cli &>/dev/null; then
  echo "[setup] Building router-rs CLI (cargo build)..."
  cd "$REPO_ROOT/core/router-rs"
  cargo build --release --bin router-rs-cli 2>&1 || {
    echo "[setup] ⚠ cargo build failed — you may need to install Rust first: https://rustup.rs"
    echo "[setup]   After installing Rust, run: cd $REPO_ROOT/core/router-rs && cargo build --release --bin router-rs-cli"
  }
  # Copy binary to ~/.local/bin/ for PATH access
  mkdir -p "$HOME/.local/bin"
  if [ -f "$REPO_ROOT/core/router-rs/target/release/router-rs-cli" ]; then
    cp "$REPO_ROOT/core/router-rs/target/release/router-rs-cli" "$HOME/.local/bin/"
    echo "[setup] ✓ router-rs-cli installed to ~/.local/bin/"
    echo "[setup]   Ensure ~/.local/bin is in your PATH"
  fi
  cd "$REPO_ROOT"
else
  echo "[setup] ✓ router-rs-cli already available in PATH"
fi

# ── 8. Create router-rs-hook.env ──────────────────────────────────
ENV_FILE="$REPO_ROOT/.claude/router-rs-hook.env"
if [ ! -f "$ENV_FILE" ]; then
  cat > "$ENV_FILE" << 'ENVEOF'
# Router-rs hook env — customize paths for your machine
SKILL_FRAMEWORK_ROOT=<REPO_ROOT>
ROUTER_RS_BIN=${HOME}/.local/bin/router-rs-cli
ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0
ENVEOF
  # Replace <REPO_ROOT> placeholder
  sed "s|<REPO_ROOT>|$REPO_ROOT|g" "$ENV_FILE" > "${ENV_FILE}.tmp" && mv "${ENV_FILE}.tmp" "$ENV_FILE"
  echo "[setup] ✓ Created .claude/router-rs-hook.env"
fi

# ── 9. Install settings.json with hooks ────────────────────────────
if [ ! -f "$REPO_ROOT/.claude/settings.json" ]; then
  if [ -f "$REPO_ROOT/setup/settings.json" ]; then
    cp "$REPO_ROOT/setup/settings.json" "$REPO_ROOT/.claude/settings.json"
    echo "[setup] ✓ Installed .claude/settings.json with hooks"
  fi
else
  echo "[setup] - .claude/settings.json already exists (merge setup/settings.json manually if needed)"
fi

# ── 10. Verify ──────────────────────────────────────────────────────
echo ""
echo "========== Setup Summary =========="
echo "Repository: $REPO_ROOT"
echo "User-level CLAUDE.md:  $( [ -f "$CLAUDE_DIR/CLAUDE.md" ] && echo '✓' || echo '✗' )"
echo "Framework rules:       $( [ -f "$CLAUDE_DIR/rules/framework.md" ] && echo '✓' || echo '✗' )"
echo "Project CLAUDE.md:     $( [ -f "$REPO_ROOT/.claude/CLAUDE.md" ] && echo '✓' || echo '✗' )"
echo "Native skills:         $(ls "$CLAUDE_DIR/skills/" 2>/dev/null | wc -l) installed"
echo "Router-rs binary:      $(command -v router-rs-cli &>/dev/null && echo '✓' || echo '✗ (build required)')"
echo "Hook env:              $( [ -f "$REPO_ROOT/.claude/router-rs-hook.env" ] && echo '✓' || echo '✗' )"
echo "Settings with hooks:   $( [ -f "$REPO_ROOT/.claude/settings.json" ] && echo '✓' || echo '✗' )"
echo ""
echo "Next step: open this repo in Claude Code / Codex and you're ready."
echo "=================================="
