#!/usr/bin/env bash
# Setup a Marp project in the current directory.
# Creates slides.md from template, installs dependencies.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

echo "🎯 Setting up Marp project..."

# Create project structure
mkdir -p assets rendered

# Copy template if slides.md doesn't exist
if [ ! -f "slides.md" ]; then
  cp "$SKILL_DIR/assets/slides.template.md" slides.md
  echo "  ✅ Created slides.md from template"
else
  echo "  ⚠️ slides.md already exists, skipping"
fi

# Create sources.md if not exists
if [ ! -f "sources.md" ]; then
  echo "# Sources" > sources.md
  echo "" >> sources.md
  echo "| Slide | Source | URL | License |" >> sources.md
  echo "|-------|--------|-----|---------|" >> sources.md
  echo "  ✅ Created sources.md"
fi

# Check if marp-cli is available
if npx @marp-team/marp-cli --version &>/dev/null; then
  echo "  ✅ Marp CLI available"
else
  echo "  📦 Installing @marp-team/marp-cli..."
  npm install --save-dev @marp-team/marp-cli
fi

echo ""
echo "📋 Quick commands:"
echo "  Preview:     npx @marp-team/marp-cli --preview slides.md"
echo "  Export PDF:   npx @marp-team/marp-cli slides.md --pdf"
echo "  Export PPTX:  npx @marp-team/marp-cli slides.md --pptx"
echo "  Export HTML:  npx @marp-team/marp-cli slides.md"
echo ""
echo "✅ Marp project ready!"
