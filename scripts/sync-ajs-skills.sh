#!/usr/bin/env bash
# sync-ajs-skills.sh — Selective sync of Awesome-Journal-Skills
#
# Syncs only the core journals/conferences from brycewang-stanford/Awesome-Journal-Skills
# into skills/journal-ajs/ as standalone SKILL.md files.
#
# The full repo has 2902 skills; this picks ~113 core ones from 9 journals/conferences.
#
# Usage: bash scripts/sync-ajs-skills.sh

set -euo pipefail
cd "$(git rev-parse --show-toplevel)" || exit 1

AJS_REPO="https://github.com/brycewang-stanford/Awesome-Journal-Skills.git"
AJS_TMP="/tmp/ajs-tmp-$$"
TARGET="skills/journal-ajs"
SCRIPT_NAME="scripts/sync-ajs-skills.sh"
LOCAL_REFS="skills/journal-ajs/references"

# Journals to sync: directory_name_in_ajs, local_skill_slug
JOURNALS=(
  "Economic-Research-Journal-Skills:ajs-economic-research"
  "Journal-of-Management-World-Skills:ajs-management-world"
  "Journal-of-World-Economy-Skills:ajs-world-economy"
  "NeurIPS-Skills:ajs-neurips"
  "ICML-Skills:ajs-icml"
  "ICLR-Skills:ajs-iclr"
  "AAAI-Skills:ajs-aaai"
  "Science-Skills:ajs-science"
  "PNAS-Skills:ajs-pnas"
)

echo "==> Cloning Awesome-Journal-Skills (sparse)..."
git clone --depth 1 --filter=blob:none --sparse "$AJS_REPO" "$AJS_TMP" 2>/dev/null

# Build sparse checkout path list
CHECKOUT_PATHS=()
for entry in "${JOURNALS[@]}"; do
  dir_name="${entry%%:*}"
  CHECKOUT_PATHS+=("$dir_name/skills")
done

cd "$AJS_TMP"
git sparse-checkout set "${CHECKOUT_PATHS[@]}" 2>/dev/null
cd - >/dev/null

mkdir -p "$TARGET" "$LOCAL_REFS"

# Copy shared references first
if [ -d "$AJS_TMP/shared-resources" ]; then
  cp -r "$AJS_TMP/shared-resources/journal-selection/"* "$LOCAL_REFS/" 2>/dev/null || true
  echo "  -> Copied shared references"
fi

# Copy each journal's skills
TOTAL_SKILLS=0
for entry in "${JOURNALS[@]}"; do
  dir_name="${entry%%:*}"
  slug="${entry##*:}"
  src="$AJS_TMP/$dir_name/skills"

  if [ ! -d "$src" ]; then
    echo "  SKIP: $dir_name (not found)"
    continue
  fi

  dst="$TARGET/$slug"
  mkdir -p "$dst"
  cp -r "$src"/* "$dst/" 2>/dev/null || true
  count=$(find "$dst" -name "SKILL.md" 2>/dev/null | wc -l | tr -d ' ')
  echo "  -> $slug: $count skills"
  TOTAL_SKILLS=$((TOTAL_SKILLS + count))
done

# Cleanup
rm -rf "$AJS_TMP"

echo ""
echo "==> Sync complete: $TOTAL_SKILLS skills in $TARGET/"
echo "==> Next step: register in SKILL_ROUTING_RUNTIME.json (run update-ajs-routes)"
echo "==> Then: bash scripts/sync-framework-global.sh"
