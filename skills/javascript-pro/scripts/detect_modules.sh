#!/usr/bin/env bash
# javascript-pro ESM/CJS detection script
# Detects module system usage and potential interop issues
set -euo pipefail

echo "=== javascript-pro module system diagnostics ==="
echo ""

# 1. package.json type field
echo "--- package.json ---"
if [ -f "package.json" ]; then
  TYPE_FIELD=$(grep '"type"' package.json 2>/dev/null | head -1 | grep -oE '"(module|commonjs)"' | tr -d '"' || true)
  if [ "$TYPE_FIELD" = "module" ]; then
    echo "[pkg] type: \"module\" (ESM by default)"
  elif [ "$TYPE_FIELD" = "commonjs" ]; then
    echo "[pkg] type: \"commonjs\" (CJS by default)"
  else
    echo "[pkg] type: not set (CJS by default per Node.js convention)"
  fi

  # Check exports field
  if grep -q '"exports"' package.json 2>/dev/null; then
    echo "[pkg] 'exports' field present (modern resolution)"
    if grep -q '"import"' package.json 2>/dev/null && grep -q '"require"' package.json 2>/dev/null; then
      echo "[pkg] dual ESM/CJS exports detected ✓"
    fi
  fi

  # Check main/module fields
  if grep -q '"main"' package.json 2>/dev/null; then
    MAIN=$(grep '"main"' package.json | head -1 | grep -oE '"[^"]*"' | tail -1 | tr -d '"')
    echo "[pkg] main: $MAIN"
  fi
  if grep -q '"module"' package.json 2>/dev/null; then
    MODULE=$(grep '"module"' package.json | head -1 | grep -oE '"[^"]*"' | tail -1 | tr -d '"')
    echo "[pkg] module: $MODULE"
  fi
else
  echo "[pkg] WARN: no package.json found"
fi

# 2. File extension analysis
echo ""
echo "--- file extensions ---"
MJS_COUNT=$(find . -name "*.mjs" -not -path "*/node_modules/*" 2>/dev/null | wc -l | tr -d ' ')
CJS_COUNT=$(find . -name "*.cjs" -not -path "*/node_modules/*" 2>/dev/null | wc -l | tr -d ' ')
JS_COUNT=$(find . -name "*.js" -not -path "*/node_modules/*" -not -path "*/dist/*" -not -path "*/build/*" 2>/dev/null | wc -l | tr -d ' ')

echo "[ext] .js files: $JS_COUNT"
echo "[ext] .mjs files: $MJS_COUNT"
echo "[ext] .cjs files: $CJS_COUNT"

# 3. Import/export pattern analysis
echo ""
echo "--- import/export patterns ---"
if [ "$JS_COUNT" -gt 0 ] || [ "$MJS_COUNT" -gt 0 ]; then
  ESM_IMPORT=$(grep -r '^\s*import\s' --include="*.js" --include="*.mjs" -l . 2>/dev/null | grep -v node_modules | wc -l | tr -d ' ')
  ESM_EXPORT=$(grep -r '^\s*export\s' --include="*.js" --include="*.mjs" -l . 2>/dev/null | grep -v node_modules | wc -l | tr -d ' ')
  CJS_REQUIRE=$(grep -r '\brequire(' --include="*.js" --include="*.cjs" -l . 2>/dev/null | grep -v node_modules | wc -l | tr -d ' ')
  CJS_EXPORTS=$(grep -r 'module\.exports' --include="*.js" --include="*.cjs" -l . 2>/dev/null | grep -v node_modules | wc -l | tr -d ' ')

  echo "[pattern] Files with ESM import: $ESM_IMPORT"
  echo "[pattern] Files with ESM export: $ESM_EXPORT"
  echo "[pattern] Files with require(): $CJS_REQUIRE"
  echo "[pattern] Files with module.exports: $CJS_EXPORTS"

  # Mixed module warning
  if [ "$ESM_IMPORT" -gt 0 ] && [ "$CJS_REQUIRE" -gt 0 ]; then
    echo ""
    echo "[WARN] Mixed ESM and CJS patterns detected — potential interop issues"
    echo "[WARN] Files mixing both patterns:"
    grep -rl 'require(' --include="*.js" . 2>/dev/null | grep -v node_modules | while read -r f; do
      if grep -q '^\s*import\s' "$f" 2>/dev/null; then
        echo "  - $f"
      fi
    done | head -5
  fi
fi

# 4. Config files
echo ""
echo "--- config files ---"
for cfg in .eslintrc .eslintrc.js .eslintrc.json .eslintrc.cjs eslint.config.js eslint.config.mjs; do
  if [ -f "$cfg" ]; then
    echo "[config] $cfg found"
  fi
done

for cfg in .prettierrc .prettierrc.js .prettierrc.json prettier.config.js; do
  if [ -f "$cfg" ]; then
    echo "[config] $cfg found"
  fi
done

for cfg in .babelrc babel.config.js babel.config.json; do
  if [ -f "$cfg" ]; then
    echo "[config] $cfg found"
  fi
done

# 5. Node.js version
echo ""
echo "--- runtime ---"
if command -v node &>/dev/null; then
  echo "[node] $(node --version)"
else
  echo "[node] WARN: node not found"
fi

echo ""
echo "=== diagnostics complete ==="
