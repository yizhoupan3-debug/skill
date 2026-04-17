#!/usr/bin/env bash
# Setup a Slidev project in the current directory.
# Creates slides.md from template, initializes Node project.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

echo "🎯 Setting up Slidev project..."

# Create project structure
mkdir -p assets rendered components

# Copy template if slides.md doesn't exist
if [ ! -f "slides.md" ]; then
  cp "$SKILL_DIR/assets/slidev.template.md" slides.md
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

# Initialize Node project if needed
if [ ! -f "package.json" ]; then
  npm init -y --silent
  echo "  ✅ Initialized package.json"
fi

# Install Slidev
if npx slidev --version &>/dev/null 2>&1; then
  echo "  ✅ Slidev available"
else
  echo "  📦 Installing @slidev/cli..."
  npm install --save-dev @slidev/cli @slidev/theme-default
fi

echo ""
echo "📋 Quick commands:"
echo "  Preview:     npx slidev slides.md"
echo "  Export PDF:   npx slidev export slides.md"
echo "  Build SPA:   npx slidev build slides.md"
echo ""
echo "✅ Slidev project ready!"
